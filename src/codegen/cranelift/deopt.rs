//! Deoptimization Safepoints & On-Stack Replacement (OSR) Runtime Infrastructure.
//!
//! Provides metadata tables for reconstructing unspecialized interpreter call frames
//! when speculative type guards fail, runtime deoptimization stubs, and atomic OSR
//! transition slots for hot-path upgrade from baseline to specialized native code.

use std::collections::HashMap;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::RwLock;
use crate::typecheck::Type;

/// Metadata describing a deoptimization safepoint site.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeoptMetadata {
    /// Unique safepoint identifier.
    pub deopt_id: u32,
    /// Function enclosing this safepoint.
    pub func_name: String,
    /// Unspecialized basic block ID to resume execution at.
    pub resume_bb: usize,
    /// Size of the interpreter frame in bytes.
    pub frame_size: usize,
    /// Live variables at the safepoint: (variable_name, type, offset_or_slot).
    pub live_vars: Vec<(String, Type, i64)>,
}

/// Registry storing metadata for all compiled deoptimization points.
#[derive(Debug, Default)]
pub struct DeoptTable {
    entries: HashMap<u32, DeoptMetadata>,
    deopt_event_counts: HashMap<u32, usize>,
}

impl DeoptTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            deopt_event_counts: HashMap::new(),
        }
    }

    /// Register metadata for a safepoint.
    pub fn register(&mut self, meta: DeoptMetadata) {
        self.entries.insert(meta.deopt_id, meta);
    }

    /// Retrieve metadata by deopt ID.
    pub fn get(&self, deopt_id: u32) -> Option<&DeoptMetadata> {
        self.entries.get(&deopt_id)
    }

    /// Record a dynamic deoptimization event.
    pub fn record_event(&mut self, deopt_id: u32) {
        *self.deopt_event_counts.entry(deopt_id).or_insert(0) += 1;
    }

    /// Get the total number of times deoptimization was triggered.
    pub fn total_deopts(&self) -> usize {
        self.deopt_event_counts.values().sum()
    }

    /// Clear all registered safepoints and metrics.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.deopt_event_counts.clear();
    }
}

/// Global thread-safe deoptimization registry.
static GLOBAL_DEOPT_TABLE: RwLock<Option<DeoptTable>> = RwLock::new(None);
static GLOBAL_DEOPT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Initialize the global deopt table.
pub fn init_global_deopt_table() {
    let mut table = GLOBAL_DEOPT_TABLE.write().unwrap();
    *table = Some(DeoptTable::new());
}

/// Register a safepoint in the global deopt table.
pub fn register_global_deopt(meta: DeoptMetadata) {
    let mut guard = GLOBAL_DEOPT_TABLE.write().unwrap();
    if guard.is_none() {
        *guard = Some(DeoptTable::new());
    }
    if let Some(ref mut table) = *guard {
        table.register(meta);
    }
}

/// Reconstruct an unspecialized interpreter call frame mapping variable names to raw values.
pub fn reconstruct_interpreter_frame(
    meta: &DeoptMetadata,
    raw_values: &[i64],
) -> HashMap<String, i64> {
    let mut frame = HashMap::new();
    for (i, (var_name, _, _)) in meta.live_vars.iter().enumerate() {
        let val = if i < raw_values.len() {
            raw_values[i]
        } else {
            0
        };
        frame.insert(var_name.clone(), val);
    }
    frame
}

/// Runtime deoptimization handler called when a speculative type guard fails.
#[no_mangle]
pub extern "C" fn __nl_deopt(deopt_id: u64, _frame_ptr: *const u8) -> i64 {
    GLOBAL_DEOPT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut guard = GLOBAL_DEOPT_TABLE.write().unwrap();
    if let Some(ref mut table) = *guard {
        table.record_event(deopt_id as u32);
    }
    0
}

/// Retrieve the total count of runtime deoptimization events.
pub fn get_runtime_deopt_count() -> u64 {
    GLOBAL_DEOPT_COUNTER.load(Ordering::SeqCst)
}

/// Reset runtime deoptimization telemetry.
pub fn reset_runtime_deopt_counter() {
    GLOBAL_DEOPT_COUNTER.store(0, Ordering::SeqCst);
}

/// On-Stack Replacement (OSR) transition slot located in function preambles.
/// Allows dynamic atomic upgrading from unspecialized Tier 0 to specialized Tier 1 native code.
pub struct OsrTransitionSlot {
    /// Atomic function pointer to Tier 1 specialized code (null if unspecialized).
    target_fn_ptr: AtomicPtr<u8>,
    /// Invocation counter tracking execution frequency.
    invocation_count: AtomicU64,
    /// Hotness threshold required to trigger OSR specialization.
    threshold: u64,
}

impl OsrTransitionSlot {
    pub fn new(threshold: u64) -> Self {
        Self {
            target_fn_ptr: AtomicPtr::new(std::ptr::null_mut()),
            invocation_count: AtomicU64::new(0),
            threshold,
        }
    }

    /// Record an entry invocation. Returns true if hot threshold has just been crossed.
    pub fn record_invocation(&self) -> bool {
        let count = self.invocation_count.fetch_add(1, Ordering::Relaxed);
        count + 1 >= self.threshold && self.target_fn_ptr.load(Ordering::Acquire).is_null()
    }

    /// Upgrade the slot with a compiled Tier 1 function pointer.
    pub fn upgrade(&self, specialized_fn: *mut u8) {
        self.target_fn_ptr.store(specialized_fn, Ordering::Release);
    }

    /// Check if specialized code is available for OSR jump.
    pub fn should_osr(&self) -> bool {
        !self.target_fn_ptr.load(Ordering::Acquire).is_null()
    }

    /// Get the specialized target function pointer.
    pub fn target_ptr(&self) -> *mut u8 {
        self.target_fn_ptr.load(Ordering::Acquire)
    }

    /// Current invocation count.
    pub fn get_invocation_count(&self) -> u64 {
        self.invocation_count.load(Ordering::Relaxed)
    }
}
