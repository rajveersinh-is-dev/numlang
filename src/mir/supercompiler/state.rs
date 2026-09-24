use std::collections::HashMap;

use super::term::{SymTerm, SymTermId, TermInterner};
use crate::ast::BinaryOp;
use crate::mir::memory_ssa::MemoryVersionId;
use crate::mir::{BasicBlockId, Place};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PathConstraintStore {
    /// Variables known to equal specific interned constant terms
    pub const_equalities: HashMap<SymTermId, SymTermId>,
    /// Equivalences between symbolic terms (union-find/leader mapping)
    pub term_equivalences: HashMap<SymTermId, SymTermId>,
    /// Lower and upper integer bounds for symbolic terms
    pub integer_bounds: HashMap<SymTermId, (Option<i64>, Option<i64>)>,
}

impl PathConstraintStore {
    pub fn new() -> Self {
        PathConstraintStore {
            const_equalities: HashMap::new(),
            term_equivalences: HashMap::new(),
            integer_bounds: HashMap::new(),
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
                let (l_min, l_max) = self.integer_bounds.get(&left_lead).copied().unwrap_or((None, None));
                let (r_min, r_max) = self.integer_bounds.get(&right_lead).copied().unwrap_or((None, None));

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicState {
    pub block: BasicBlockId,
    pub stmt_idx: usize,
    pub env: HashMap<Place, SymTermId>,
    pub memory_version: MemoryVersionId,
    pub path_constraints: PathConstraintStore,
}

impl SymbolicState {
    pub fn new(block: BasicBlockId, memory_version: MemoryVersionId) -> Self {
        SymbolicState {
            block,
            stmt_idx: 0,
            env: HashMap::new(),
            memory_version,
            path_constraints: PathConstraintStore::new(),
        }
    }

    pub fn get_value(&self, place: &Place) -> Option<SymTermId> {
        self.env.get(place).copied()
    }

    pub fn set_value(&mut self, place: Place, term: SymTermId) {
        self.env.insert(place, term);
    }
}
