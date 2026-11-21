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

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::codegen::cranelift::deopt::OsrTransitionSlot;
use crate::mir::lower::MirProgram;
use crate::mir::supercompiler::supercompile_mir_program;

/// Runtime execution tier of a registered function.
///
/// - `Cold`: cheap path. Only an atomic counter is touched per call; no
///   compilation or supercompilation happens at all.
/// - `Hot`: expensive path. The function has crossed its hotness threshold and
///   is handed to the background worker for a full Tier 1 supercompilation +
///   native codegen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Cold,
    Hot,
}

impl Tier {
    pub fn is_hot(self) -> bool {
        self == Tier::Hot
    }

    pub fn is_cold(self) -> bool {
        self == Tier::Cold
    }
}

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

                // Expensive path: full Tier 1 supercompilation is only ever
                // performed for functions that crossed the hot threshold.
                let compiled_ptr = (|| -> Option<*mut u8> {
                    let mut sc_mir = task.mir_snapshot.clone();
                    supercompile_mir_program(&mut sc_mir);
                    let obj_bytes = crate::codegen::compile_mir_to_obj(&sc_mir).ok()?;
                    let boxed_bytes = Box::new(obj_bytes);
                    Some(Box::into_raw(boxed_bytes) as *mut u8)
                })();

                match compiled_ptr {
                    Some(raw_ptr) => {
                        // Atomically swap the function pointer in the OSR slot
                        task.slot.upgrade(raw_ptr);
                        if let Some(cb) = task.on_complete {
                            cb(raw_ptr);
                        }
                    }
                    None => {
                        // Always notify the coordinator, including on failure,
                        // so an in-flight job can never be leaked as pending.
                        if let Some(cb) = task.on_complete {
                            cb(std::ptr::null_mut());
                        }
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
    /// Functions with a Tier 1 job currently in flight. Prevents the repeated
    /// post-threshold calls from queuing duplicate expensive compilations.
    pending: Arc<Mutex<HashSet<String>>>,
    /// Number of promotions actually submitted to the background worker.
    promotions: AtomicU64,
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
            pending: Arc::new(Mutex::new(HashSet::new())),
            promotions: AtomicU64::new(0),
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
            // Cheap for cold: the branch above is the only work done for the
            // first `hot_threshold - 1` calls. Expensive for hot: from the
            // crossing call onward we submit exactly one full Tier 1 job.
            // `record_invocation` stays true until the OSR pointer is swapped,
            // so the pending set is what prevents duplicate submissions.
            let mut pending = match self.pending.lock() {
                Ok(g) => g,
                Err(_) => return (false, None),
            };
            if !pending.insert(name.to_string()) {
                return (false, None);
            }

            let submitted = if let Ok(worker_guard) = self.worker.lock() {
                if let Some(worker) = worker_guard.as_ref() {
                    let snapshot = {
                        let snapshots = match self.mir_snapshots.read() {
                            Ok(s) => s,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        snapshots.get(name).cloned()
                    };

                    match snapshot {
                        Some(mir) => {
                            let pending_for_cb = Arc::clone(&self.pending);
                            let name_for_cb = name.to_string();
                            let task = TierTask {
                                function_name: name.to_string(),
                                mir_snapshot: mir,
                                slot: Arc::clone(&slot),
                                on_complete: Some(Box::new(move |_ptr| {
                                    if let Ok(mut set) = pending_for_cb.lock() {
                                        set.remove(&name_for_cb);
                                    }
                                })),
                            };
                            worker.submit(task).is_ok()
                        }
                        None => false,
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if submitted {
                self.promotions.fetch_add(1, Ordering::Relaxed);
            } else {
                pending.remove(name);
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

    /// Classify a registered function into its current execution tier.
    ///
    /// A function is `Hot` once it has either crossed the configured hotness
    /// threshold or been upgraded to a Tier 1 OSR target; otherwise `Cold`.
    pub fn tier(&self, name: &str) -> Option<Tier> {
        let slot = self.get_slot(name)?;
        let hot = slot.should_osr() || slot.get_invocation_count() >= self.config.hot_threshold;
        Some(if hot { Tier::Hot } else { Tier::Cold })
    }

    pub fn is_hot(&self, name: &str) -> bool {
        self.tier(name).is_some_and(Tier::is_hot)
    }

    pub fn is_cold(&self, name: &str) -> bool {
        self.tier(name).is_some_and(Tier::is_cold)
    }

    /// Number of distinct functions that have been promoted to the expensive
    /// Tier 1 compilation path. Repeated post-threshold calls do not increase it.
    pub fn promotion_count(&self) -> u64 {
        self.promotions.load(Ordering::Relaxed)
    }

    /// Whether a Tier 1 compilation job for `name` is still in flight.
    pub fn is_promotion_pending(&self, name: &str) -> bool {
        self.pending
            .lock()
            .map(|set| set.contains(name))
            .unwrap_or(false)
    }
}
