use std::collections::HashMap;

use super::term::{SymTerm, SymTermId, TermInterner};
use crate::ast::BinaryOp;
pub use crate::mir::memory_ssa::MemoryVersionId;
use crate::mir::{BasicBlockId, Place};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PathConstraintStore {
    /// Variables known to equal specific interned constant terms
    pub const_equalities: HashMap<SymTermId, SymTermId>,
    /// Equivalences between symbolic terms (union-find/leader mapping)
    pub term_equivalences: HashMap<SymTermId, SymTermId>,
    /// Lower and upper integer bounds for symbolic terms
    pub integer_bounds: HashMap<SymTermId, (Option<i64>, Option<i64>)>,
    /// Tracks sets of values that a symbolic term is known NOT to equal.
    pub excluded_values: HashMap<SymTermId, Vec<i64>>,
}

impl PathConstraintStore {
    pub fn new() -> Self {
        PathConstraintStore {
            const_equalities: HashMap::new(),
            term_equivalences: HashMap::new(),
            integer_bounds: HashMap::new(),
            excluded_values: HashMap::new(),
        }
    }

    pub fn add_not_equal_int(&mut self, term: SymTermId, val: i64) {
        let leader = self.find_leader(term);
        let entry = self.excluded_values.entry(leader).or_default();
        if !entry.contains(&val) {
            entry.push(val);
        }
    }

    /// If all values of an enum are excluded except one, return that one.
    pub fn deduce_must_equal_int(&self, term: SymTermId, all_variants: &[i64]) -> Option<i64> {
        let leader = self.find_leader(term);
        let excluded = self.excluded_values.get(&leader)?;
        let remaining: Vec<i64> = all_variants
            .iter()
            .copied()
            .filter(|v| !excluded.contains(v))
            .collect();
        if remaining.len() == 1 {
            Some(remaining[0])
        } else {
            None
        }
    }

    pub fn find_leader(&self, mut term: SymTermId) -> SymTermId {
        while let Some(&next) = self.term_equivalences.get(&term) {
            term = next;
        }
        term
    }

    pub fn add_condition(&mut self, cond: SymTermId, is_true: bool, interner: &TermInterner) {
        let root = self.find_leader(cond);
        match interner.get(root) {
            SymTerm::ConstBool(_) => {}
            SymTerm::Binary(op, left, right, _) => {
                let l = self.find_leader(*left);
                let r = self.find_leader(*right);

                match (op, is_true) {
                    (BinaryOp::Eq, true) | (BinaryOp::Ne, false) => {
                        self.unify_terms(l, r, interner);
                    }
                    (BinaryOp::Eq, false) | (BinaryOp::Ne, true) => {
                        if let SymTerm::ConstInt(val, _) = interner.get(r) {
                            self.add_not_equal_int(l, *val);
                        } else if let SymTerm::ConstInt(val, _) = interner.get(l) {
                            self.add_not_equal_int(r, *val);
                        }
                    }
                    (BinaryOp::Lt, true) | (BinaryOp::Ge, false) => {
                        self.add_less_than(l, r, interner);
                    }
                    (BinaryOp::Le, true) | (BinaryOp::Gt, false) => {
                        self.add_less_or_equal(l, r, interner);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub fn evaluate_condition(&self, cond: SymTermId, interner: &TermInterner) -> Option<bool> {
        let root = self.find_leader(cond);
        match interner.get(root) {
            SymTerm::ConstBool(b) => Some(*b),
            SymTerm::Binary(BinaryOp::Eq, l, r, _) => {
                let left_lead = self.find_leader(*l);
                let right_lead = self.find_leader(*r);
                if left_lead == right_lead {
                    return Some(true);
                }
                if let (Some(&l_val), Some(&r_val)) = (
                    self.const_equalities.get(&left_lead),
                    self.const_equalities.get(&right_lead),
                ) {
                    return Some(l_val == r_val);
                }
                if let SymTerm::ConstInt(r_val, _) = interner.get(right_lead) {
                    if let Some(excluded) = self.excluded_values.get(&left_lead) {
                        if excluded.contains(r_val) {
                            return Some(false);
                        }
                    }
                }
                if let SymTerm::ConstInt(l_val, _) = interner.get(left_lead) {
                    if let Some(excluded) = self.excluded_values.get(&right_lead) {
                        if excluded.contains(l_val) {
                            return Some(false);
                        }
                    }
                }
                None
            }
            SymTerm::Binary(BinaryOp::Ne, l, r, _) => {
                let left_lead = self.find_leader(*l);
                let right_lead = self.find_leader(*r);
                if left_lead == right_lead {
                    return Some(false);
                }
                if let (Some(&l_val), Some(&r_val)) = (
                    self.const_equalities.get(&left_lead),
                    self.const_equalities.get(&right_lead),
                ) {
                    return Some(l_val != r_val);
                }
                None
            }
            SymTerm::Binary(BinaryOp::Lt, l, r, _) => {
                let left_lead = self.find_leader(*l);
                let right_lead = self.find_leader(*r);
                if left_lead == right_lead {
                    return Some(false);
                }
                let (l_min, l_max) = self
                    .integer_bounds
                    .get(&left_lead)
                    .copied()
                    .unwrap_or((None, None));
                let (r_min, r_max) = self
                    .integer_bounds
                    .get(&right_lead)
                    .copied()
                    .unwrap_or((None, None));

                if let (Some(max_l), Some(min_r)) = (l_max, r_min) {
                    if max_l < min_r {
                        return Some(true);
                    }
                }
                if let (Some(min_l), Some(max_r)) = (l_min, r_max) {
                    if min_l >= max_r {
                        return Some(false);
                    }
                }
                None
            }
            SymTerm::Binary(BinaryOp::Gt, l, r, _) => {
                let left_lead = self.find_leader(*l);
                let right_lead = self.find_leader(*r);
                if left_lead == right_lead {
                    return Some(false);
                }
                None
            }
            SymTerm::Binary(BinaryOp::Le, l, r, _) => {
                let left_lead = self.find_leader(*l);
                let right_lead = self.find_leader(*r);
                if left_lead == right_lead {
                    return Some(true);
                }
                None
            }
            SymTerm::Binary(BinaryOp::Ge, l, r, _) => {
                let left_lead = self.find_leader(*l);
                let right_lead = self.find_leader(*r);
                if left_lead == right_lead {
                    return Some(true);
                }
                None
            }
            _ => None,
        }
    }

    fn unify_terms(&mut self, t1: SymTermId, t2: SymTermId, interner: &TermInterner) {
        if t1 == t2 {
            return;
        }
        self.term_equivalences.insert(t1, t2);

        // Propagate constants
        if matches!(
            interner.get(t1),
            SymTerm::ConstInt(_, _) | SymTerm::ConstFloat(_, _) | SymTerm::ConstBool(_)
        ) {
            self.const_equalities.insert(t2, t1);
        }
        if matches!(
            interner.get(t2),
            SymTerm::ConstInt(_, _) | SymTerm::ConstFloat(_, _) | SymTerm::ConstBool(_)
        ) {
            self.const_equalities.insert(t1, t2);
        }
    }

    fn add_less_than(&mut self, l: SymTermId, r: SymTermId, interner: &TermInterner) {
        if let SymTerm::ConstInt(r_val, _) = interner.get(r) {
            let bounds = self.integer_bounds.entry(l).or_insert((None, None));
            bounds.1 = Some(bounds.1.map_or(r_val - 1, |max| max.min(r_val - 1)));
        }
        if let SymTerm::ConstInt(l_val, _) = interner.get(l) {
            let bounds = self.integer_bounds.entry(r).or_insert((None, None));
            bounds.0 = Some(bounds.0.map_or(l_val + 1, |min| min.max(l_val + 1)));
        }
    }

    fn add_less_or_equal(&mut self, l: SymTermId, r: SymTermId, interner: &TermInterner) {
        if let SymTerm::ConstInt(r_val, _) = interner.get(r) {
            let bounds = self.integer_bounds.entry(l).or_insert((None, None));
            bounds.1 = Some(bounds.1.map_or(*r_val, |max| max.min(*r_val)));
        }
        if let SymTerm::ConstInt(l_val, _) = interner.get(l) {
            let bounds = self.integer_bounds.entry(r).or_insert((None, None));
            bounds.0 = Some(bounds.0.map_or(*l_val, |min| min.max(*l_val)));
        }
    }
}

/// Interval refinement attached to a symbolic term at a given program point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interval {
    pub lo: Option<i64>, // None = -∞
    pub hi: Option<i64>, // None = +∞
}

impl Interval {
    pub const FULL: Self = Interval { lo: None, hi: None };
    pub const fn exact(v: i64) -> Self {
        Interval {
            lo: Some(v),
            hi: Some(v),
        }
    }

    pub fn meet(&self, other: &Interval) -> Interval {
        // intersect: take max of los, min of his
        let lo = match (self.lo, other.lo) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };
        let hi = match (self.hi, other.hi) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };
        Interval { lo, hi }
    }

    pub fn is_empty(&self) -> bool {
        match (self.lo, self.hi) {
            (Some(lo), Some(hi)) => lo > hi,
            _ => false,
        }
    }

    pub fn definitely_nonneg(&self) -> bool {
        self.lo.is_some_and(|lo| lo >= 0)
    }

    pub fn definitely_lt(&self, bound: i64) -> bool {
        self.hi.is_some_and(|hi| hi < bound)
    }

    pub fn definitely_le(&self, bound: i64) -> bool {
        self.hi.is_some_and(|hi| hi <= bound)
    }

    pub fn definitely_gt(&self, bound: i64) -> bool {
        self.lo.is_some_and(|lo| lo > bound)
    }

    pub fn definitely_ge(&self, bound: i64) -> bool {
        self.lo.is_some_and(|lo| lo >= bound)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicState {
    pub block: BasicBlockId,
    pub stmt_idx: usize,
    pub env: HashMap<Place, SymTermId>,
    pub memory_version: MemoryVersionId,
    pub path_constraints: PathConstraintStore,
    pub symbolic_heap: HashMap<SymTermId, SymTermId>,
    pub next_heap_id: usize,
    pub refinements: HashMap<SymTermId, Interval>,
}

impl SymbolicState {
    pub fn new(block: BasicBlockId, memory_version: MemoryVersionId) -> Self {
        SymbolicState {
            block,
            stmt_idx: 0,
            env: HashMap::new(),
            memory_version,
            path_constraints: PathConstraintStore::new(),
            symbolic_heap: HashMap::new(),
            next_heap_id: 0,
            refinements: HashMap::new(),
        }
    }

    pub fn get_value(&self, place: &Place) -> Option<SymTermId> {
        self.env.get(place).copied()
    }

    pub fn set_value(&mut self, place: Place, term: SymTermId) {
        self.env.insert(place, term);
    }

    pub fn refine(&mut self, term: SymTermId, iv: Interval) {
        let leader = self.path_constraints.find_leader(term);
        let updated = {
            let entry = self.refinements.entry(leader).or_insert(Interval::FULL);
            *entry = entry.meet(&iv);
            entry.clone()
        };
        if leader != term {
            self.refinements.insert(term, updated);
        }
    }

    pub fn get_refinement(&self, term: SymTermId) -> Interval {
        let leader = self.path_constraints.find_leader(term);
        if let Some(iv) = self.refinements.get(&leader) {
            return iv.clone();
        }
        self.refinements
            .get(&term)
            .cloned()
            .unwrap_or(Interval::FULL)
    }
}
