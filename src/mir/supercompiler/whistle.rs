use super::state::SymbolicState;
use super::term::{SymTerm, SymTermId, TermInterner};
use crate::mir::Place;

/// Fast homeomorphic embedding check: returns true if `t1` is homeomorphically embedded in `t2` (t1 ⊴ t2).
pub fn is_embedded(t1: SymTermId, t2: SymTermId, interner: &TermInterner) -> bool {
    if t1 == t2 {
        return true;
    }

    // Fast size filter: a larger term cannot be embedded in a smaller term!
    if interner.size(t1) > interner.size(t2) {
        return false;
    }

    let term1 = interner.get(t1);
    let term2 = interner.get(t2);

    // 1. Diving test: does t1 embed in any child of t2?
    let embedded_in_child = match term2 {
        SymTerm::Binary(_, l2, r2, _) => {
            is_embedded(t1, *l2, interner) || is_embedded(t1, *r2, interner)
        }
        SymTerm::Unary(_, inner2, _) => is_embedded(t1, *inner2, interner),
        SymTerm::Constructor(_, _, fields2, _) => {
            fields2.iter().any(|&f| is_embedded(t1, f, interner))
        }
        SymTerm::Call(_, args2, _) => {
            args2.iter().any(|&a| is_embedded(t1, a, interner))
        }
        SymTerm::Select(c2, th2, el2, _) => {
            is_embedded(t1, *c2, interner)
                || is_embedded(t1, *th2, interner)
                || is_embedded(t1, *el2, interner)
        }
        SymTerm::Phi(incoming2, _) => incoming2.iter().any(|(_, t)| is_embedded(t1, *t, interner)),
        _ => false,
    };
    if embedded_in_child {
        return true;
    }

    // 2. Coupling test: do t1 and t2 share the same constructor and pairwise embed?
    match (term1, term2) {
        (SymTerm::Binary(op1, l1, r1, _), SymTerm::Binary(op2, l2, r2, _)) => {
            op1 == op2
                && is_embedded(*l1, *l2, interner)
                && is_embedded(*r1, *r2, interner)
        }
        (SymTerm::Unary(op1, in1, _), SymTerm::Unary(op2, in2, _)) => {
            op1 == op2 && is_embedded(*in1, *in2, interner)
        }
        (SymTerm::Constructor(n1, t1, f1, _), SymTerm::Constructor(n2, t2, f2, _)) => {
            n1 == n2
                && t1 == t2
                && f1.len() == f2.len()
                && f1
                    .iter()
                    .zip(f2.iter())
                    .all(|(&a, &b)| is_embedded(a, b, interner))
        }
        (SymTerm::Call(c1, a1, _), SymTerm::Call(c2, a2, _)) => {
            c1 == c2
                && a1.len() == a2.len()
                && a1
                    .iter()
                    .zip(a2.iter())
                    .all(|(&a, &b)| is_embedded(a, b, interner))
        }
        (SymTerm::Select(c1, th1, el1, _), SymTerm::Select(c2, th2, el2, _)) => {
            is_embedded(*c1, *c2, interner)
                && is_embedded(*th1, *th2, interner)
                && is_embedded(*el1, *el2, interner)
        }
        _ => false,
    }
}

/// Checks if descendant state `curr` embeds ancestor state `anc` at the same program point.
pub fn state_embeds(
    anc: &SymbolicState,
    curr: &SymbolicState,
    active_places: &[Place],
    interner: &TermInterner,
) -> bool {
    if anc.block != curr.block {
        return false;
    }

    // If all active places in anc embed into curr, the whistle blows!
    let mut any_embedded = false;
    for place in active_places {
        if let (Some(t_anc), Some(t_curr)) = (anc.get_value(place), curr.get_value(place)) {
            if is_embedded(t_anc, t_curr, interner) {
                any_embedded = true;
            } else {
                return false;
            }
        }
    }

    any_embedded
}

/// Checks if `curr` is an exact alpha-equivalent instance of `anc` (candidate for immediate knot-tying).
pub fn is_instance_of(
    anc: &SymbolicState,
    curr: &SymbolicState,
    active_places: &[Place],
) -> bool {
    if anc.block != curr.block {
        return false;
    }

    for place in active_places {
        if anc.get_value(place) != curr.get_value(place) {
            return false;
        }
    }

    true
}
