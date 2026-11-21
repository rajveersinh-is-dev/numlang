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
use crate::mir::{Place, Projection};
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

#[derive(Debug, Clone, PartialEq)]
enum ConstVal {
    Int(i64, Type),
    Bool(bool),
    Float(f64, Type),
    Array(Vec<ConstVal>),
    Struct(String, Vec<(String, ConstVal)>),
}

impl ConstVal {
    fn as_i64(&self) -> Option<i64> {
        match self {
            ConstVal::Int(v, _) => Some(*v),
            ConstVal::Bool(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }

    fn to_rvalue(&self) -> Option<Rvalue> {
        match self {
            ConstVal::Int(v, ty) => Some(Rvalue::Constant(TypedLiteral::Int(*v, ty.clone()))),
            ConstVal::Bool(b) => Some(Rvalue::Constant(TypedLiteral::Bool(*b))),
            ConstVal::Float(f, ty) => Some(Rvalue::Constant(TypedLiteral::Float(*f, ty.clone()))),
            _ => None,
        }
    }
}

fn resolve_place_const<'a>(place: &Place, env: &'a HashMap<String, ConstVal>) -> Option<&'a ConstVal> {
    let mut cur = env.get(&place.local)?;
    for proj in &place.projections {
        match proj {
            Projection::Index(idx) => {
                let idx_val = if idx.projections.is_empty() {
                    env.get(&idx.local)?.as_i64()?
                } else {
                    resolve_place_const(idx, env)?.as_i64()?
                };
                if idx_val < 0 {
                    return None;
                }
                let arr = match cur {
                    ConstVal::Array(items) => items,
                    _ => return None,
                };
                cur = arr.get(idx_val as usize)?;
            }
            Projection::Field(name) => {
                let fields = match cur {
                    ConstVal::Struct(_, fields) => fields,
                    _ => return None,
                };
                cur = fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)?;
            }
            Projection::Deref | Projection::Payload(_) => return None,
        }
    }
    Some(cur)
}

fn eval_rvalue_const(rval: &Rvalue, env: &HashMap<String, ConstVal>) -> Option<ConstVal> {
    match rval {
        Rvalue::Constant(TypedLiteral::Int(v, ty)) => Some(ConstVal::Int(*v, ty.clone())),
        Rvalue::Constant(TypedLiteral::Bool(b)) => Some(ConstVal::Bool(*b)),
        Rvalue::Constant(TypedLiteral::Float(f, ty)) => Some(ConstVal::Float(*f, ty.clone())),
        Rvalue::Use(p) => resolve_place_const(p, env).cloned(),
        Rvalue::Array(items) => {
            let mut vals = Vec::with_capacity(items.len());
            for it in items {
                vals.push(resolve_place_const(it, env).cloned()?);
            }
            Some(ConstVal::Array(vals))
        }
        Rvalue::Struct(name, fields) => {
            let mut vals = Vec::with_capacity(fields.len());
            for (fname, fp) in fields {
                vals.push((fname.clone(), resolve_place_const(fp, env).cloned()?));
            }
            Some(ConstVal::Struct(name.clone(), vals))
        }
        Rvalue::BinaryOp(op, l, r) => {
            let lv = resolve_place_const(l, env)?;
            let rv = resolve_place_const(r, env)?;
            match (lv, rv) {
                (ConstVal::Int(a, ty), ConstVal::Int(b, _)) => {
                    let int_res = match op {
                        BinaryOp::Add => Some(a.wrapping_add(*b)),
                        BinaryOp::Sub => Some(a.wrapping_sub(*b)),
                        BinaryOp::Mul => Some(a.wrapping_mul(*b)),
                        BinaryOp::Div if *b != 0 => Some(a.wrapping_div(*b)),
                        BinaryOp::Mod if *b != 0 => Some(a.wrapping_rem(*b)),
                        BinaryOp::BitAnd => Some(a & *b),
                        BinaryOp::BitOr => Some(a | *b),
                        BinaryOp::BitXor => Some(a ^ *b),
                        BinaryOp::Shl => Some(a.wrapping_shl((*b as u32) & 63)),
                        BinaryOp::Shr => Some(a.wrapping_shr((*b as u32) & 63)),
                        _ => None,
                    };
                    if let Some(v) = int_res {
                        Some(ConstVal::Int(v, ty.clone()))
                    } else {
                        let cmp = match op {
                            BinaryOp::Eq => Some(*a == *b),
                            BinaryOp::Ne => Some(*a != *b),
                            BinaryOp::Lt => Some(*a < *b),
                            BinaryOp::Le => Some(*a <= *b),
                            BinaryOp::Gt => Some(*a > *b),
                            BinaryOp::Ge => Some(*a >= *b),
                            _ => None,
                        };
                        cmp.map(ConstVal::Bool)
                    }
                }
                (ConstVal::Bool(a), ConstVal::Bool(b)) => {
                    let res = match op {
                        BinaryOp::Eq => Some(*a == *b),
                        BinaryOp::Ne => Some(*a != *b),
                        _ => None,
                    };
                    res.map(ConstVal::Bool)
                }
                _ => None,
            }
        }
        Rvalue::UnaryOp(op, p) => {
            let v = resolve_place_const(p, env)?;
            match (op, v) {
                (UnaryOp::Neg, ConstVal::Int(a, ty)) => Some(ConstVal::Int(a.wrapping_neg(), ty.clone())),
                (UnaryOp::Not, ConstVal::Bool(b)) => Some(ConstVal::Bool(!*b)),
                // UnaryOp::BitNot does not exist in NumLang AST; skip.
                _ => None,
            }
        }
        _ => None,
    }
}

fn collect_global_single_assign_consts(func: &MirFunction) -> HashMap<String, ConstVal> {
    let mut assign_counts: HashMap<String, usize> = HashMap::new();
    let mut rvals: HashMap<String, Rvalue> = HashMap::new();
    for block in &func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(dest, rval) = stmt;
            if dest.projections.is_empty() {
                *assign_counts.entry(dest.local.clone()).or_default() += 1;
                rvals.insert(dest.local.clone(), rval.clone());
            } else {
                // projected store mutates the root - not single-assignment const
                assign_counts.entry(dest.local.clone()).and_modify(|c| *c += 10);
            }
        }
    }
    let mut env: HashMap<String, ConstVal> = HashMap::new();
    // fixed-point evaluation
    for _ in 0..8 {
        let mut changed = false;
        for (local, count) in &assign_counts {
            if *count != 1 || env.contains_key(local) {
                continue;
            }
            if let Some(rv) = rvals.get(local) {
                if let Some(cv) = eval_rvalue_const(rv, &env) {
                    env.insert(local.clone(), cv);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    env
}

fn const_propagate_block(block: &mut MirBasicBlock, global_env: &HashMap<String, ConstVal>) -> usize {
    let mut env: HashMap<String, ConstVal> = global_env.clone();
    let mut folded = 0usize;
    let mut new_stmts = Vec::with_capacity(block.statements.len());
    for stmt in &block.statements {
        let Statement::Assign(dest, rval) = stmt;
        let mut new_rval = rval.clone();

        // Fold Use with projections to scalar constant
        if let Rvalue::Use(p) = &new_rval {
            if !p.projections.is_empty() {
                if let Some(cv) = resolve_place_const(p, &env) {
                    if let Some(rv) = cv.to_rvalue() {
                        new_rval = rv;
                        folded += 1;
                    }
                }
            }
        }

        // Try to evaluate whole rvalue
        if let Some(cv) = eval_rvalue_const(&new_rval, &env) {
            if let Some(rv) = cv.to_rvalue() {
                if new_rval != rv {
                    new_rval = rv;
                    folded += 1;
                }
            }
            if dest.projections.is_empty() {
                env.insert(dest.local.clone(), cv);
            } else {
                apply_store_to_env(dest, &cv, &mut env);
            }
            new_stmts.push(Statement::Assign(dest.clone(), new_rval));
            continue;
        }

        // If projected Use yields array/struct, keep env knowledge
        if let Rvalue::Use(p) = &new_rval {
            if !p.projections.is_empty() {
                if let Some(cv) = resolve_place_const(p, &env).cloned() {
                    if dest.projections.is_empty() {
                        env.insert(dest.local.clone(), cv.clone());
                        if let Some(rv) = cv.to_rvalue() {
                            new_stmts.push(Statement::Assign(dest.clone(), rv));
                            folded += 1;
                            continue;
                        }
                    }
                }
            }
        }

        // Update env for destination
        if dest.projections.is_empty() {
            if let Some(cv) = eval_rvalue_const(&new_rval, &env) {
                env.insert(dest.local.clone(), cv);
            } else {
                env.remove(&dest.local);
            }
        } else if let Some(cv) = eval_rvalue_const(&new_rval, &env) {
            apply_store_to_env(dest, &cv, &mut env);
        } else {
            invalidate_dest_root(dest, &mut env);
        }

        new_stmts.push(Statement::Assign(dest.clone(), new_rval));
    }
    block.statements = new_stmts;
    folded
}

fn apply_store_to_env(dest: &Place, val: &ConstVal, env: &mut HashMap<String, ConstVal>) {
    if dest.projections.is_empty() {
        env.insert(dest.local.clone(), val.clone());
        return;
    }
    let Some(root) = env.get(&dest.local).cloned() else { return };
    let Some(updated) = set_const_at_projections(&root, &dest.projections, 0, val, env) else {
        env.remove(&dest.local);
        return;
    };
    env.insert(dest.local.clone(), updated);
}

fn set_const_at_projections(
    cur: &ConstVal,
    projections: &[Projection],
    pos: usize,
    val: &ConstVal,
    env: &HashMap<String, ConstVal>,
) -> Option<ConstVal> {
    if pos >= projections.len() {
        return Some(val.clone());
    }
    match &projections[pos] {
        Projection::Index(idx) => {
            let idx_val = if idx.projections.is_empty() {
                env.get(&idx.local)?.as_i64()?
            } else {
                resolve_place_const(idx, env)?.as_i64()?
            };
            if idx_val < 0 {
                return None;
            }
            let ConstVal::Array(items) = cur else { return None };
            let mut new_items = items.clone();
            let i = idx_val as usize;
            if i >= new_items.len() {
                return None;
            }
            new_items[i] = set_const_at_projections(&new_items[i], projections, pos + 1, val, env)?;
            Some(ConstVal::Array(new_items))
        }
        Projection::Field(name) => {
            let ConstVal::Struct(sname, fields) = cur else { return None };
            let mut new_fields = fields.clone();
            let idx = new_fields.iter().position(|(n, _)| n == name)?;
            let updated = set_const_at_projections(&new_fields[idx].1, projections, pos + 1, val, env)?;
            new_fields[idx].1 = updated;
            Some(ConstVal::Struct(sname.clone(), new_fields))
        }
        Projection::Deref | Projection::Payload(_) => None,
    }
}

fn invalidate_dest_root(dest: &Place, env: &mut HashMap<String, ConstVal>) {
    if dest.projections.is_empty() {
        env.remove(&dest.local);
    } else {
        // conservative: invalidate whole root
        env.remove(&dest.local);
    }
}

/// Forward constant propagation and folding for scalars, arrays, and structs.
/// Returns the number of folded expressions/stores.
pub fn const_propagate_mir_function(func: &mut MirFunction) -> usize {
    let mut total = 0usize;
    let global_env = collect_global_single_assign_consts(func);
    for block in &mut func.blocks {
        total += const_propagate_block(block, &global_env);
    }
    total += dce_mir_function(func);
    total
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

fn rvalue_has_side_effects(rval: &Rvalue) -> bool {
    matches!(
        rval,
        Rvalue::Call(_, _)
            | Rvalue::Load(_)
            | Rvalue::Alloc(_)
            | Rvalue::Thunk { .. }
    )
}

fn count_local_reads(func: &MirFunction) -> HashMap<String, usize> {
    let mut reads: HashMap<String, usize> = HashMap::new();
    let mut bump = |p: &Place| {
        *reads.entry(p.local.clone()).or_default() += 1;
        for proj in &p.projections {
            if let Projection::Index(idx) = proj {
                *reads.entry(idx.local.clone()).or_default() += 1;
                for ip in &idx.projections {
                    if let Projection::Index(ii) = ip {
                        *reads.entry(ii.local.clone()).or_default() += 1;
                    }
                }
            }
        }
    };
    for block in &func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(_dest, rval) = stmt;
            match rval {
                Rvalue::Use(p) | Rvalue::UnaryOp(_, p) | Rvalue::Discriminant(p)
                | Rvalue::Alloc(p) | Rvalue::Load(p) => bump(p),
                Rvalue::BinaryOp(_, l, r) => { bump(l); bump(r); }
                Rvalue::Call(_, args) | Rvalue::Array(args) => { for a in args { bump(a); } }
                Rvalue::Struct(_, fields) => { for (_, p) in fields { bump(p); } }
                Rvalue::EnumVariant { fields, .. } => { for p in fields { bump(p); } }
                Rvalue::Phi(incoming) => { for (_, p) in incoming { bump(p); } }
                Rvalue::ClosureAlloc { captured, .. } => { for p in captured { bump(p); } }
                Rvalue::Thunk { .. } | Rvalue::Constant(_) | Rvalue::FnPtr(_) => {}
            }
            // reads in dest projections
            let Statement::Assign(dest, _) = stmt;
            for proj in &dest.projections {
                if let Projection::Index(idx) = proj {
                    bump(idx);
                }
            }
        }
        // terminator reads
        match &block.terminator {
            crate::mir::Terminator::BranchIf { condition, .. } => bump(condition),
            crate::mir::Terminator::Switch { value, .. } => bump(value),
            crate::mir::Terminator::Return { value: Some(v) } => bump(v),
            crate::mir::Terminator::IndirectCall { callee, args, .. } => {
                bump(callee);
                for a in args { bump(a); }
            }
            _ => {}
        }
    }
    reads
}

/// Simple dead-assignment elimination. Removes pure assignments to locals that are never read.
/// Also removes dead indexed stores to locals that are never read at all.
pub fn dce_mir_function(func: &mut MirFunction) -> usize {
    let mut removed = 0usize;
    for _ in 0..4 {
        let reads = count_local_reads(func);
        let mut changed = 0usize;
        for block in &mut func.blocks {
            let before = block.statements.len();
            block.statements.retain(|stmt| {
                let Statement::Assign(dest, rval) = stmt;
                if rvalue_has_side_effects(rval) {
                    return true;
                }
                let read_count = reads.get(&dest.local).copied().unwrap_or(0);
                if dest.projections.is_empty() {
                    // dead plain assignment
                    read_count > 0
                } else {
                    // indexed/field store: dead if base local never read
                    read_count > 0
                }
            });
            changed += before - block.statements.len();
        }
        removed += changed;
        if changed == 0 {
            break;
        }
    }
    removed
}

/// Performs strength reduction across all basic blocks of a MirFunction.
/// Returns the number of operations reduced.
pub fn strength_reduce_mir_function(func: &mut MirFunction) -> usize {
    let mut reductions = 0;
    reductions += const_propagate_mir_function(func);
    reductions += dce_mir_function(func);
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
                            let var_ty = local_types.get(&x.local).cloned().unwrap_or(Type::I64);

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
                                    new_stmts.push(Statement::Assign(dest.clone(), Rvalue::Use(x)));
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
                                        Rvalue::Constant(TypedLiteral::Int(
                                            k as i64,
                                            var_ty.clone(),
                                        )),
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
                                        Rvalue::Constant(TypedLiteral::Int(
                                            k as i64,
                                            var_ty.clone(),
                                        )),
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
                                            Rvalue::Constant(TypedLiteral::Int(
                                                a as i64,
                                                var_ty.clone(),
                                            )),
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
                                            Rvalue::Constant(TypedLiteral::Int(
                                                a as i64,
                                                var_ty.clone(),
                                            )),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            b_place.clone(),
                                            Rvalue::Constant(TypedLiteral::Int(
                                                b as i64,
                                                var_ty.clone(),
                                            )),
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
                                            Rvalue::Constant(TypedLiteral::Int(
                                                a as i64,
                                                var_ty.clone(),
                                            )),
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
                                            Rvalue::Constant(TypedLiteral::Int(
                                                a as i64,
                                                var_ty.clone(),
                                            )),
                                        ));
                                        new_stmts.push(Statement::Assign(
                                            b_place.clone(),
                                            Rvalue::Constant(TypedLiteral::Int(
                                                b as i64,
                                                var_ty.clone(),
                                            )),
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
                            let var_ty = local_types.get(&left.local).cloned().unwrap_or(Type::I64);

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
                                        Rvalue::Constant(TypedLiteral::Int(
                                            k as i64,
                                            var_ty.clone(),
                                        )),
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
    reductions += dce_mir_function(func);
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
