//! Two-Tier JIT & Compilation Runtime for NumLang.
//!
//! Provides a two-tier execution architecture:
//! - **Tier 0**: Ultra-fast cold start compilation (< 2ms cold latency) by lowering
//!   directly from AST/MIR to native code without supercompilation passes.
//! - **Tier 1**: Background speculative supercompilation executed asynchronously on
//!   a dedicated `SupercompileWorker` thread when a function crosses the hot threshold
//!   (default 100 invocations).
//! - **Atomic OSR Swap**: Function entry points store an `AtomicPtr<u8>`. When Tier 1
//!   compilation completes, the pointer is atomically updated using `Ordering::Release`.
//!   Subsequent function dispatches read this pointer using `Ordering::Acquire` to jump
//!   directly to specialized Tier 1 code without locking or pausing execution.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::codegen::cranelift::deopt::OsrTransitionSlot;
use crate::mir::lower::MirProgram;
use crate::mir::supercompiler::supercompile_mir_program;

/// Configuration knobs for the Tiered JIT runtime.
#[derive(Debug, Clone)]
pub struct TierConfig {
    /// Hotness invocation threshold required to trigger Tier 1 background compilation.
    pub hot_threshold: u64,
    /// Whether background Tier 1 worker thread should be spawned.
    pub enable_tier1: bool,
}

impl Default for TierConfig {
    fn default() -> Self {
        Self {
            hot_threshold: 100,
            enable_tier1: true,
        }
    }
}

/// Compilation request sent to the background SupercompileWorker.
pub struct TierTask {
    pub function_name: String,
    pub mir_snapshot: MirProgram,
    pub slot: Arc<OsrTransitionSlot>,
    pub on_complete: Option<Box<dyn FnOnce(*mut u8) + Send>>,
}

/// Background worker thread running Tier 1 supercompilation jobs.
pub struct SupercompileWorker {
    sender: Option<Sender<TierTask>>,
    worker_handle: Option<JoinHandle<()>>,
    shutdown_flag: Arc<AtomicBool>,
}

impl SupercompileWorker {
    /// Spawns a new background worker thread.
    pub fn spawn() -> Self {
        let (sender, receiver): (Sender<TierTask>, Receiver<TierTask>) = mpsc::channel();
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&shutdown_flag);

        let worker_handle = thread::spawn(move || {
            while let Ok(task) = receiver.recv() {
                if flag_clone.load(Ordering::Relaxed) {
                    break;
                }

                // Execute Tier 1 full supercompilation pipeline
                let mut sc_mir = task.mir_snapshot.clone();
                supercompile_mir_program(&mut sc_mir);

                // Compile supercompiled MIR to native object bytes
                if let Ok(obj_bytes) = crate::codegen::compile_mir_to_obj(&sc_mir) {
                    // Create stable pointer to compiled native artifacts
                    let boxed_bytes = Box::new(obj_bytes);
                    let raw_ptr = Box::into_raw(boxed_bytes) as *mut u8;

                    // Atomically swap the function pointer in the OSR slot
                    task.slot.upgrade(raw_ptr);

                    if let Some(cb) = task.on_complete {
                        cb(raw_ptr);
                    }
                }
            }
        });

        Self {
            sender: Some(sender),
            worker_handle: Some(worker_handle),
            shutdown_flag,
        }
    }

    /// Submits a task to the background worker.
    pub fn submit(&self, task: TierTask) -> Result<(), String> {
        match &self.sender {
            Some(sender) => sender
                .send(task)
                .map_err(|e| format!("Failed to send task to SupercompileWorker: {}", e)),
            None => Err("SupercompileWorker sender has been shut down".to_string()),
        }
    }

    /// Shuts down the background worker cleanly.
    pub fn shutdown(&mut self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
        self.sender.take(); // Dropping sender closes channel and unblocks receiver.recv()
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SupercompileWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Coordinator for tiered compilation across registered functions.
pub struct TierManager {
    config: TierConfig,
    slots: RwLock<HashMap<String, Arc<OsrTransitionSlot>>>,
    mir_snapshots: RwLock<HashMap<String, MirProgram>>,
    worker: Mutex<Option<SupercompileWorker>>,
}

impl TierManager {
    pub fn new(config: TierConfig) -> Self {
        let worker = if config.enable_tier1 {
            Some(SupercompileWorker::spawn())
        } else {
            None
        };

        Self {
            config,
            slots: RwLock::new(HashMap::new()),
            mir_snapshots: RwLock::new(HashMap::new()),
            worker: Mutex::new(worker),
        }
    }

    /// Register a function with its baseline MIR representation.
    pub fn register_function(&self, name: &str, mir: &MirProgram) -> Arc<OsrTransitionSlot> {
        let slot = Arc::new(OsrTransitionSlot::new(self.config.hot_threshold));
        if let Ok(mut slots) = self.slots.write() {
            slots.insert(name.to_string(), Arc::clone(&slot));
        }

        if let Ok(mut snapshots) = self.mir_snapshots.write() {
            snapshots.insert(name.to_string(), mir.clone());
        }

        slot
    }

    /// Record an invocation of a function.
    /// If the invocation count crosses the hot threshold, automatically dispatches
    /// a Tier 1 supercompilation job to the background SupercompileWorker.
    ///
    /// Returns `(is_tier1, Option<*mut u8>)`:
    /// - If Tier 1 is ready, returns `(true, Some(tier1_ptr))`.
    /// - If Tier 0 is active, returns `(false, None)`.
    pub fn record_call(&self, name: &str) -> (bool, Option<*mut u8>) {
        let slot = {
            let slots = match self.slots.read() {
                Ok(s) => s,
                Err(_) => return (false, None),
            };
            match slots.get(name) {
                Some(s) => Arc::clone(s),
                None => return (false, None),
            }
        };

        if slot.should_osr() {
            return (true, Some(slot.target_ptr()));
        }

        let crossed = slot.record_invocation();
        if crossed {
            // Trigger Tier 1 background compilation
            if let Ok(worker_guard) = self.worker.lock() {
                if let Some(worker) = worker_guard.as_ref() {
                    let snapshot = {
                        let snapshots = match self.mir_snapshots.read() {
                            Ok(s) => s,
                            Err(_) => return (false, None),
                        };
                        snapshots.get(name).cloned()
                    };

                    if let Some(mir) = snapshot {
                        let task = TierTask {
                            function_name: name.to_string(),
                            mir_snapshot: mir,
                            slot: Arc::clone(&slot),
                            on_complete: None,
                        };
                        let _ = worker.submit(task);
                    }
                }
            }
        }

        (false, None)
    }

    /// Wait until Tier 1 compilation finishes for `name`, up to `timeout`.
    pub fn wait_for_tier1(&self, name: &str, timeout: Duration) -> bool {
        let slot = {
            let slots = match self.slots.read() {
                Ok(s) => s,
                Err(_) => return false,
            };
            match slots.get(name) {
                Some(s) => Arc::clone(s),
                None => return false,
            }
        };

        let start = Instant::now();
        while start.elapsed() < timeout {
            if slot.should_osr() {
                return true;
            }
            thread::sleep(Duration::from_millis(2));
        }
        slot.should_osr()
    }

    /// Retrieve the transition slot for a function.
    pub fn get_slot(&self, name: &str) -> Option<Arc<OsrTransitionSlot>> {
        let slots = self.slots.read().ok()?;
        slots.get(name).cloned()
    }
}
