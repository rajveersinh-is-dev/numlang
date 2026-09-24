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
        (SymTerm::Constructor(n1, f1, ty1), SymTerm::Constructor(n2, f2, _))
            if n1 == n2 && f1.len() == f2.len() =>
        {
            let mut common_f = Vec::with_capacity(f1.len());
            for (&a, &b) in f1.iter().zip(f2.iter()) {
                common_f.push(msg_helper(a, b, interner, next_var_id, memo, mappings));
            }
            interner.intern_constructor(n1.clone(), common_f, ty1.clone())
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

    None
}
