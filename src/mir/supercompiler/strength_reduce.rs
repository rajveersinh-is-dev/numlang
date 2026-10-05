//! Phase 64: Post-Collapse Residual MIR Strength Reduction.
//!
//! Scans residual MIR basic blocks and performs strength reduction on expensive operations:
//! 1. Power-of-2 multiplication: `x * 2^k` -> `x << k` (STRENGTH-02)
//! 2. Near-power-of-2 multiplication: `x * (2^a ± 2^b)` -> shift + add/sub sequences (STRENGTH-03)
//! 3. Power-of-2 division: `x / 2^k` -> `x >> k` (STRENGTH-04)
//! 4. Identity operations: `x * 0 -> 0`, `x * 1 -> x`, `x * -1 -> -x`, `x / 1 -> x`.

use std::collections::HashMap;

use crate::ast::{BinaryOp, UnaryOp};
use crate::mir::lower::{MirBasicBlock, MirFunction, MirLocalDecl, MirProgram, Rvalue, Statement};
use crate::mir::Place;
use crate::typecheck::typed_ast::TypedLiteral;
use crate::typecheck::types::Type;

/// Classification of multiplication constant for strength reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MulReduction {
    /// x * 0 = 0
    Zero,
    /// x * 1 = x
    Identity,
    /// x * -1 = -x
    Negate,
    /// x * 2^k = x << k (k >= 1)
    PowerOfTwo(u32),
    /// x * -(2^k) = -(x << k) (k >= 1)
    NegPowerOfTwo(u32),
    /// x * (2^a + 2^b) = (x << a) + (x << b) (a > b >= 0)
    SumOfPowers(u32, u32),
    /// x * (2^a - 2^b) = (x << a) - (x << b) (a > b >= 0)
    DiffOfPowers(u32, u32),
}

/// Classify an integer multiplier for strength reduction opportunities.
pub fn classify_mul_const(c: i64) -> Option<MulReduction> {
    if c == 0 {
        return Some(MulReduction::Zero);
    }
    if c == 1 {
        return Some(MulReduction::Identity);
    }
    if c == -1 {
        return Some(MulReduction::Negate);
    }
    if c > 0 && (c as u64).is_power_of_two() {
        let k = c.trailing_zeros();
        if k < 64 {
            return Some(MulReduction::PowerOfTwo(k));
        }
    }
    if c < 0 && c != i64::MIN && ((-c) as u64).is_power_of_two() {
        let k = (-c).trailing_zeros();
        if k < 64 {
            return Some(MulReduction::NegPowerOfTwo(k));
        }
    }
    if c > 0 {
        // Check if c has exactly 2 bits set: c = 2^a + 2^b (a > b >= 0)
        if c.count_ones() == 2 {
            let b = c.trailing_zeros();
            let remaining = c ^ (1i64 << b);
            let a = remaining.trailing_zeros();
            if a < 64 && b < 64 && a > b {
                return Some(MulReduction::SumOfPowers(a, b));
            }
        }
        // Check if c = 2^a - 2^b (a > b >= 0)
        let b = c.trailing_zeros();
        if let Some(sum) = c.checked_add(1i64 << b) {
            if sum > 0 && (sum as u64).is_power_of_two() {
                let a = sum.trailing_zeros();
                if a < 64 && a > b {
                    return Some(MulReduction::DiffOfPowers(a, b));
                }
            }
        }
    }
    None
}

/// Classification of division constant for strength reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivReduction {
    /// x / 1 = x
    Identity,
    /// x / 2^k = x >> k (k >= 1)
    PowerOfTwo(u32),
}

/// Classify an integer divisor for strength reduction opportunities.
pub fn classify_div_const(c: i64) -> Option<DivReduction> {
    if c == 1 {
        Some(DivReduction::Identity)
    } else if c > 0 && (c as u64).is_power_of_two() {
        let k = c.trailing_zeros();
        if k < 64 {
            Some(DivReduction::PowerOfTwo(k))
        } else {
            None
        }
    } else {
        None
    }
}

/// Collects single-assignment integer constants throughout a MirFunction.
fn collect_single_assignment_constants(func: &MirFunction) -> HashMap<String, (i64, Type)> {
    let mut assign_counts: HashMap<String, usize> = HashMap::new();
    let mut const_values: HashMap<String, (i64, Type)> = HashMap::new();

    for block in &func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(dest, rval) = stmt;
            if dest.projections.is_empty() {
                *assign_counts.entry(dest.local.clone()).or_default() += 1;
                if let Rvalue::Constant(TypedLiteral::Int(val, ty)) = rval {
                    const_values.insert(dest.local.clone(), (*val, ty.clone()));
                }
            }
        }
    }

    const_values
        .into_iter()
        .filter(|(local, _)| assign_counts.get(local).copied().unwrap_or(0) == 1)
        .collect()
}

/// Allocates a new temporary local in `func.locals` and records its type.
fn alloc_temp(
    locals: &mut Vec<MirLocalDecl>,
    local_types: &mut HashMap<String, Type>,
    counter: &mut usize,
    ty: &Type,
) -> Place {
    let name = format!("_sr_t{}", *counter);
    *counter += 1;
    locals.push(MirLocalDecl {
        name: name.clone(),
        ty: ty.clone(),
        mutable: false,
    });
    local_types.insert(name.clone(), ty.clone());
    Place {
        local: name,
        projections: vec![],
    }
}

/// Performs strength reduction across all basic blocks of a MirFunction.
/// Returns the number of operations reduced.
pub fn strength_reduce_mir_function(func: &mut MirFunction) -> usize {
    let mut reductions = 0;
    let global_consts = collect_single_assignment_constants(func);

    let mut local_types: HashMap<String, Type> = HashMap::new();
    for (name, ty) in &func.params {
        local_types.insert(name.clone(), ty.clone());
    }
    for l in &func.locals {
        local_types.insert(l.name.clone(), l.ty.clone());
    }

    let mut temp_counter = func.locals.len() + 1;
    let mut new_locals = func.locals.clone();

    let mut updated_blocks = Vec::with_capacity(func.blocks.len());

    for block in &func.blocks {
        let mut block_consts = global_consts.clone();
        let mut new_stmts = Vec::with_capacity(block.statements.len() * 2);

        for stmt in &block.statements {
            let Statement::Assign(dest, rval) = stmt;

            // Track any in-block constant assignment
            if dest.projections.is_empty() {
                if let Rvalue::Constant(TypedLiteral::Int(val, ty)) = rval {
                    block_consts.insert(dest.local.clone(), (*val, ty.clone()));
                }
            }

            match rval {
                Rvalue::BinaryOp(BinaryOp::Mul, left, right) => {
                    let left_const = if left.projections.is_empty() {
                        block_consts.get(&left.local).cloned()
                    } else {
                        None
                    };
                    let right_const = if right.projections.is_empty() {
                        block_consts.get(&right.local).cloned()
                    } else {
                        None
                    };

                    // Multiplication is commutative: pick constant and variable operand
                    let candidate = if let Some((c, _)) = right_const {
                        Some((left.clone(), c))
                    } else if let Some((c, _)) = left_const {
                        Some((right.clone(), c))
                    } else {
                        None
                    };

                    if let Some((x, c)) = candidate {
                        if let Some(reduction) = classify_mul_const(c) {
                            let var_ty = local_types
                                .get(&x.local)
                                .cloned()
                                .unwrap_or(Type::I64);

                            match reduction {
                                MulReduction::Zero => {
                                    new_stmts.push(Statement::Assign(
                                        dest.clone(),
                                        Rvalue::Constant(TypedLiteral::Int(0, var_ty)),
                                    ));
                                    reductions += 1;
                                    continue;
                                }
                                MulReduction::Identity => {
                                    new_stmts.push(Statement::Assign(
                                        dest.clone(),
                                        Rvalue::Use(x),
                                    ));
                                    reductions += 1;
                                    continue;
                                }
                                MulReduction::Negate => {
                                    new_stmts.push(Statement::Assign(
                                        dest.clone(),
                                        Rvalue::UnaryOp(UnaryOp::Neg, x),
                                    ));
                                    reductions += 1;
                                    continue;
                                }
                                MulReduction::PowerOfTwo(k) => {
                                    let k_place = alloc_temp(
                                        &mut new_locals,
                                        &mut local_types,
                                        &mut temp_counter,
                                        &var_ty,
                                    );
                                    new_stmts.push(Statement::Assign(
                                        k_place.clone(),
                                        Rvalue::Constant(TypedLiteral::Int(k as i64, var_ty.clone())),
                                    ));
                                    new_stmts.push(Statement::Assign(
                                        dest.clone(),
                                        Rvalue::BinaryOp(BinaryOp::Shl, x, k_place),
                                    ));
                                    reductions += 1;
                                    continue;
                                }
                                MulReduction::NegPowerOfTwo(k) => {
                                    let k_place = alloc_temp(
                                        &mut new_locals,
                                        &mut local_types,
                                        &mut temp_counter,
                                        &var_ty,
                                    );
                                    let t1 = alloc_temp(
                                        &mut new_locals,
                                        &mut local_types,
                                        &mut temp_counter,
                                        &var_ty,
                                    );
                                    new_stmts.push(Statement::Assign(
                                        k_place.clone(),
                                        Rvalue::Constant(TypedLiteral::Int(k as i64, var_ty.clone())),
                                    ));
                                    new_stmts.push(Statement::Assign(
                                        t1.clone(),
                                        Rvalue::BinaryOp(BinaryOp::Shl, x, k_place),
                                    ));
                                    new_stmts.push(Statement::Assign(
                                        dest.clone(),
                                        Rvalue::UnaryOp(UnaryOp::Neg, t1),
                                    ));
                                    reductions += 1;
                                    continue;
                                }
                                MulReduction::SumOfPowers(a, b) => {
                                    if b == 0 {
                                        let a_place = alloc_temp(
                                            &mut new_locals,
                                            &mut local_types,
                                            &mut temp_counter,
                                            &var_ty,
                                        );
                                        let t1 = alloc_temp(
                                            &mut new_locals,
                                            &mut local_types,
                                            &mut temp_counter,
                                            &var_ty,
                                        );
                                        new_stmts.push(Statement::Assign(
                                            a_place.clone(),
                                            Rvalue::Constant(TypedLiteral::Int(a as i64, var_ty.clone())),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            t1.clone(),
                                            Rvalue::BinaryOp(BinaryOp::Shl, x.clone(), a_place),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            dest.clone(),
                                            Rvalue::BinaryOp(BinaryOp::Add, t1, x),
                                        ));
                                    } else {
                                        let a_place = alloc_temp(
                                            &mut new_locals,
                                            &mut local_types,
                                            &mut temp_counter,
                                            &var_ty,
                                        );
                                        let b_place = alloc_temp(
                                            &mut new_locals,
                                            &mut local_types,
                                            &mut temp_counter,
                                            &var_ty,
                                        );
                                        let t1 = alloc_temp(
                                            &mut new_locals,
                                            &mut local_types,
                                            &mut temp_counter,
                                            &var_ty,
                                        );
                                        let t2 = alloc_temp(
                                            &mut new_locals,
                                            &mut local_types,
                                            &mut temp_counter,
                                            &var_ty,
                                        );
                                        new_stmts.push(Statement::Assign(
                                            a_place.clone(),
                                            Rvalue::Constant(TypedLiteral::Int(a as i64, var_ty.clone())),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            b_place.clone(),
                                            Rvalue::Constant(TypedLiteral::Int(b as i64, var_ty.clone())),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            t1.clone(),
                                            Rvalue::BinaryOp(BinaryOp::Shl, x.clone(), a_place),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            t2.clone(),
                                            Rvalue::BinaryOp(BinaryOp::Shl, x, b_place),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            dest.clone(),
                                            Rvalue::BinaryOp(BinaryOp::Add, t1, t2),
                                        ));
                                    }
                                    reductions += 1;
                                    continue;
                                }
                                MulReduction::DiffOfPowers(a, b) => {
                                    if b == 0 {
                                        let a_place = alloc_temp(
                                            &mut new_locals,
                                            &mut local_types,
                                            &mut temp_counter,
                                            &var_ty,
                                        );
                                        let t1 = alloc_temp(
                                            &mut new_locals,
                                            &mut local_types,
                                            &mut temp_counter,
                                            &var_ty,
                                        );
                                        new_stmts.push(Statement::Assign(
                                            a_place.clone(),
                                            Rvalue::Constant(TypedLiteral::Int(a as i64, var_ty.clone())),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            t1.clone(),
                                            Rvalue::BinaryOp(BinaryOp::Shl, x.clone(), a_place),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            dest.clone(),
                                            Rvalue::BinaryOp(BinaryOp::Sub, t1, x),
                                        ));
                                    } else {
                                        let a_place = alloc_temp(
                                            &mut new_locals,
                                            &mut local_types,
                                            &mut temp_counter,
                                            &var_ty,
                                        );
                                        let b_place = alloc_temp(
                                            &mut new_locals,
                                            &mut local_types,
                                            &mut temp_counter,
                                            &var_ty,
                                        );
                                        let t1 = alloc_temp(
                                            &mut new_locals,
                                            &mut local_types,
                                            &mut temp_counter,
                                            &var_ty,
                                        );
                                        let t2 = alloc_temp(
                                            &mut new_locals,
                                            &mut local_types,
                                            &mut temp_counter,
                                            &var_ty,
                                        );
                                        new_stmts.push(Statement::Assign(
                                            a_place.clone(),
                                            Rvalue::Constant(TypedLiteral::Int(a as i64, var_ty.clone())),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            b_place.clone(),
                                            Rvalue::Constant(TypedLiteral::Int(b as i64, var_ty.clone())),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            t1.clone(),
                                            Rvalue::BinaryOp(BinaryOp::Shl, x.clone(), a_place),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            t2.clone(),
                                            Rvalue::BinaryOp(BinaryOp::Shl, x, b_place),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            dest.clone(),
                                            Rvalue::BinaryOp(BinaryOp::Sub, t1, t2),
                                        ));
                                    }
                                    reductions += 1;
                                    continue;
                                }
                            }
                        }
                    }
                    new_stmts.push(stmt.clone());
                }
                Rvalue::BinaryOp(BinaryOp::Div, left, right) => {
                    // Division is not commutative: divisor must be the right operand
                    let right_const = if right.projections.is_empty() {
                        block_consts.get(&right.local).cloned()
                    } else {
                        None
                    };

                    if let Some((c, _)) = right_const {
                        if let Some(reduction) = classify_div_const(c) {
                            let var_ty = local_types
                                .get(&left.local)
                                .cloned()
                                .unwrap_or(Type::I64);

                            match reduction {
                                DivReduction::Identity => {
                                    new_stmts.push(Statement::Assign(
                                        dest.clone(),
                                        Rvalue::Use(left.clone()),
                                    ));
                                    reductions += 1;
                                    continue;
                                }
                                DivReduction::PowerOfTwo(k) => {
                                    let k_place = alloc_temp(
                                        &mut new_locals,
                                        &mut local_types,
                                        &mut temp_counter,
                                        &var_ty,
                                    );
                                    new_stmts.push(Statement::Assign(
                                        k_place.clone(),
                                        Rvalue::Constant(TypedLiteral::Int(k as i64, var_ty.clone())),
                                    ));
                                    new_stmts.push(Statement::Assign(
                                        dest.clone(),
                                        Rvalue::BinaryOp(BinaryOp::Shr, left.clone(), k_place),
                                    ));
                                    reductions += 1;
                                    continue;
                                }
                            }
                        }
                    }
                    new_stmts.push(stmt.clone());
                }
                _ => {
                    new_stmts.push(stmt.clone());
                }
            }
        }

        updated_blocks.push(MirBasicBlock {
            id: block.id.clone(),
            arguments: block.arguments.clone(),
            statements: new_stmts,
            terminator: block.terminator.clone(),
        });
    }

    func.locals = new_locals;
    func.blocks = updated_blocks;
    reductions
}

/// Applies strength reduction to every function in a MirProgram.
pub fn strength_reduce_mir_program(program: &mut MirProgram) -> usize {
    let mut total = 0;
    for func in &mut program.functions {
        total += strength_reduce_mir_function(func);
    }
    total
}
