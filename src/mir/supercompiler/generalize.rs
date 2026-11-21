use std::collections::HashMap;

use super::term::{SymTerm, SymTermId, TermInterner};
use crate::ast::BinaryOp;
use crate::mir::Place;
use crate::typecheck::types::Type;

#[derive(Debug, Clone)]
pub struct GeneralizationResult {
    pub common_term: SymTermId,
    pub var_mappings: Vec<(Place, SymTermId, SymTermId)>, // (fresh_var, t1_val, t2_val)
}

/// Computes the Most Specific Generalization (MSG) between two terms.
pub fn most_specific_generalization(
    t1: SymTermId,
    t2: SymTermId,
    interner: &mut TermInterner,
    next_var_id: &mut usize,
) -> GeneralizationResult {
    let mut memo: HashMap<(SymTermId, SymTermId), SymTermId> = HashMap::new();
    let mut mappings = Vec::new();

    let common = msg_helper(t1, t2, interner, next_var_id, &mut memo, &mut mappings);
    GeneralizationResult {
        common_term: common,
        var_mappings: mappings,
    }
}

/// Textbook first-order anti-unification (Sørensen & Glück 1995; Plotkin 1970).
pub fn anti_unify(
    t1: SymTermId,
    t2: SymTermId,
    interner: &mut TermInterner,
    subst1: &mut HashMap<String, SymTermId>,
    subst2: &mut HashMap<String, SymTermId>,
) -> SymTermId {
    let mut next_var_id = subst1.len();
    let res = most_specific_generalization(t1, t2, interner, &mut next_var_id);
    for (place, v1, v2) in res.var_mappings {
        subst1.insert(place.local.clone(), v1);
        subst2.insert(place.local, v2);
    }
    res.common_term
}

fn msg_helper(
    t1: SymTermId,
    t2: SymTermId,
    interner: &mut TermInterner,
    next_var_id: &mut usize,
    memo: &mut HashMap<(SymTermId, SymTermId), SymTermId>,
    mappings: &mut Vec<(Place, SymTermId, SymTermId)>,
) -> SymTermId {
    if t1 == t2 {
        return t1;
    }

    if let Some(&cached) = memo.get(&(t1, t2)) {
        return cached;
    }

    let term1 = interner.get(t1).clone();
    let term2 = interner.get(t2).clone();

    let result = match (&term1, &term2) {
        (SymTerm::Binary(op1, l1, r1, ty1), SymTerm::Binary(op2, l2, r2, _)) if op1 == op2 => {
            let common_l = msg_helper(*l1, *l2, interner, next_var_id, memo, mappings);
            let common_r = msg_helper(*r1, *r2, interner, next_var_id, memo, mappings);
            interner.intern_binary(*op1, common_l, common_r, ty1.clone())
        }
        (SymTerm::Unary(op1, in1, ty1), SymTerm::Unary(op2, in2, _)) if op1 == op2 => {
            let common_in = msg_helper(*in1, *in2, interner, next_var_id, memo, mappings);
            interner.intern_unary(*op1, common_in, ty1.clone())
        }
        (SymTerm::Constructor(n1, t1, f1, ty1), SymTerm::Constructor(n2, t2, f2, _))
            if n1 == n2 && t1 == t2 && f1.len() == f2.len() =>
        {
            let mut common_f = Vec::with_capacity(f1.len());
            for (&a, &b) in f1.iter().zip(f2.iter()) {
                common_f.push(msg_helper(a, b, interner, next_var_id, memo, mappings));
            }
            interner.intern_constructor(n1.clone(), *t1, common_f, ty1.clone())
        }
        (SymTerm::Call(c1, a1, ty1), SymTerm::Call(c2, a2, _))
            if c1 == c2 && a1.len() == a2.len() =>
        {
            let mut common_a = Vec::with_capacity(a1.len());
            for (&a, &b) in a1.iter().zip(a2.iter()) {
                common_a.push(msg_helper(a, b, interner, next_var_id, memo, mappings));
            }
            interner.intern_call(c1.clone(), common_a, ty1.clone())
        }
        (SymTerm::ClosureVal(fn1, c1, ty1), SymTerm::ClosureVal(fn2, c2, _))
            if fn1 == fn2 && c1.len() == c2.len() =>
        {
            let mut common_c = Vec::with_capacity(c1.len());
            for (&a, &b) in c1.iter().zip(c2.iter()) {
                common_c.push(msg_helper(a, b, interner, next_var_id, memo, mappings));
            }
            interner.intern_closure_val(fn1.clone(), common_c, ty1.clone())
        }
        (SymTerm::Thunk(b1, c1, ty1), SymTerm::Thunk(b2, c2, _))
            if b1 == b2 && c1.len() == c2.len() =>
        {
            let mut common_c = Vec::with_capacity(c1.len());
            for (&a, &b) in c1.iter().zip(c2.iter()) {
                common_c.push(msg_helper(a, b, interner, next_var_id, memo, mappings));
            }
            interner.intern_thunk(b1.clone(), common_c, ty1.clone())
        }
        (SymTerm::Ref(i1, ty1), SymTerm::Ref(i2, _)) => {
            let common = msg_helper(*i1, *i2, interner, next_var_id, memo, mappings);
            interner.intern_ref(common, ty1.clone())
        }
        (SymTerm::Deref(p1, ty1), SymTerm::Deref(p2, _)) => {
            let common = msg_helper(*p1, *p2, interner, next_var_id, memo, mappings);
            interner.intern_deref(common, ty1.clone())
        }
        (SymTerm::Discriminant(i1, _), SymTerm::Discriminant(i2, _)) => {
            let common = msg_helper(*i1, *i2, interner, next_var_id, memo, mappings);
            interner.intern_discriminant(common)
        }
        (SymTerm::Select(c1, t1, e1, ty1), SymTerm::Select(c2, t2, e2, _)) => {
            let common_c = msg_helper(*c1, *c2, interner, next_var_id, memo, mappings);
            let common_t = msg_helper(*t1, *t2, interner, next_var_id, memo, mappings);
            let common_e = msg_helper(*e1, *e2, interner, next_var_id, memo, mappings);
            interner.intern_select(common_c, common_t, common_e, ty1.clone())
        }
        _ => {
            // Generalize to fresh variable
            let var_name = format!("_gen_{}", *next_var_id);
            *next_var_id += 1;
            let ty = term1.ty().clone();
            let place = Place {
                local: var_name,
                projections: vec![],
            };
            let var_id = interner.intern_var(place.clone(), ty);
            mappings.push((place, t1, t2));
            var_id
        }
    };

    memo.insert((t1, t2), result);
    result
}

/// Constructs the triangular term (k * (k - 1) / 2) * coeff without intermediate overflow.
/// If coeff is even, factors (coeff / 2) to eliminate division completely: k * (k - 1) * (coeff / 2).
/// If coeff is odd, checks parity of k so the division by 2 is performed on the even factor
/// before multiplication: (k / 2) * (k - 1) when k is even, or k * ((k - 1) / 2) when k is odd.
pub fn build_triangular_term(interner: &mut TermInterner, k: SymTermId, coeff: i64) -> SymTermId {
    let one_term = interner.intern_int(1);
    let two_term = interner.intern_int(2);
    let k_minus_1 = interner.intern_binary(BinaryOp::Sub, k, one_term, Type::I64);

    if coeff % 2 == 0 {
        let half_coeff = interner.intern_int(coeff / 2);
        let k_times_k1 = interner.intern_binary(BinaryOp::Mul, k, k_minus_1, Type::I64);
        interner.intern_binary(BinaryOp::Mul, k_times_k1, half_coeff, Type::I64)
    } else {
        let zero_term = interner.intern_int(0);
        let bit_and = interner.intern_binary(BinaryOp::BitAnd, k, one_term, Type::I64);
        let is_even = interner.intern_binary(BinaryOp::Eq, bit_and, zero_term, Type::I64);

        let k_div_2 = interner.intern_binary(BinaryOp::Div, k, two_term, Type::I64);
        let even_tri = interner.intern_binary(BinaryOp::Mul, k_div_2, k_minus_1, Type::I64);

        let k1_div_2 = interner.intern_binary(BinaryOp::Div, k_minus_1, two_term, Type::I64);
        let odd_tri = interner.intern_binary(BinaryOp::Mul, k, k1_div_2, Type::I64);

        let tri_part = interner.intern_select(is_even, even_tri, odd_tri, Type::I64);
        if coeff == 1 {
            tri_part
        } else {
            let c_term = interner.intern_int(coeff);
            interner.intern_binary(BinaryOp::Mul, tri_part, c_term, Type::I64)
        }
    }
}

/// Greatest common divisor of two unsigned integers.
fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

/// Parity test of the iteration variable: true iff k is even.
fn intern_is_even(interner: &mut TermInterner, k: SymTermId) -> SymTermId {
    let one = interner.intern_int(1);
    let zero = interner.intern_int(0);
    let bit_and = interner.intern_binary(BinaryOp::BitAnd, k, one, Type::I64);
    interner.intern_binary(BinaryOp::Eq, bit_and, zero, Type::I64)
}

/// Residue test of the iteration variable: true iff k % m == r (for k >= 0).
fn intern_mod_eq(interner: &mut TermInterner, k: SymTermId, m: i64, r: i64) -> SymTermId {
    let m_term = interner.intern_int(m);
    let r_term = interner.intern_int(r);
    let rem = interner.intern_binary(BinaryOp::Mod, k, m_term, Type::I64);
    interner.intern_binary(BinaryOp::Eq, rem, r_term, Type::I64)
}

/// Constructs `C(k, 3) * coeff = k*(k-1)*(k-2)/6 * coeff` without ever dividing
/// a value that may already have wrapped (LANGUAGE.md §9).
///
/// The divisor 6 = 2 * 3 is distributed into the individual consecutive factors
/// *before* any multiplication: the factor selected by `k mod 3` is divided by 3
/// (exact), and the even factor selected by the parity of `k` is halved (exact).
/// `g = gcd(|coeff|, 6)` is folded into the coefficient at compile time.
/// When `6 | coeff` (e.g. the common `d3 = 6`) no runtime division is emitted
/// at all, preserving the historical closed-form shape exactly.
fn build_cubic_term(interner: &mut TermInterner, k: SymTermId, coeff: i64) -> SymTermId {
    let fold = gcd_u64(coeff.unsigned_abs(), 6) as i64;
    let folded_coeff = coeff / fold;
    let divisor = 6 / fold;

    let one = interner.intern_int(1);
    let two = interner.intern_int(2);
    let k_minus_1 = interner.intern_binary(BinaryOp::Sub, k, one, Type::I64);
    let k_minus_2 = interner.intern_binary(BinaryOp::Sub, k, two, Type::I64);
    let folded_term = interner.intern_int(folded_coeff);

    let product = if divisor == 1 {
        // 6 | coeff: fold the whole divisor into the coefficient (no division).
        let k_times_k1 = interner.intern_binary(BinaryOp::Mul, k, k_minus_1, Type::I64);
        interner.intern_binary(BinaryOp::Mul, k_times_k1, k_minus_2, Type::I64)
    } else if divisor == 2 {
        // 3 | coeff, 2 ∤ coeff: fold 3 and distribute only the halving, using
        // the same parity-select shape as build_triangular_term (k, 1).
        let tri = build_triangular_term(interner, k, 1);
        interner.intern_binary(BinaryOp::Mul, tri, k_minus_2, Type::I64)
    } else {
        // divisor ∈ {3, 6}: distribute /3 over the factor selected by `k mod 3`,
        // and (for divisor = 6) /2 over an even factor selected by parity.
        let three = interner.intern_int(3);
        let (fa, fb, fc) = if divisor % 3 == 0 {
            let t0 = interner.intern_binary(BinaryOp::Div, k, three, Type::I64);
            let t1 = interner.intern_binary(BinaryOp::Div, k_minus_1, three, Type::I64);
            let t2 = interner.intern_binary(BinaryOp::Div, k_minus_2, three, Type::I64);
            let c0 = intern_mod_eq(interner, k, 3, 0);
            let c1 = intern_mod_eq(interner, k, 3, 1);
            let c2 = intern_mod_eq(interner, k, 3, 2);
            let fa = interner.intern_select(c0, t0, k, Type::I64);
            let fb = interner.intern_select(c1, t1, k_minus_1, Type::I64);
            let fc = interner.intern_select(c2, t2, k_minus_2, Type::I64);
            (fa, fb, fc)
        } else {
            (k, k_minus_1, k_minus_2)
        };

        if divisor % 2 == 0 {
            // divisor = 6: halve an even factor chosen by the parity of k.
            let is_even = intern_is_even(interner, k);
            let fa_div_2 = interner.intern_binary(BinaryOp::Div, fa, two, Type::I64);
            let fb_div_2 = interner.intern_binary(BinaryOp::Div, fb, two, Type::I64);
            let even_leaf = interner.intern_binary(BinaryOp::Mul, fa_div_2, fb, Type::I64);
            let even_leaf = interner.intern_binary(BinaryOp::Mul, even_leaf, fc, Type::I64);
            let odd_leaf = interner.intern_binary(BinaryOp::Mul, fa, fb_div_2, Type::I64);
            let odd_leaf = interner.intern_binary(BinaryOp::Mul, odd_leaf, fc, Type::I64);
            interner.intern_select(is_even, even_leaf, odd_leaf, Type::I64)
        } else {
            let p = interner.intern_binary(BinaryOp::Mul, fa, fb, Type::I64);
            interner.intern_binary(BinaryOp::Mul, p, fc, Type::I64)
        }
    };

    interner.intern_binary(BinaryOp::Mul, product, folded_term, Type::I64)
}

/// Constructs `C(k, 4) * coeff = k*(k-1)*(k-2)*(k-3)/24 * coeff` without ever
/// dividing a value that may already have wrapped (LANGUAGE.md §9).
///
/// The divisor 24 = 2^3 * 3 is distributed into the individual consecutive
/// factors *before* any multiplication. `g = gcd(|coeff|, 24)` is folded into
/// the coefficient at compile time. When `24 | coeff` (e.g. the common `d4 = 24`)
/// no runtime division is emitted at all, preserving the historical closed-form
/// shape exactly.
fn build_quartic_term(interner: &mut TermInterner, k: SymTermId, coeff: i64) -> SymTermId {
    let fold = gcd_u64(coeff.unsigned_abs(), 24) as i64;
    let folded_coeff = coeff / fold;
    let divisor = 24 / fold;

    let one = interner.intern_int(1);
    let two = interner.intern_int(2);
    let three = interner.intern_int(3);
    let four = interner.intern_int(4);
    let k_minus_1 = interner.intern_binary(BinaryOp::Sub, k, one, Type::I64);
    let k_minus_2 = interner.intern_binary(BinaryOp::Sub, k, two, Type::I64);
    let k_minus_3 = interner.intern_binary(BinaryOp::Sub, k, three, Type::I64);
    let folded_term = interner.intern_int(folded_coeff);

    if divisor == 1 {
        // 24 | coeff: fold the whole divisor into the coefficient (no division).
        let p1 = interner.intern_binary(BinaryOp::Mul, k, k_minus_1, Type::I64);
        let p2 = interner.intern_binary(BinaryOp::Mul, p1, k_minus_2, Type::I64);
        let p3 = interner.intern_binary(BinaryOp::Mul, p2, k_minus_3, Type::I64);
        return interner.intern_binary(BinaryOp::Mul, p3, folded_term, Type::I64);
    }

    // Stage 1: distribute /3 into the factor selected by `k mod 3`.
    // (d = k - 3 is never chosen; it is congruent to k mod 3.)
    let (fa, fb, fc, fd) = if divisor % 3 == 0 {
        let c0 = intern_mod_eq(interner, k, 3, 0);
        let c1 = intern_mod_eq(interner, k, 3, 1);
        let c2 = intern_mod_eq(interner, k, 3, 2);
        let t0 = interner.intern_binary(BinaryOp::Div, k, three, Type::I64);
        let t1 = interner.intern_binary(BinaryOp::Div, k_minus_1, three, Type::I64);
        let t2 = interner.intern_binary(BinaryOp::Div, k_minus_2, three, Type::I64);
        let fa = interner.intern_select(c0, t0, k, Type::I64);
        let fb = interner.intern_select(c1, t1, k_minus_1, Type::I64);
        let fc = interner.intern_select(c2, t2, k_minus_2, Type::I64);
        (fa, fb, fc, k_minus_3)
    } else {
        (k, k_minus_1, k_minus_2, k_minus_3)
    };

    // Stage 2: distribute 2^p over the even factors.
    let pow2 = divisor.trailing_zeros() as i32;

    let product = match pow2 {
        0 => {
            let p1 = interner.intern_binary(BinaryOp::Mul, fa, fb, Type::I64);
            let p2 = interner.intern_binary(BinaryOp::Mul, p1, fc, Type::I64);
            interner.intern_binary(BinaryOp::Mul, p2, fd, Type::I64)
        }
        1 => {
            // Halve a single even factor chosen by the parity of k.
            let is_even = intern_is_even(interner, k);
            let fa_div_2 = interner.intern_binary(BinaryOp::Div, fa, two, Type::I64);
            let fb_div_2 = interner.intern_binary(BinaryOp::Div, fb, two, Type::I64);
            let even_leaf = interner.intern_binary(BinaryOp::Mul, fa_div_2, fb, Type::I64);
            let even_leaf = interner.intern_binary(BinaryOp::Mul, even_leaf, fc, Type::I64);
            let even_leaf = interner.intern_binary(BinaryOp::Mul, even_leaf, fd, Type::I64);
            let odd_leaf = interner.intern_binary(BinaryOp::Mul, fa, fb_div_2, Type::I64);
            let odd_leaf = interner.intern_binary(BinaryOp::Mul, odd_leaf, fc, Type::I64);
            let odd_leaf = interner.intern_binary(BinaryOp::Mul, odd_leaf, fd, Type::I64);
            interner.intern_select(is_even, even_leaf, odd_leaf, Type::I64)
        }
        2 => {
            // Halve both even factors of the appropriate parity.
            let is_even = intern_is_even(interner, k);
            let fa_div_2 = interner.intern_binary(BinaryOp::Div, fa, two, Type::I64);
            let fb_div_2 = interner.intern_binary(BinaryOp::Div, fb, two, Type::I64);
            let fc_div_2 = interner.intern_binary(BinaryOp::Div, fc, two, Type::I64);
            let fd_div_2 = interner.intern_binary(BinaryOp::Div, fd, two, Type::I64);
            let even_leaf = interner.intern_binary(BinaryOp::Mul, fa_div_2, fb, Type::I64);
            let even_leaf = interner.intern_binary(BinaryOp::Mul, even_leaf, fc_div_2, Type::I64);
            let even_leaf = interner.intern_binary(BinaryOp::Mul, even_leaf, fd, Type::I64);
            let odd_leaf = interner.intern_binary(BinaryOp::Mul, fa, fb_div_2, Type::I64);
            let odd_leaf = interner.intern_binary(BinaryOp::Mul, odd_leaf, fc, Type::I64);
            let odd_leaf = interner.intern_binary(BinaryOp::Mul, odd_leaf, fd_div_2, Type::I64);
            interner.intern_select(is_even, even_leaf, odd_leaf, Type::I64)
        }
        _ => {
            // p == 3: /8 = /4 on the factor divisible by 4 + /2 on the other even.
            let c0 = intern_mod_eq(interner, k, 4, 0);
            let c1 = intern_mod_eq(interner, k, 4, 1);
            let c2 = intern_mod_eq(interner, k, 4, 2);
            let fa_div_4 = interner.intern_binary(BinaryOp::Div, fa, four, Type::I64);
            let fb_div_4 = interner.intern_binary(BinaryOp::Div, fb, four, Type::I64);
            let fc_div_4 = interner.intern_binary(BinaryOp::Div, fc, four, Type::I64);
            let fd_div_4 = interner.intern_binary(BinaryOp::Div, fd, four, Type::I64);
            let fa_div_2 = interner.intern_binary(BinaryOp::Div, fa, two, Type::I64);
            let fb_div_2 = interner.intern_binary(BinaryOp::Div, fb, two, Type::I64);
            let fc_div_2 = interner.intern_binary(BinaryOp::Div, fc, two, Type::I64);
            let fd_div_2 = interner.intern_binary(BinaryOp::Div, fd, two, Type::I64);
            // k ≡ 0: a/4, c/2 ; k ≡ 1: b/4, d/2 ; k ≡ 2: a/2, c/4 ; k ≡ 3: b/2, d/4
            let leaf0 = interner.intern_binary(BinaryOp::Mul, fa_div_4, fb, Type::I64);
            let leaf0 = interner.intern_binary(BinaryOp::Mul, leaf0, fc_div_2, Type::I64);
            let leaf0 = interner.intern_binary(BinaryOp::Mul, leaf0, fd, Type::I64);
            let leaf1 = interner.intern_binary(BinaryOp::Mul, fa, fb_div_4, Type::I64);
            let leaf1 = interner.intern_binary(BinaryOp::Mul, leaf1, fc, Type::I64);
            let leaf1 = interner.intern_binary(BinaryOp::Mul, leaf1, fd_div_2, Type::I64);
            let leaf2 = interner.intern_binary(BinaryOp::Mul, fa_div_2, fb, Type::I64);
            let leaf2 = interner.intern_binary(BinaryOp::Mul, leaf2, fc_div_4, Type::I64);
            let leaf2 = interner.intern_binary(BinaryOp::Mul, leaf2, fd, Type::I64);
            let leaf3 = interner.intern_binary(BinaryOp::Mul, fa, fb_div_2, Type::I64);
            let leaf3 = interner.intern_binary(BinaryOp::Mul, leaf3, fc, Type::I64);
            let leaf3 = interner.intern_binary(BinaryOp::Mul, leaf3, fd_div_4, Type::I64);
            let inner = interner.intern_select(c2, leaf2, leaf3, Type::I64);
            let mid = interner.intern_select(c1, leaf1, inner, Type::I64);
            interner.intern_select(c0, leaf0, mid, Type::I64)
        }
    };

    interner.intern_binary(BinaryOp::Mul, product, folded_term, Type::I64)
}

/// True iff the `order`-th forward differences of `samples` are constant across
/// every available sample (not just the first two). Prevent shortcuts like
/// `0,0,0,1,4,10` being mistaken for a degree-1 (constant) sequence.
fn differences_constant(samples: &[i64], order: usize) -> bool {
    let mut cur: Vec<i64> = samples.to_vec();
    for _ in 0..order {
        if cur.len() < 2 {
            return false;
        }
        cur = cur.windows(2).map(|w| w[1].wrapping_sub(w[0])).collect();
    }
    cur.iter().all(|v| *v == cur[0])
}

/// Solves closed forms for numerical recurrence sequences:
/// Given simulated initial states [s0, s1, s2, s3], detects polynomial degree
/// and returns closed form in terms of `num_iters`.
pub fn solve_recurrence(
    samples: &[i64],
    num_iters: SymTermId,
    interner: &mut TermInterner,
) -> Option<SymTermId> {
    if samples.len() < 3 {
        return None;
    }

    let s0 = samples[0];
    let s1 = samples[1];
    let s2 = samples[2];

    // First differences
    let d1_0 = s1.wrapping_sub(s0);
    let d1_1 = s2.wrapping_sub(s1);

    // Degree 1: Linear sequence s_k = s0 + k * d1
    if differences_constant(samples, 1) {
        // s0 + N * d1
        let s0_term = interner.intern_int(s0);
        let d1_term = interner.intern_int(d1_0);
        let n_times_d1 = interner.intern_binary(BinaryOp::Mul, num_iters, d1_term, Type::I64);
        return Some(interner.intern_binary(BinaryOp::Add, s0_term, n_times_d1, Type::I64));
    }

    // Degree 2: Quadratic / Triangular sequence
    if samples.len() >= 4 {
        let s3 = samples[3];
        let _d1_2 = s3.wrapping_sub(s2);
        let d2_0 = d1_1.wrapping_sub(d1_0);

        if differences_constant(samples, 2) {
            // Formula: s_k = s0 + k * d1_0 + (k * (k - 1) / 2) * d2_0
            let s0_term = interner.intern_int(s0);
            let d1_term = interner.intern_int(d1_0);

            // k * d1_0
            let linear_part = interner.intern_binary(BinaryOp::Mul, num_iters, d1_term, Type::I64);

            // (k * (k - 1) / 2) * d2_0 computed with parity-halving / factor distribution
            let quad_part = build_triangular_term(interner, num_iters, d2_0);

            // s0 + linear + quad
            let part1 = interner.intern_binary(BinaryOp::Add, s0_term, linear_part, Type::I64);
            return Some(interner.intern_binary(BinaryOp::Add, part1, quad_part, Type::I64));
        }
    }

    // Degree 3: Cubic sequence
    if samples.len() >= 5 {
        let s3 = samples[3];
        let s4 = samples[4];
        let d1_2 = s3.wrapping_sub(s2);
        let d1_3 = s4.wrapping_sub(s3);
        let d2_0 = d1_1.wrapping_sub(d1_0);
        let d2_1 = d1_2.wrapping_sub(d1_1);
        let _d2_2 = d1_3.wrapping_sub(d1_2);
        let d3_0 = d2_1.wrapping_sub(d2_0);

        if differences_constant(samples, 3) {
            let s0_term = interner.intern_int(s0);
            let d1_term = interner.intern_int(d1_0);

            let linear_part = interner.intern_binary(BinaryOp::Mul, num_iters, d1_term, Type::I64);

            let quad_part = build_triangular_term(interner, num_iters, d2_0);

            // Sound cubic closed form: C(k, 3) * d3_0 without dividing a wrapped value.
            let cubic_part = build_cubic_term(interner, num_iters, d3_0);

            let sum1 = interner.intern_binary(BinaryOp::Add, s0_term, linear_part, Type::I64);
            let sum2 = interner.intern_binary(BinaryOp::Add, sum1, quad_part, Type::I64);
            return Some(interner.intern_binary(BinaryOp::Add, sum2, cubic_part, Type::I64));
        }
    }

    // Degree 4: Quartic sequence (e.g. sum of cubes \sum i^3)
    if samples.len() >= 6 {
        let s3 = samples[3];
        let s4 = samples[4];
        let s5 = samples[5];
        let d1_2 = s3.wrapping_sub(s2);
        let d1_3 = s4.wrapping_sub(s3);
        let d1_4 = s5.wrapping_sub(s4);
        let d2_0 = d1_1.wrapping_sub(d1_0);
        let d2_1 = d1_2.wrapping_sub(d1_1);
        let d2_2 = d1_3.wrapping_sub(d1_2);
        let _d2_3 = d1_4.wrapping_sub(d1_3);
        let d3_0 = d2_1.wrapping_sub(d2_0);
        let d3_1 = d2_2.wrapping_sub(d2_1);
        let d4_0 = d3_1.wrapping_sub(d3_0);

        if differences_constant(samples, 4) {
            let s0_term = interner.intern_int(s0);
            let d1_term = interner.intern_int(d1_0);

            let linear_part = interner.intern_binary(BinaryOp::Mul, num_iters, d1_term, Type::I64);

            let quad_part = build_triangular_term(interner, num_iters, d2_0);

            let cubic_part = build_cubic_term(interner, num_iters, d3_0);

            let quartic_part = build_quartic_term(interner, num_iters, d4_0);

            let sum1 = interner.intern_binary(BinaryOp::Add, s0_term, linear_part, Type::I64);
            let sum2 = interner.intern_binary(BinaryOp::Add, sum1, quad_part, Type::I64);
            let sum3 = interner.intern_binary(BinaryOp::Add, sum2, cubic_part, Type::I64);
            return Some(interner.intern_binary(BinaryOp::Add, sum3, quartic_part, Type::I64));
        }
    }

    // Geometric sequence: s_{k+1} = s_k * ratio
    if s0 != 0 && s1 != 0 {
        let ratio = s1 / s0;
        if ratio != 0 && s1 == s0 * ratio && s2 == s1 * ratio {
            // s_k = s0 * ratio^k
            let s0_term = interner.intern_int(s0);
            let ratio_term = interner.intern_int(ratio);
            let pow_term = interner.intern_binary(BinaryOp::Pow, ratio_term, num_iters, Type::I64);
            return Some(interner.intern_binary(BinaryOp::Mul, s0_term, pow_term, Type::I64));
        }
    }

    // Nonlinear recurrence detection (polynomial sums, geometric series, power towers)
    if let Some(nonlin) =
        crate::mir::supercompiler::recurrence::detect_nonlinear_recurrence(samples, interner)
    {
        return Some(
            crate::mir::supercompiler::recurrence::solve_nonlinear_recurrence(
                &nonlin, num_iters, interner,
            ),
        );
    }

    // Linear-mod recurrence: s_k = (s0 + k * C) mod M
    // Appears as constant difference C with periodic wraps by -M.
    if samples.len() >= 6 {
        let diffs: Vec<i64> = samples.windows(2).map(|w| w[1].wrapping_sub(w[0])).collect();
        let mut pos_d = None;
        let mut neg_d = None;
        let mut ok = true;
        for &d in &diffs {
            if d >= 0 {
                if pos_d.is_none() { pos_d = Some(d); }
                else if pos_d != Some(d) { ok = false; break; }
            } else {
                if neg_d.is_none() { neg_d = Some(d); }
                else if neg_d != Some(d) { ok = false; break; }
            }
        }
        if ok {
            if let (Some(c), Some(c_minus_m)) = (pos_d, neg_d) {
                let m = c.wrapping_sub(c_minus_m);
                if m > 0 && c >= 0 && c < m {
                    // Verify all samples
                    let mut valid = true;
                    for (k, &sv) in samples.iter().enumerate() {
                        let expected = ((s0 as i128 + (k as i128) * (c as i128)).rem_euclid(m as i128)) as i64;
                        if sv != expected { valid = false; break; }
                    }
                    if valid {
                        let s0_term = interner.intern_int(s0);
                        let c_term = interner.intern_int(c);
                        let m_term = interner.intern_int(m);
                        let k_times_c = interner.intern_binary(BinaryOp::Mul, num_iters, c_term, Type::I64);
                        let sum = interner.intern_binary(BinaryOp::Add, s0_term, k_times_c, Type::I64);
                        // Use rem_euclid semantics for safety: ((sum % m) + m) % m
                        let rem = interner.intern_binary(BinaryOp::Mod, sum, m_term, Type::I64);
                        let rem_plus_m = interner.intern_binary(BinaryOp::Add, rem, m_term, Type::I64);
                        let result = interner.intern_binary(BinaryOp::Mod, rem_plus_m, m_term, Type::I64);
                        return Some(result);
                    }
                }
            } else if let Some(c) = pos_d {
                // Pure linear with no wrap observed - but differences_constant already handled.
                // Still, if all diffs same, it's linear (handled above). No-op here.
                let _ = c;
            }
        }
    }

    // Order-2 linear recurrence: s_k = c1 * s_{k-1} + c2 * s_{k-2} (Fibonacci, Lucas, coupled systems)
    if let Some(res) = solve_order2_recurrence(samples, num_iters, interner) {
        return Some(res);
    }

    // Order-3 linear recurrence: s_k = c1 * s_{k-1} + c2 * s_{k-2} + c3 * s_{k-3} (Tribonacci)
    if let Some(res) = solve_order3_recurrence(samples, num_iters, interner) {
        return Some(res);
    }

    None
}

/// Solves constant-coefficient linear recurrence of order 2:
/// s_k = c1 * s_{k-1} + c2 * s_{k-2}
pub fn solve_order2_recurrence(
    samples: &[i64],
    num_iters: SymTermId,
    interner: &mut TermInterner,
) -> Option<SymTermId> {
    if samples.len() < 4 {
        return None;
    }
    let s0 = samples[0];
    let s1 = samples[1];
    let s2 = samples[2];
    let s3 = samples[3];

    // s2 = c1 * s1 + c2 * s0
    // s3 = c1 * s2 + c2 * s1
    let det = s1.wrapping_mul(s1).wrapping_sub(s0.wrapping_mul(s2));
    if det == 0 {
        return None;
    }

    let num_c1 = s2.wrapping_mul(s1).wrapping_sub(s3.wrapping_mul(s0));
    let num_c2 = s1.wrapping_mul(s3).wrapping_sub(s2.wrapping_mul(s2));

    if num_c1 % det != 0 || num_c2 % det != 0 {
        return None;
    }

    let c1 = num_c1 / det;
    let c2 = num_c2 / det;

    // Verify against all remaining samples
    for k in 4..samples.len() {
        let expected = c1
            .wrapping_mul(samples[k - 1])
            .wrapping_add(c2.wrapping_mul(samples[k - 2]));
        if samples[k] != expected {
            return None;
        }
    }

    // Case 1: Constant number of iterations -> exact matrix exponentiation
    let iter_term = interner.get(num_iters).clone();
    if let SymTerm::ConstInt(n, _) = iter_term {
        if n < 0 {
            return None;
        }
        let n_idx = n as usize;
        if n_idx < samples.len() {
            return Some(interner.intern_int(samples[n_idx]));
        }
        let m = [[c1, c2], [1, 0]];
        let m_pow = mat_pow_2x2(m, n - 1);
        let s_n = m_pow[0][0]
            .wrapping_mul(s1)
            .wrapping_add(m_pow[0][1].wrapping_mul(s0));
        return Some(interner.intern_int(s_n));
    }

    // Case 2: Integer characteristic roots r^2 - c1*r - c2 = 0
    let disc = c1.wrapping_mul(c1).wrapping_add(4i64.wrapping_mul(c2));
    if disc >= 0 {
        let d = (disc as f64).sqrt().round() as i64;
        if d.checked_mul(d) == Some(disc) && c1.checked_add(d).is_some_and(|sum| sum % 2 == 0) {
            let r1 = c1.wrapping_add(d) / 2;
            let r2 = c1.wrapping_sub(d) / 2;
            if r1 != r2 {
                let num_a = s1.wrapping_sub(s0.wrapping_mul(r2));
                let den_a = r1.wrapping_sub(r2);
                if den_a != 0 && num_a % den_a == 0 {
                    let a = num_a / den_a;
                    let b = s0.wrapping_sub(a);
                    let a_term = interner.intern_int(a);
                    let b_term = interner.intern_int(b);
                    let r1_term = interner.intern_int(r1);
                    let r2_term = interner.intern_int(r2);
                    let pow1 = interner.intern_binary(BinaryOp::Pow, r1_term, num_iters, Type::I64);
                    let pow2 = interner.intern_binary(BinaryOp::Pow, r2_term, num_iters, Type::I64);
                    let part1 = interner.intern_binary(BinaryOp::Mul, a_term, pow1, Type::I64);
                    let part2 = interner.intern_binary(BinaryOp::Mul, b_term, pow2, Type::I64);
                    return Some(interner.intern_binary(BinaryOp::Add, part1, part2, Type::I64));
                }
            }
        }
    }

    // No codegen handler exists for symbolic order-2 recurrence intrinsics.
    // Fall back to the loop instead of emitting an unhandleable call.
    None
}

/// Solves coupled 2-variable linear recurrences:
/// a_{k+1} = p * a_k + q * b_k + c_a
/// b_{k+1} = r * a_k + s * b_k + c_b
pub fn solve_coupled_2var_recurrence(
    samples_a: &[i64],
    samples_b: &[i64],
    num_iters: SymTermId,
    interner: &mut TermInterner,
) -> Option<(SymTermId, SymTermId)> {
    if samples_a.len() < 4 || samples_b.len() < 4 {
        return None;
    }

    let a0 = samples_a[0];
    let a1 = samples_a[1];
    let a2 = samples_a[2];
    let a3 = samples_a[3];

    let b0 = samples_b[0];
    let b1 = samples_b[1];
    let b2 = samples_b[2];
    let b3 = samples_b[3];

    // Method 1: Try homogeneous linear recurrence (c_a = 0, c_b = 0)
    // a1 = p * a0 + q * b0
    // a2 = p * a1 + q * b1
    // b1 = r * a0 + s * b0
    // b2 = r * a1 + s * b1
    let mut solved_params = None;

    let det_h = a0.wrapping_mul(b1).wrapping_sub(b0.wrapping_mul(a1));
    if det_h != 0 {
        let num_p = a1.wrapping_mul(b1).wrapping_sub(a2.wrapping_mul(b0));
        let num_q = a0.wrapping_mul(a2).wrapping_sub(a1.wrapping_mul(a1));
        let num_r = b1.wrapping_mul(b1).wrapping_sub(b2.wrapping_mul(b0));
        let num_s = a0.wrapping_mul(b2).wrapping_sub(a1.wrapping_mul(b1));

        if num_p % det_h == 0 && num_q % det_h == 0 && num_r % det_h == 0 && num_s % det_h == 0 {
            let p = num_p / det_h;
            let q = num_q / det_h;
            let r = num_r / det_h;
            let s = num_s / det_h;

            let mut valid = true;
            for k in 2..samples_a.len().min(samples_b.len()) {
                let exp_a = p
                    .wrapping_mul(samples_a[k - 1])
                    .wrapping_add(q.wrapping_mul(samples_b[k - 1]));
                let exp_b = r
                    .wrapping_mul(samples_a[k - 1])
                    .wrapping_add(s.wrapping_mul(samples_b[k - 1]));
                if samples_a[k] != exp_a || samples_b[k] != exp_b {
                    valid = false;
                    break;
                }
            }
            if valid {
                solved_params = Some((p, q, 0i64, r, s, 0i64));
            }
        }
    }

    // Method 2: Try affine linear recurrence with differences
    if solved_params.is_none() {
        let da0 = a1.wrapping_sub(a0);
        let da1 = a2.wrapping_sub(a1);
        let da2 = a3.wrapping_sub(a2);

        let db0 = b1.wrapping_sub(b0);
        let db1 = b2.wrapping_sub(b1);
        let db2 = b3.wrapping_sub(b2);

        let det_d = da0.wrapping_mul(db1).wrapping_sub(db0.wrapping_mul(da1));
        if det_d != 0 {
            let num_p = da1.wrapping_mul(db1).wrapping_sub(da2.wrapping_mul(db0));
            let num_q = da0.wrapping_mul(da2).wrapping_sub(da1.wrapping_mul(da1));
            let num_r = db1.wrapping_mul(db1).wrapping_sub(db2.wrapping_mul(db0));
            let num_s = da0.wrapping_mul(db2).wrapping_sub(da1.wrapping_mul(db1));

            if num_p % det_d == 0 && num_q % det_d == 0 && num_r % det_d == 0 && num_s % det_d == 0
            {
                let p = num_p / det_d;
                let q = num_q / det_d;
                let r = num_r / det_d;
                let s = num_s / det_d;

                let ca = a1
                    .wrapping_sub(p.wrapping_mul(a0))
                    .wrapping_sub(q.wrapping_mul(b0));
                let cb = b1
                    .wrapping_sub(r.wrapping_mul(a0))
                    .wrapping_sub(s.wrapping_mul(b0));

                let mut valid = true;
                for k in 1..samples_a.len().min(samples_b.len()) {
                    let exp_a = p
                        .wrapping_mul(samples_a[k - 1])
                        .wrapping_add(q.wrapping_mul(samples_b[k - 1]))
                        .wrapping_add(ca);
                    let exp_b = r
                        .wrapping_mul(samples_a[k - 1])
                        .wrapping_add(s.wrapping_mul(samples_b[k - 1]))
                        .wrapping_add(cb);
                    if samples_a[k] != exp_a || samples_b[k] != exp_b {
                        valid = false;
                        break;
                    }
                }
                if valid {
                    solved_params = Some((p, q, ca, r, s, cb));
                }
            }
        }
    }

    let (p, q, ca, r, s, cb) = solved_params?;

    // Check if number of iterations is constant
    let iter_term = interner.get(num_iters).clone();
    if let SymTerm::ConstInt(n, _) = iter_term {
        if n < 0 {
            return None;
        }
        let n_usize = n as usize;
        if n_usize < samples_a.len() && n_usize < samples_b.len() {
            return Some((
                interner.intern_int(samples_a[n_usize]),
                interner.intern_int(samples_b[n_usize]),
            ));
        }

        let m = [[p, q, ca], [r, s, cb], [0, 0, 1]];
        let m_pow = mat_pow_3x3(m, n);
        let a_n = m_pow[0][0]
            .wrapping_mul(a0)
            .wrapping_add(m_pow[0][1].wrapping_mul(b0))
            .wrapping_add(m_pow[0][2]);
        let b_n = m_pow[1][0]
            .wrapping_mul(a0)
            .wrapping_add(m_pow[1][1].wrapping_mul(b0))
            .wrapping_add(m_pow[1][2]);

        return Some((interner.intern_int(a_n), interner.intern_int(b_n)));
    }

    // No codegen handler exists for symbolic coupled recurrence intrinsics.
    // Fall back to the loop instead of emitting unhandleable calls.
    None
}

fn mat_pow_2x2(mut m: [[i64; 2]; 2], mut exp: i64) -> [[i64; 2]; 2] {
    let mut res = [[1, 0], [0, 1]];
    while exp > 0 {
        if exp & 1 == 1 {
            res = mat_mul_2x2(res, m);
        }
        m = mat_mul_2x2(m, m);
        exp >>= 1;
    }
    res
}

fn mat_mul_2x2(a: [[i64; 2]; 2], b: [[i64; 2]; 2]) -> [[i64; 2]; 2] {
    [
        [
            a[0][0]
                .wrapping_mul(b[0][0])
                .wrapping_add(a[0][1].wrapping_mul(b[1][0])),
            a[0][0]
                .wrapping_mul(b[0][1])
                .wrapping_add(a[0][1].wrapping_mul(b[1][1])),
        ],
        [
            a[1][0]
                .wrapping_mul(b[0][0])
                .wrapping_add(a[1][1].wrapping_mul(b[1][0])),
            a[1][0]
                .wrapping_mul(b[0][1])
                .wrapping_add(a[1][1].wrapping_mul(b[1][1])),
        ],
    ]
}

/// Solves constant-coefficient linear recurrence of order 3:
/// s_k = c1 * s_{k-1} + c2 * s_{k-2} + c3 * s_{k-3} (e.g. Tribonacci)
pub fn solve_order3_recurrence(
    samples: &[i64],
    num_iters: SymTermId,
    interner: &mut TermInterner,
) -> Option<SymTermId> {
    if samples.len() < 6 {
        return None;
    }
    let s = samples;
    let a = [[s[2], s[1], s[0]], [s[3], s[2], s[1]], [s[4], s[3], s[2]]];
    let b = [s[3], s[4], s[5]];

    let det = det_3x3(a);
    if det == 0 {
        return None;
    }

    let c1_num = det_3x3([
        [b[0], a[0][1], a[0][2]],
        [b[1], a[1][1], a[1][2]],
        [b[2], a[2][1], a[2][2]],
    ]);
    let c2_num = det_3x3([
        [a[0][0], b[0], a[0][2]],
        [a[1][0], b[1], a[1][2]],
        [a[2][0], b[2], a[2][2]],
    ]);
    let c3_num = det_3x3([
        [a[0][0], a[0][1], b[0]],
        [a[1][0], a[1][1], b[1]],
        [a[2][0], a[2][1], b[2]],
    ]);

    if c1_num % det != 0 || c2_num % det != 0 || c3_num % det != 0 {
        return None;
    }

    let c1 = c1_num / det;
    let c2 = c2_num / det;
    let c3 = c3_num / det;

    // Verify against remaining samples
    for k in 6..samples.len() {
        let expected = c1
            .wrapping_mul(s[k - 1])
            .wrapping_add(c2.wrapping_mul(s[k - 2]))
            .wrapping_add(c3.wrapping_mul(s[k - 3]));
        if s[k] != expected {
            return None;
        }
    }

    let iter_term = interner.get(num_iters).clone();
    if let SymTerm::ConstInt(n, _) = iter_term {
        if n < 0 {
            return None;
        }
        let n_idx = n as usize;
        if n_idx < samples.len() {
            return Some(interner.intern_int(samples[n_idx]));
        }
        let m = [[c1, c2, c3], [1, 0, 0], [0, 1, 0]];
        let m_pow = mat_pow_3x3(m, n - 2);
        let s_n = m_pow[0][0]
            .wrapping_mul(s[2])
            .wrapping_add(m_pow[0][1].wrapping_mul(s[1]))
            .wrapping_add(m_pow[0][2].wrapping_mul(s[0]));
        return Some(interner.intern_int(s_n));
    }

    // Symbolic number of iterations: emit __order3_recurrence intrinsic
    // Arguments: c1, c2, c3, s0, s1, s2, num_iters
    let c1_term = interner.intern_int(c1);
    let c2_term = interner.intern_int(c2);
    let c3_term = interner.intern_int(c3);
    let s0_term = interner.intern_int(s[0]);
    let s1_term = interner.intern_int(s[1]);
    let s2_term = interner.intern_int(s[2]);

    let args = vec![
        c1_term, c2_term, c3_term, s0_term, s1_term, s2_term, num_iters,
    ];
    Some(interner.intern_call("__order3_recurrence".to_string(), args, Type::I64))
}

fn det_3x3(m: [[i64; 3]; 3]) -> i64 {
    m[0][0]
        .wrapping_mul(
            m[1][1]
                .wrapping_mul(m[2][2])
                .wrapping_sub(m[1][2].wrapping_mul(m[2][1])),
        )
        .wrapping_sub(
            m[0][1].wrapping_mul(
                m[1][0]
                    .wrapping_mul(m[2][2])
                    .wrapping_sub(m[1][2].wrapping_mul(m[2][0])),
            ),
        )
        .wrapping_add(
            m[0][2].wrapping_mul(
                m[1][0]
                    .wrapping_mul(m[2][1])
                    .wrapping_sub(m[1][1].wrapping_mul(m[2][0])),
            ),
        )
}

#[allow(clippy::needless_range_loop)]
fn mat_mul_3x3(a: [[i64; 3]; 3], b: [[i64; 3]; 3]) -> [[i64; 3]; 3] {
    let mut res = [[0i64; 3]; 3];
    for r in 0..3 {
        for c in 0..3 {
            res[r][c] = a[r][0]
                .wrapping_mul(b[0][c])
                .wrapping_add(a[r][1].wrapping_mul(b[1][c]))
                .wrapping_add(a[r][2].wrapping_mul(b[2][c]));
        }
    }
    res
}

fn mat_pow_3x3(mut m: [[i64; 3]; 3], mut exp: i64) -> [[i64; 3]; 3] {
    let mut res = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
    while exp > 0 {
        if exp & 1 == 1 {
            res = mat_mul_3x3(res, m);
        }
        m = mat_mul_3x3(m, m);
        exp >>= 1;
    }
    res
}

/// Computes closed form for a linear induction variable: v_n = init + n * step
pub fn solve_symbolic_linear_induction(
    init: SymTermId,
    step: SymTermId,
    num_iters: SymTermId,
    interner: &mut TermInterner,
) -> SymTermId {
    let n_times_step = interner.intern_binary(BinaryOp::Mul, num_iters, step, Type::I64);
    interner.intern_binary(BinaryOp::Add, init, n_times_step, Type::I64)
}

/// Computes closed form for an accumulator of a linear variable:
/// s_n = init + n * base + (n * (n - 1) / 2) * step
pub fn solve_symbolic_accumulator(
    init: SymTermId,
    base: SymTermId,
    step: SymTermId,
    num_iters: SymTermId,
    interner: &mut TermInterner,
) -> SymTermId {
    let n_times_base = interner.intern_binary(BinaryOp::Mul, num_iters, base, Type::I64);
    let one = interner.intern_int(1);
    let two = interner.intern_int(2);
    let n_minus_1 = interner.intern_binary(BinaryOp::Sub, num_iters, one, Type::I64);
    let n_times_n1 = interner.intern_binary(BinaryOp::Mul, num_iters, n_minus_1, Type::I64);
    let tri = interner.intern_binary(BinaryOp::Div, n_times_n1, two, Type::I64);
    let quad = interner.intern_binary(BinaryOp::Mul, tri, step, Type::I64);
    let sum1 = interner.intern_binary(BinaryOp::Add, init, n_times_base, Type::I64);
    interner.intern_binary(BinaryOp::Add, sum1, quad, Type::I64)
}

/// Computes closed form for a geometric variable: s_n = init * ratio^n
pub fn solve_symbolic_geometric(
    init: SymTermId,
    ratio: SymTermId,
    num_iters: SymTermId,
    interner: &mut TermInterner,
) -> SymTermId {
    let pow = interner.intern_binary(BinaryOp::Pow, ratio, num_iters, Type::I64);
    interner.intern_binary(BinaryOp::Mul, init, pow, Type::I64)
}
