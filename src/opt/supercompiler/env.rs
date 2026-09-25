//! Partial evaluation environment for the NumLang supercompiler.
//!
//! `Env` maps variable names to their current `Value` during symbolic driving.
//! It also tracks the call stack to detect and break recursive cycles.

use std::collections::HashMap;
use super::value::{values_equal, Value};

/// The partial evaluation environment.
#[derive(Debug, Clone)]
pub struct Env {
    /// Variable bindings — the partial environment.
    bindings: HashMap<String, Value>,

    /// Active call stack: (function_name, concrete_args_key).
    /// Used to detect cycles in recursive or mutually recursive calls.
    call_stack: Vec<(String, Vec<Value>)>,

    /// Taint flag: set to true whenever driving encounters a symbolic
    /// value, unresolved condition, or unhandled statement.
    pub has_symbolic: bool,
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

impl Env {
    pub fn new() -> Self {
        Env {
            bindings: HashMap::new(),
            call_stack: Vec::new(),
            has_symbolic: false,
        }
    }

    /// Mark the environment as tainted by symbolic or non-concrete evaluation.
    pub fn mark_symbolic(&mut self) {
        self.has_symbolic = true;
    }

    /// Look up a variable.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.bindings.get(name)
    }

    /// Bind or update a variable.
    pub fn set(&mut self, name: String, val: Value) {
        if matches!(val, Value::Symbolic(_)) {
            self.has_symbolic = true;
        }
        self.bindings.insert(name, val);
    }

    /// Remove a variable (when it goes out of scope).
    pub fn remove(&mut self, name: &str) {
        self.bindings.remove(name);
    }

    /// Push a call frame. Returns false if this exact call is already on the
    /// stack (cycle detected — caller should return Symbolic instead).
    pub fn push_call(&mut self, name: String, args: Vec<Value>) -> bool {
        // Detect same call with same concrete args already active
        if self.call_stack.iter().any(|(n, a)| {
            n == &name
                && a.len() == args.len()
                && a.iter().zip(args.iter()).all(|(x, y)| values_equal(x, y))
        }) {
            return false; // cycle
        }
        self.call_stack.push((name, args));
        true
    }

    /// Pop the topmost call frame.
    pub fn pop_call(&mut self) {
        self.call_stack.pop();
    }

    /// Snapshot the current bindings for loop state tracking.
    /// Returns only the variables that are in `var_names`.
    pub fn snapshot_vars(&self, var_names: &[String]) -> HashMap<String, Value> {
        var_names
            .iter()
            .filter_map(|n| self.bindings.get(n).map(|v| (n.clone(), v.clone())))
            .collect()
    }

    /// Restore loop variables from a snapshot.
    pub fn restore_vars(&mut self, snapshot: &HashMap<String, Value>) {
        for (k, v) in snapshot {
            self.bindings.insert(k.clone(), v.clone());
        }
    }

    /// Create a child environment for function call scope.
    /// The child starts with the given parameter bindings.
    pub fn child_for_call(&self, params: Vec<(String, Value)>) -> Env {
        let mut child = Env {
            bindings: HashMap::new(),
            call_stack: self.call_stack.clone(),
            has_symbolic: false,
        };
        for (name, val) in params {
            if matches!(val, Value::Symbolic(_)) {
                child.has_symbolic = true;
            }
            child.bindings.insert(name, val);
        }
        child
    }

    pub fn call_depth(&self) -> usize {
        self.call_stack.len()
    }
}
