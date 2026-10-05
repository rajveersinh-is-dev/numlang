use super::state::SymbolicState;
use super::term::{SymTerm, SymTermId, TermInterner};
use crate::mir::Place;

/// Fast homeomorphic embedding check: returns true if `t1` is homeomorphically embedded in `t2` (t1 ⊴ t2).
pub fn is_embedded(t1: SymTermId, t2: SymTermId, interner: &TermInterner) -> bool {
    // 0. O(1) identity check via canonical hash-consing
    if t1 == t2 {
        return true;
    }

    let s1 = interner.size(t1);
    let s2 = interner.size(t2);
    // Fast size filter: a larger term cannot be embedded in a smaller term!
    if s1 > s2 {
        return false;
    }

    let d1 = interner.depth(t1);
    let d2 = interner.depth(t2);
    // Fast depth filter: a deeper term cannot be embedded in a shallower term!
    if d1 > d2 {
        return false;
    }

    let term1 = interner.get(t1);
    let term2 = interner.get(t2);

    // 1. Diving test: does t1 embed in any child of t2?
    // Pruning: all children of t2 have strictly smaller size (< s2) and depth (< d2).
    // If s1 == s2 or d1 == d2, t1 cannot possibly embed in any child of t2.
    if s1 < s2 && d1 < d2 {
        let embedded_in_child = match term2 {
            SymTerm::Binary(_, l2, r2, _) => {
                (interner.size(*l2) >= s1 && interner.depth(*l2) >= d1 && is_embedded(t1, *l2, interner))
                    || (interner.size(*r2) >= s1 && interner.depth(*r2) >= d1 && is_embedded(t1, *r2, interner))
            }
            SymTerm::Unary(_, inner2, _) => {
                interner.size(*inner2) >= s1 && interner.depth(*inner2) >= d1 && is_embedded(t1, *inner2, interner)
            }
            SymTerm::Constructor(_, _, fields2, _) => {
                fields2.iter().any(|&f| interner.size(f) >= s1 && interner.depth(f) >= d1 && is_embedded(t1, f, interner))
            }
            SymTerm::Call(_, args2, _) => {
                args2.iter().any(|&a| interner.size(a) >= s1 && interner.depth(a) >= d1 && is_embedded(t1, a, interner))
            }
            SymTerm::Select(c2, th2, el2, _) => {
                (interner.size(*c2) >= s1 && interner.depth(*c2) >= d1 && is_embedded(t1, *c2, interner))
                    || (interner.size(*th2) >= s1 && interner.depth(*th2) >= d1 && is_embedded(t1, *th2, interner))
                    || (interner.size(*el2) >= s1 && interner.depth(*el2) >= d1 && is_embedded(t1, *el2, interner))
            }
            SymTerm::Phi(incoming2, _) => incoming2.iter().any(|(_, t)| {
                interner.size(*t) >= s1 && interner.depth(*t) >= d1 && is_embedded(t1, *t, interner)
            }),
            SymTerm::Ref(inner2, _) | SymTerm::Deref(inner2, _) | SymTerm::Discriminant(inner2, _) => {
                interner.size(*inner2) >= s1 && interner.depth(*inner2) >= d1 && is_embedded(t1, *inner2, interner)
            }
            SymTerm::ClosureVal(_, captured2, _) | SymTerm::Thunk(_, captured2, _) => {
                captured2.iter().any(|&c| interner.size(c) >= s1 && interner.depth(c) >= d1 && is_embedded(t1, c, interner))
            }
            _ => false,
        };
        if embedded_in_child {
            return true;
        }
    }

    // 2. Coupling test: do t1 and t2 share the same constructor and pairwise embed?
    match (term1, term2) {
        (SymTerm::Ref(i1, _), SymTerm::Ref(i2, _)) => is_embedded(*i1, *i2, interner),
        (SymTerm::Deref(p1, _), SymTerm::Deref(p2, _)) => is_embedded(*p1, *p2, interner),
        (SymTerm::Discriminant(i1, _), SymTerm::Discriminant(i2, _)) => is_embedded(*i1, *i2, interner),
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
        (SymTerm::ClosureVal(fn1, c1, _), SymTerm::ClosureVal(fn2, c2, _)) => {
            fn1 == fn2
                && c1.len() == c2.len()
                && c1
                    .iter()
                    .zip(c2.iter())
                    .all(|(&a, &b)| is_embedded(a, b, interner))
        }
        (SymTerm::Thunk(b1, c1, _), SymTerm::Thunk(b2, c2, _)) => {
            b1 == b2
                && c1.len() == c2.len()
                && c1
                    .iter()
                    .zip(c2.iter())
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
            // Fast O(1) identity check via canonical hash-consing:
            // if term IDs are identical, t_anc trivially embeds in t_curr!
            if t_anc == t_curr {
                if let SymTerm::ConstInt(_, _) = interner.get(t_anc) {
                    continue;
                }
                any_embedded = true;
                continue;
            }

            // Concrete constant values represent finite loop unrolling progress, not unbounded symbolic expression growth.
            if let (SymTerm::ConstInt(_, _), SymTerm::ConstInt(_, _)) =
                (interner.get(t_anc), interner.get(t_curr))
            {
                continue;
            }

            // Fast O(1) size and depth filters:
            if interner.size(t_anc) > interner.size(t_curr)
                || interner.depth(t_anc) > interner.depth(t_curr)
            {
                return false;
            }

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
