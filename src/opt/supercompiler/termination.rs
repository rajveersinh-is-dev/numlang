//! Termination detection for the NumLang supercompiler.
//!
//! Implements homeomorphic embedding over symbolic `Value`s to detect
//! when the driving of a loop is diverging. When embedding is detected,
//! we stop driving and hand off to the generalizer.

use std::collections::HashMap;
use super::value::{Value, SymExpr, values_equal};

/// A snapshot of loop-variable state after one iteration.
#[derive(Debug, Clone)]
pub struct LoopSnapshot {
    pub state: HashMap<String, Value>,
    pub iteration: usize,
}

/// Detect whether any earlier snapshot homeomorphically embeds the current one.
/// For our purposes: if states are identical (fixed point) or the
/// concrete integer values follow a detectable pattern.
pub fn detect_embedding(snapshots: &[LoopSnapshot], current: &LoopSnapshot) -> bool {
    if snapshots.is_empty() {
        return false;
    }
    // First check: exact fixed point — state did not change at all
    if let Some(prev) = snapshots.last() {
        if states_equal(&prev.state, &current.state) {
            return true; // converged to fixed point
        }
    }
    // Homeomorphic embedding: current state is "structurally contained in" earlier state.
    // For concrete integer sequences, we detect by symbolic depth increase.
    for prev in snapshots {
        if homeomorphic_embedding_states(&prev.state, &current.state) {
            return true;
        }
    }
    false
}

/// States are equal if all tracked variables map to equal values.
fn states_equal(a: &HashMap<String, Value>, b: &HashMap<String, Value>) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().all(|(k, av)| b.get(k).map_or(false, |bv| values_equal(av, bv)))
}

/// Homeomorphic embedding over value states.
/// For concrete values, we check if every key maps to a concrete value in both
/// (no symbolic drift), which means we can unroll. If symbolic values appear,
/// we detect if the symbolic expression trees are growing unboundedly.
fn homeomorphic_embedding_states(older: &HashMap<String, Value>, newer: &HashMap<String, Value>) -> bool {
    // If all values are still concrete in both, no embedding needed yet —
    // we just keep unrolling (up to 2048 cap).
    let older_all_concrete = older.values().all(|v| v.is_concrete());
    let newer_all_concrete = newer.values().all(|v| v.is_concrete());
    if older_all_concrete && newer_all_concrete {
        return false; // still concrete, no embedding
    }

    // If symbolic values appeared: check if newer symbolic expressions
    // embed the older ones (i.e., they are growing in depth).
    for (k, nv) in newer {
        if let Some(ov) = older.get(k) {
            if let (Value::Symbolic(ns), Value::Symbolic(os)) = (nv, ov) {
                if sym_depth(ns) > sym_depth(os) + 2 {
                    return true; // symbolic tree is growing — generalize
                }
            }
        }
    }
    false
}

/// Depth of a symbolic expression tree.
fn sym_depth(e: &SymExpr) -> usize {
    match e {
        SymExpr::Var(_, _) => 0,
        SymExpr::UnOp(_, inner, _) => 1 + sym_depth(inner),
        SymExpr::BinOp(_, l, r, _) => 1 + sym_depth(l).max(sym_depth(r)),
        SymExpr::Call(_, args, _) => 1 + args.iter().map(sym_depth).max().unwrap_or(0),
        SymExpr::If(c, t, e, _) => 1 + sym_depth(c).max(sym_depth(t)).max(sym_depth(e)),
        SymExpr::Index(t, i, _) => 1 + sym_depth(t).max(sym_depth(i)),
    }
}

/// Homeomorphic embedding of individual values.
/// Returns true if `v1` is embedded in `v2` (v2 is "at least as complex as v1").
pub fn values_embedding(v1: &Value, v2: &Value) -> bool {
    match (v1, v2) {
        // Concrete values embed trivially into anything
        (Value::Int(_), _) | (Value::Float(_), _) | (Value::Bool(_), _) => true,
        // Symbolic embedding: coupling (same constructor, args embed)
        (Value::Symbolic(s1), Value::Symbolic(s2)) => sym_embedding(s1, s2),
        _ => false,
    }
}

fn sym_embedding(s1: &SymExpr, s2: &SymExpr) -> bool {
    // Diving: s1 embeds in any subterm of s2
    if sym_diving(s1, s2) {
        return true;
    }
    // Coupling: same constructor, all children embed
    sym_coupling(s1, s2)
}

fn sym_diving(needle: &SymExpr, haystack: &SymExpr) -> bool {
    match haystack {
        SymExpr::Var(_, _) => false,
        SymExpr::UnOp(_, inner, _) => sym_embedding(needle, inner),
        SymExpr::BinOp(_, l, r, _) => sym_embedding(needle, l) || sym_embedding(needle, r),
        SymExpr::Call(_, args, _) => args.iter().any(|a| sym_embedding(needle, a)),
        SymExpr::If(c, t, e, _) => {
            sym_embedding(needle, c) || sym_embedding(needle, t) || sym_embedding(needle, e)
        }
        SymExpr::Index(t, i, _) => sym_embedding(needle, t) || sym_embedding(needle, i),
    }
}

fn sym_coupling(s1: &SymExpr, s2: &SymExpr) -> bool {
    match (s1, s2) {
        (SymExpr::Var(n1, _), SymExpr::Var(n2, _)) => n1 == n2,
        (SymExpr::UnOp(op1, i1, _), SymExpr::UnOp(op2, i2, _)) => {
            op1 == op2 && sym_embedding(i1, i2)
        }
        (SymExpr::BinOp(op1, l1, r1, _), SymExpr::BinOp(op2, l2, r2, _)) => {
            op1 == op2 && sym_embedding(l1, l2) && sym_embedding(r1, r2)
        }
        _ => false,
    }
}
