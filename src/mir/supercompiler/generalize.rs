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
        (SymTerm::Ref(i1, ty1), SymTerm::Ref(i2, _)) => {
            let common = msg_helper(*i1, *i2, interner, next_var_id, memo, mappings);
            interner.intern_ref(common, ty1.clone())
        }
        (SymTerm::Deref(p1, ty1), SymTerm::Deref(p2, _)) => {
            let common = msg_helper(*p1, *p2, interner, next_var_id, memo, mappings);
            interner.intern_deref(common, ty1.clone())
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
    if d1_0 == d1_1 {
        // s0 + N * d1
        let s0_term = interner.intern_int(s0);
        let d1_term = interner.intern_int(d1_0);
        let n_times_d1 = interner.intern_binary(
            BinaryOp::Mul,
            num_iters,
            d1_term,
            Type::I64,
        );
        return Some(interner.intern_binary(
            BinaryOp::Add,
            s0_term,
            n_times_d1,
            Type::I64,
        ));
    }

    // Degree 2: Quadratic / Triangular sequence
    if samples.len() >= 4 {
        let s3 = samples[3];
        let d1_2 = s3.wrapping_sub(s2);
        let d2_0 = d1_1.wrapping_sub(d1_0);
        let d2_1 = d1_2.wrapping_sub(d1_1);

        if d2_0 == d2_1 {
            // Formula: s_k = s0 + k * d1_0 + (k * (k - 1) / 2) * d2_0
            let s0_term = interner.intern_int(s0);
            let d1_term = interner.intern_int(d1_0);
            let d2_term = interner.intern_int(d2_0);
            let one_term = interner.intern_int(1);
            let two_term = interner.intern_int(2);

            // k * d1_0
            let linear_part = interner.intern_binary(
                BinaryOp::Mul,
                num_iters,
                d1_term,
                Type::I64,
            );

            // (k - 1)
            let k_minus_1 = interner.intern_binary(
                BinaryOp::Sub,
                num_iters,
                one_term,
                Type::I64,
            );

            // k * (k - 1)
            let k_times_k_minus_1 = interner.intern_binary(
                BinaryOp::Mul,
                num_iters,
                k_minus_1,
                Type::I64,
            );

            // k * (k - 1) / 2
            let tri_part = interner.intern_binary(
                BinaryOp::Div,
                k_times_k_minus_1,
                two_term,
                Type::I64,
            );

            // (k * (k - 1) / 2) * d2_0
            let quad_part = interner.intern_binary(
                BinaryOp::Mul,
                tri_part,
                d2_term,
                Type::I64,
            );

            // s0 + linear + quad
            let part1 = interner.intern_binary(
                BinaryOp::Add,
                s0_term,
                linear_part,
                Type::I64,
            );
            return Some(interner.intern_binary(
                BinaryOp::Add,
                part1,
                quad_part,
                Type::I64,
            ));
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
        let d2_2 = d1_3.wrapping_sub(d1_2);
        let d3_0 = d2_1.wrapping_sub(d2_0);
        let d3_1 = d2_2.wrapping_sub(d2_1);

        if d3_0 == d3_1 {
            let s0_term = interner.intern_int(s0);
            let d1_term = interner.intern_int(d1_0);
            let d2_term = interner.intern_int(d2_0);
            let d3_term = interner.intern_int(d3_0);
            let one_term = interner.intern_int(1);
            let two_term = interner.intern_int(2);
            let six_term = interner.intern_int(6);

            let linear_part = interner.intern_binary(BinaryOp::Mul, num_iters, d1_term, Type::I64);
            let k_minus_1 = interner.intern_binary(BinaryOp::Sub, num_iters, one_term, Type::I64);
            let k_minus_2 = interner.intern_binary(BinaryOp::Sub, num_iters, two_term, Type::I64);

            let k_times_k1 = interner.intern_binary(BinaryOp::Mul, num_iters, k_minus_1, Type::I64);
            let tri_part = interner.intern_binary(BinaryOp::Div, k_times_k1, two_term, Type::I64);
            let quad_part = interner.intern_binary(BinaryOp::Mul, tri_part, d2_term, Type::I64);

            let k_times_k1_k2 = interner.intern_binary(BinaryOp::Mul, k_times_k1, k_minus_2, Type::I64);
            let cubic_binom = interner.intern_binary(BinaryOp::Div, k_times_k1_k2, six_term, Type::I64);
            let cubic_part = interner.intern_binary(BinaryOp::Mul, cubic_binom, d3_term, Type::I64);

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
        let d2_3 = d1_4.wrapping_sub(d1_3);
        let d3_0 = d2_1.wrapping_sub(d2_0);
        let d3_1 = d2_2.wrapping_sub(d2_1);
        let d3_2 = d2_3.wrapping_sub(d2_2);
        let d4_0 = d3_1.wrapping_sub(d3_0);
        let d4_1 = d3_2.wrapping_sub(d3_1);

        if d4_0 == d4_1 {
            let s0_term = interner.intern_int(s0);
            let d1_term = interner.intern_int(d1_0);
            let d2_term = interner.intern_int(d2_0);
            let d3_term = interner.intern_int(d3_0);
            let d4_term = interner.intern_int(d4_0);
            let one_term = interner.intern_int(1);
            let two_term = interner.intern_int(2);
            let three_term = interner.intern_int(3);
            let six_term = interner.intern_int(6);
            let twentyfour_term = interner.intern_int(24);

            let linear_part = interner.intern_binary(BinaryOp::Mul, num_iters, d1_term, Type::I64);
            let k_minus_1 = interner.intern_binary(BinaryOp::Sub, num_iters, one_term, Type::I64);
            let k_minus_2 = interner.intern_binary(BinaryOp::Sub, num_iters, two_term, Type::I64);
            let k_minus_3 = interner.intern_binary(BinaryOp::Sub, num_iters, three_term, Type::I64);

            let k_times_k1 = interner.intern_binary(BinaryOp::Mul, num_iters, k_minus_1, Type::I64);
            let tri_part = interner.intern_binary(BinaryOp::Div, k_times_k1, two_term, Type::I64);
            let quad_part = interner.intern_binary(BinaryOp::Mul, tri_part, d2_term, Type::I64);

            let k_times_k1_k2 = interner.intern_binary(BinaryOp::Mul, k_times_k1, k_minus_2, Type::I64);
            let cubic_binom = interner.intern_binary(BinaryOp::Div, k_times_k1_k2, six_term, Type::I64);
            let cubic_part = interner.intern_binary(BinaryOp::Mul, cubic_binom, d3_term, Type::I64);

            let k_times_k1_k2_k3 = interner.intern_binary(BinaryOp::Mul, k_times_k1_k2, k_minus_3, Type::I64);
            let quartic_binom = interner.intern_binary(BinaryOp::Div, k_times_k1_k2_k3, twentyfour_term, Type::I64);
            let quartic_part = interner.intern_binary(BinaryOp::Mul, quartic_binom, d4_term, Type::I64);

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
            let pow_term = interner.intern_binary(
                BinaryOp::Pow,
                ratio_term,
                num_iters,
                Type::I64,
            );
            return Some(interner.intern_binary(
                BinaryOp::Mul,
                s0_term,
                pow_term,
                Type::I64,
            ));
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
        if d * d == disc && (c1 + d) % 2 == 0 {
            let r1 = (c1 + d) / 2;
            let r2 = (c1 - d) / 2;
            if r1 != r2 {
                let num_a = s1.wrapping_sub(s0.wrapping_mul(r2));
                let den_a = r1 - r2;
                if den_a != 0 && num_a % den_a == 0 {
                    let a = num_a / den_a;
                    let b = s0 - a;
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

    // Fibonacci pattern: c1 = 1, c2 = 1, s0 = 0, s1 = 1
    if c1 == 1 && c2 == 1 && s0 == 0 && s1 == 1 {
        return Some(interner.intern_call(
            "__numlang_fib".to_string(),
            vec![num_iters],
            Type::I64,
        ));
    }
    // Shifted Fibonacci pattern (e.g. b in fib loop): c1 = 1, c2 = 1, s0 = 1, s1 = 1 (F(k+1))
    if c1 == 1 && c2 == 1 && s0 == 1 && s1 == 1 {
        let one = interner.intern_int(1);
        let n_plus_1 = interner.intern_binary(BinaryOp::Add, num_iters, one, Type::I64);
        return Some(interner.intern_call(
            "__numlang_fib".to_string(),
            vec![n_plus_1],
            Type::I64,
        ));
    }

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
                let exp_a = p.wrapping_mul(samples_a[k - 1]).wrapping_add(q.wrapping_mul(samples_b[k - 1]));
                let exp_b = r.wrapping_mul(samples_a[k - 1]).wrapping_add(s.wrapping_mul(samples_b[k - 1]));
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

            if num_p % det_d == 0 && num_q % det_d == 0 && num_r % det_d == 0 && num_s % det_d == 0 {
                let p = num_p / det_d;
                let q = num_q / det_d;
                let r = num_r / det_d;
                let s = num_s / det_d;

                let ca = a1.wrapping_sub(p.wrapping_mul(a0)).wrapping_sub(q.wrapping_mul(b0));
                let cb = b1.wrapping_sub(r.wrapping_mul(a0)).wrapping_sub(s.wrapping_mul(b0));

                let mut valid = true;
                for k in 1..samples_a.len().min(samples_b.len()) {
                    let exp_a = p.wrapping_mul(samples_a[k - 1])
                        .wrapping_add(q.wrapping_mul(samples_b[k - 1]))
                        .wrapping_add(ca);
                    let exp_b = r.wrapping_mul(samples_a[k - 1])
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

        let m = [
            [p, q, ca],
            [r, s, cb],
            [0, 0, 1],
        ];
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

    // Symbolic number of iterations -> emit intrinsics __coupled_a and __coupled_b
    let p_term = interner.intern_int(p);
    let q_term = interner.intern_int(q);
    let ca_term = interner.intern_int(ca);
    let r_term = interner.intern_int(r);
    let s_term = interner.intern_int(s);
    let cb_term = interner.intern_int(cb);
    let a0_term = interner.intern_int(a0);
    let b0_term = interner.intern_int(b0);

    let args = vec![
        p_term,
        q_term,
        ca_term,
        r_term,
        s_term,
        cb_term,
        a0_term,
        b0_term,
        num_iters,
    ];

    let term_a = interner.intern_call("__coupled_a".to_string(), args.clone(), Type::I64);
    let term_b = interner.intern_call("__coupled_b".to_string(), args, Type::I64);

    Some((term_a, term_b))
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
            a[0][0].wrapping_mul(b[0][0]).wrapping_add(a[0][1].wrapping_mul(b[1][0])),
            a[0][0].wrapping_mul(b[0][1]).wrapping_add(a[0][1].wrapping_mul(b[1][1])),
        ],
        [
            a[1][0].wrapping_mul(b[0][0]).wrapping_add(a[1][1].wrapping_mul(b[1][0])),
            a[1][0].wrapping_mul(b[0][1]).wrapping_add(a[1][1].wrapping_mul(b[1][1])),
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
    let a = [
        [s[2], s[1], s[0]],
        [s[3], s[2], s[1]],
        [s[4], s[3], s[2]],
    ];
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
        let m = [
            [c1, c2, c3],
            [1, 0, 0],
            [0, 1, 0],
        ];
        let m_pow = mat_pow_3x3(m, n - 2);
        let s_n = m_pow[0][0]
            .wrapping_mul(s[2])
            .wrapping_add(m_pow[0][1].wrapping_mul(s[1]))
            .wrapping_add(m_pow[0][2].wrapping_mul(s[0]));
        return Some(interner.intern_int(s_n));
    }

    None
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
