//! Phase 64: Post-Collapse Residual MIR Strength Reduction.
//!
//! Scans residual MIR basic blocks and performs strength reduction on expensive operations:
//! 1. Power-of-2 multiplication: `x * 2^k` -> `x << k` (STRENGTH-02)
//! 2. Near-power-of-2 multiplication: `x * (2^a ± 2^b)` -> shift + add/sub sequences (STRENGTH-03)
//! 3. Power-of-2 division: `x / 2^k` -> `x >> k` (STRENGTH-04)
//! 4. Identity operations: `x * 0 -> 0`, `x * 1 -> x`, `x * -1 -> -x`, `x / 1 -> x`.

use std::collections::{HashMap, HashSet};

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
                Rvalue::Select(c, t, e) => { bump(c); bump(t); bump(e); }
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

/// Count the number of CFG edges targeting `target`.
fn count_predecessors(func: &MirFunction, target: crate::mir::BasicBlockId) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| b.terminator.successors())
        .filter(|t| *t == target)
        .count()
}

/// Turn a simple value-producing Rvalue into a place available at the branch block.
fn select_operand(
    func: &mut MirFunction,
    block_idx: usize,
    rv: &Rvalue,
    ty: &Type,
    counter: &mut usize,
) -> Option<Place> {
    match rv {
        Rvalue::Use(p) => Some(p.clone()),
        Rvalue::Constant(lit) => {
            let name = loop {
                let candidate = format!("__sel_const_{}_{}", block_idx, *counter);
                *counter += 1;
                if !func.locals.iter().any(|l| l.name == candidate) {
                    break candidate;
                }
            };
            func.locals.push(MirLocalDecl {
                name: name.clone(),
                ty: ty.clone(),
                mutable: true,
            });
            let place = Place { local: name, projections: vec![] };
            func.blocks[block_idx]
                .statements
                .push(Statement::Assign(place.clone(), Rvalue::Constant(lit.clone())));
            Some(place)
        }
        _ => None,
    }
}

/// Replace safe compare/assign diamonds with a branchless `Rvalue::Select`.
/// Conditions: both arms contain exactly one assignment to the same plain local,
/// values are a simple `Use` or constant, both arms branch to the same successor,
/// and neither arm has any other predecessor. No name matching is involved.
fn fold_simple_diamonds_to_select(func: &mut MirFunction) -> usize {
    let mut transformed = 0usize;
    let mut counter = 0usize;
    let mut block_idx = 0usize;
    while block_idx < func.blocks.len() {
        let (condition, then_target, else_target) = match &func.blocks[block_idx].terminator {
            crate::mir::Terminator::BranchIf { condition, then_target, else_target } => {
                (condition.clone(), then_target.clone(), else_target.clone())
            }
            _ => { block_idx += 1; continue; }
        };
        let then_idx = func.blocks.iter().position(|b| b.id == then_target);
        let else_idx = func.blocks.iter().position(|b| b.id == else_target);
        let (Some(ti), Some(ei)) = (then_idx, else_idx) else { block_idx += 1; continue; };
        if ti == ei || ti == block_idx || ei == block_idx { block_idx += 1; continue; }
        if func.blocks[ti].statements.len() != 1 || func.blocks[ei].statements.len() != 1 {
            block_idx += 1; continue;
        }
        if count_predecessors(func, then_target) != 1 || count_predecessors(func, else_target) != 1 {
            block_idx += 1; continue;
        }
        let common = match (&func.blocks[ti].terminator, &func.blocks[ei].terminator) {
            (crate::mir::Terminator::Branch { target: a }, crate::mir::Terminator::Branch { target: b }) if a == b => a.clone(),
            _ => { block_idx += 1; continue; }
        };
        let tstmt = func.blocks[ti].statements[0].clone();
        let estmt = func.blocks[ei].statements[0].clone();
        let (Statement::Assign(tdest, trval), Statement::Assign(edest, erval)) = (&tstmt, &estmt);
        if tdest.local != edest.local || !tdest.projections.is_empty() || !edest.projections.is_empty() {
            block_idx += 1; continue;
        }
        let dest_ty = func
            .locals
            .iter()
            .find(|l| l.name == tdest.local)
            .map(|l| l.ty.clone())
            .unwrap_or(Type::I64);
        let Some(tplace) = select_operand(func, block_idx, trval, &dest_ty, &mut counter) else {
            block_idx += 1; continue;
        };
        let Some(eplace) = select_operand(func, block_idx, erval, &dest_ty, &mut counter) else {
            block_idx += 1; continue;
        };
        func.blocks[block_idx].statements.push(Statement::Assign(
            tdest.clone(),
            Rvalue::Select(condition.clone(), tplace, eplace),
        ));
        func.blocks[block_idx].terminator = crate::mir::Terminator::Branch { target: common };
        func.blocks[ti].statements.clear();
        func.blocks[ti].terminator = crate::mir::Terminator::Unreachable;
        func.blocks[ei].statements.clear();
        func.blocks[ei].terminator = crate::mir::Terminator::Unreachable;
        transformed += 1;
        block_idx += 1;
    }
    transformed
}

/// Performs strength reduction across all basic blocks of a MirFunction.
/// Returns the number of operations reduced.
pub fn strength_reduce_mir_function(func: &mut MirFunction) -> usize {
    let mut reductions = 0;
    reductions += const_propagate_mir_function(func);
    reductions += dce_mir_function(func);
    reductions += fold_simple_diamonds_to_select(func);
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

/// Collect every local name mentioned by an rvalue, including projection
/// index expressions and phi incoming places.  Used by dead-loop elimination
/// to prove that a cycle's definitions never escape the cycle.
fn rvalue_local_reads(rv: &Rvalue, out: &mut HashSet<String>) {
    let mut bump = |p: &Place| {
        out.insert(p.local.clone());
        for proj in &p.projections {
            if let Projection::Index(idx) = proj {
                out.insert(idx.local.clone());
                for ip in &idx.projections {
                    if let Projection::Index(ii) = ip {
                        out.insert(ii.local.clone());
                    }
                }
            }
        }
    };
    match rv {
        Rvalue::Use(p) | Rvalue::UnaryOp(_, p) | Rvalue::Discriminant(p)
        | Rvalue::Alloc(p) | Rvalue::Load(p) => bump(p),
        Rvalue::BinaryOp(_, l, r) => { bump(l); bump(r); }
        Rvalue::Select(c, t, e) => { bump(c); bump(t); bump(e); }
        Rvalue::Call(_, args) | Rvalue::Array(args) => { for a in args { bump(a); } }
        Rvalue::Struct(_, fields) => { for (_, p) in fields { bump(p); } }
        Rvalue::EnumVariant { fields, .. } => { for p in fields { bump(p); } }
        Rvalue::Phi(incoming) => { for (_, p) in incoming { bump(p); } }
        Rvalue::ClosureAlloc { captured, .. } => { for p in captured { bump(p); } }
        Rvalue::Thunk { .. } | Rvalue::Constant(_) | Rvalue::FnPtr(_) => {}
    }
}

fn terminator_local_reads(term: &crate::mir::Terminator, out: &mut HashSet<String>) {
    use crate::mir::Terminator;
    match term {
        Terminator::BranchIf { condition, .. } => { out.insert(condition.local.clone()); }
        Terminator::Switch { value, .. } => { out.insert(value.local.clone()); }
        Terminator::Return { value: Some(v) } => { out.insert(v.local.clone()); }
        Terminator::IndirectCall { callee, args, .. } => {
            out.insert(callee.local.clone());
            for a in args { out.insert(a.local.clone()); }
        }
        Terminator::Force { thunk, .. } => { out.insert(thunk.clone()); }
        Terminator::TypeGuard { local, .. } => { out.insert(local.local.clone()); }
        Terminator::Branch { .. } | Terminator::Return { value: None }
        | Terminator::Unreachable | Terminator::Fork { .. } => {}
    }
}

/// Tarjan's strongly-connected-components algorithm over a successor graph.
fn tarjan_scc(succ: &[Vec<usize>]) -> Vec<Vec<usize>> {
    #[allow(clippy::too_many_arguments)]
    fn visit(
        v: usize,
        succ: &[Vec<usize>],
        index: &mut usize,
        indices: &mut [Option<usize>],
        low: &mut [usize],
        stack: &mut Vec<usize>,
        on_stack: &mut [bool],
        out: &mut Vec<Vec<usize>>,
    ) {
        indices[v] = Some(*index);
        low[v] = *index;
        *index += 1;
        stack.push(v);
        on_stack[v] = true;
        for &w in &succ[v] {
            if indices[w].is_none() {
                visit(w, succ, index, indices, low, stack, on_stack, out);
                low[v] = low[v].min(low[w]);
            } else if on_stack[w] {
                if let Some(iw) = indices[w] {
                    low[v] = low[v].min(iw);
                }
            }
        }
        if indices[v] == Some(low[v]) {
            let mut comp = Vec::new();
            while let Some(w) = stack.pop() {
                on_stack[w] = false;
                comp.push(w);
                if w == v { break; }
            }
            out.push(comp);
        }
    }
    let n = succ.len();
    let mut index = 0;
    let mut indices = vec![None; n];
    let mut low = vec![0usize; n];
    let mut stack = Vec::new();
    let mut on_stack = vec![false; n];
    let mut out = Vec::new();
    for v in 0..n {
        if indices[v].is_none() {
            visit(v, succ, &mut index, &mut indices, &mut low, &mut stack, &mut on_stack, &mut out);
        }
    }
    out
}

/// Redirect every edge that enters one of `members` to `new_target`.
fn redirect_edges_into(
    blocks: &mut [MirBasicBlock],
    members: &HashSet<usize>,
    new_target: crate::mir::BasicBlockId,
) {
    use crate::mir::Terminator;
    let member_ids: Vec<crate::mir::BasicBlockId> = members
        .iter()
        .map(|&i| blocks[i].id.clone())
        .collect();
    let enters = |t: &crate::mir::BasicBlockId| member_ids.contains(t);
    for (i, block) in blocks.iter_mut().enumerate() {
        if members.contains(&i) { continue; }
        match &mut block.terminator {
            Terminator::Branch { target } => {
                if enters(target) { *target = new_target.clone(); }
            }
            Terminator::BranchIf { then_target, else_target, .. } => {
                if enters(then_target) { *then_target = new_target.clone(); }
                if enters(else_target) { *else_target = new_target.clone(); }
            }
            Terminator::Switch { targets, default, .. } => {
                for (_, t) in targets.iter_mut() {
                    if enters(t) { *t = new_target.clone(); }
                }
                if enters(default) { *default = new_target.clone(); }
            }
            _ => {}
        }
    }
}

/// Eliminate control-flow cycles whose computation is no longer observable.
///
/// A cycle (strongly connected component with a back edge) can be replaced by
/// a direct branch when it has a single entry predecessor and a single exit
/// target, contains no side-effecting statements, and none of the locals it
/// defines are read outside the cycle (e.g. the producer loop for a value a
/// closed-form pass has since replaced by a constant).  This is a general MIR
/// peephole: it never inspects function or local names.
pub fn eliminate_dead_loops_mir_function(func: &mut MirFunction) -> usize {
    let mut total = 0usize;
    for _ in 0..4 {
        let n = func.blocks.len();
        let mut succ: Vec<Vec<usize>> = Vec::with_capacity(n);
        for block in &func.blocks {
            let targets = block.terminator.successors();
            succ.push(
                targets
                    .iter()
                    .filter_map(|t| func.blocks.iter().position(|b| b.id == *t))
                    .collect(),
            );
        }
        let mut removed_this_round = 0usize;
        for comp in tarjan_scc(&succ) {
            let is_cycle = comp.len() > 1 || (comp.len() == 1 && succ[comp[0]].contains(&comp[0]));
            if !is_cycle { continue; }
            let members: HashSet<usize> = comp.iter().copied().collect();

            // Single entry predecessor and single exit target.
            let mut entry_preds: HashSet<usize> = HashSet::new();
            let mut exit_targets: HashSet<usize> = HashSet::new();
            for (u, succ_u) in succ.iter().enumerate() {
                if members.contains(&u) { continue; }
                for &v in succ_u {
                    if members.contains(&v) { entry_preds.insert(u); }
                }
            }
            for &v in &comp {
                for &t in &succ[v] {
                    if !members.contains(&t) { exit_targets.insert(t); }
                }
            }
            if entry_preds.len() != 1 || exit_targets.len() != 1 { continue; }
            let Some(&exit_idx) = exit_targets.iter().next() else { continue; };
            let exit_id = func.blocks[exit_idx].id.clone();

            // No side effects and no projected stores inside the cycle.
            let mut safe = true;
            let mut defs: HashSet<String> = HashSet::new();
            for &i in &comp {
                let block = &func.blocks[i];
                if matches!(
                    block.terminator,
                    crate::mir::Terminator::IndirectCall { .. }
                        | crate::mir::Terminator::Force { .. }
                        | crate::mir::Terminator::Fork { .. }
                ) { safe = false; break; }
                for Statement::Assign(dest, rv) in &block.statements {
                    if rvalue_has_side_effects(rv) || !dest.projections.is_empty() {
                        safe = false;
                        break;
                    }
                    defs.insert(dest.local.clone());
                }
                if !safe { break; }
            }
            if !safe { continue; }

            // Definitions must not escape the cycle.  Phi predecessors in
            // outside blocks that mention a removed block are also disallowed.
            let mut reads_outside: HashSet<String> = HashSet::new();
            for (i, block) in func.blocks.iter().enumerate() {
                if members.contains(&i) { continue; }
                for Statement::Assign(_, rv) in &block.statements {
                    if let Rvalue::Phi(incoming) = rv {
                        if incoming.iter().any(|(bid, _)| comp.iter().any(|&m| func.blocks[m].id == *bid)) {
                            safe = false;
                            break;
                        }
                    }
                    rvalue_local_reads(rv, &mut reads_outside);
                }
                if !safe { break; }
                terminator_local_reads(&block.terminator, &mut reads_outside);
            }
            if !safe || defs.iter().any(|d| reads_outside.contains(d)) { continue; }

            // Safe: bypass the cycle and drop its blocks.
            redirect_edges_into(&mut func.blocks, &members, exit_id);
            for &i in &comp {
                func.blocks[i].statements.clear();
                func.blocks[i].terminator = crate::mir::Terminator::Unreachable;
            }
            removed_this_round += comp.len();
        }
        total += removed_this_round;
        if removed_this_round == 0 { break; }
    }
    if total > 0 {
        // The bypass can leave dead assignments to the cycle's former locals.
        dce_mir_function(func);
    }
    total
}

/// Algebraic peepholes that the closed-form emitter does not perform itself:
/// additive identities and removal of a modulo that is applied twice with the
/// same positive divisor.  All rewrites are semantics-preserving for wrapping
/// i64 arithmetic (`(x % m) % m == x % m` for m > 0).
pub fn algebraic_identity_peephole_mir_function(func: &mut MirFunction) -> usize {
    let mut assign_counts: HashMap<String, usize> = HashMap::new();
    for block in &func.blocks {
        for Statement::Assign(dest, _) in &block.statements {
            if dest.projections.is_empty() {
                *assign_counts.entry(dest.local.clone()).or_default() += 1;
            }
        }
    }
    let single: HashSet<String> = assign_counts
        .iter()
        .filter(|(_, &c)| c == 1)
        .map(|(n, _)| n.clone())
        .collect();
    let consts = collect_single_assignment_constants(func);

    // Locals that are the result of `% m` (m > 0) or a plain copy of such a
    // local, restricted to single-assignment locals so the fact stays valid.
    let mut mod_divisor: HashMap<String, i64> = HashMap::new();
    let mut copy_of: HashMap<String, String> = HashMap::new();
    for _ in 0..8 {
        let mut changed = false;
        for block in &func.blocks {
            for Statement::Assign(dest, rv) in &block.statements {
                if dest.projections.is_empty() && !single.contains(&dest.local) { continue; }
                match rv {
                    Rvalue::BinaryOp(BinaryOp::Mod, l, r) => {
                        if let Some((m, _)) = r.projections.is_empty().then(|| consts.get(&r.local)).flatten() {
                            if *m > 0 && !mod_divisor.contains_key(&dest.local) {
                                mod_divisor.insert(dest.local.clone(), *m);
                                changed = true;
                            }
                        }
                        let _ = l;
                    }
                    Rvalue::Use(p) if p.projections.is_empty() && !copy_of.contains_key(&dest.local) => {
                        copy_of.insert(dest.local.clone(), p.local.clone());
                        changed = true;
                    }
                    _ => {}
                }
            }
        }
        if !changed { break; }
    }

    fn resolve_mod_divisor(
        local: &str,
        mod_divisor: &HashMap<String, i64>,
        copy_of: &HashMap<String, String>,
    ) -> Option<i64> {
        let mut cur = local;
        let mut seen = HashSet::new();
        loop {
            if let Some(&m) = mod_divisor.get(cur) { return Some(m); }
            let next = copy_of.get(cur)?;
            if !seen.insert(cur.to_string()) { return None; }
            cur = next;
        }
    }

    let dest_types: HashMap<String, Type> = func
        .locals
        .iter()
        .map(|d| (d.name.clone(), d.ty.clone()))
        .collect();

    let mut folded = 0usize;
    for block in &mut func.blocks {
        let mut block_consts = consts.clone();
        for Statement::Assign(dest, rv) in &mut block.statements {
            let const_int = |p: &Place| -> Option<i64> {
                if p.projections.is_empty() { block_consts.get(&p.local).map(|(v, _)| *v) } else { None }
            };
            match rv {
                Rvalue::BinaryOp(BinaryOp::Add, l, r) => {
                    if const_int(r) == Some(0) {
                        *rv = Rvalue::Use(l.clone()); folded += 1;
                    } else if const_int(l) == Some(0) {
                        *rv = Rvalue::Use(r.clone()); folded += 1;
                    }
                }
                Rvalue::BinaryOp(BinaryOp::Sub, _l, r) => {
                    if const_int(r) == Some(0) {
                        *rv = Rvalue::Use(_l.clone()); folded += 1;
                    }
                }
                Rvalue::BinaryOp(BinaryOp::Mod, l, r) => {
                    if let Some(m) = const_int(r) {
                        if m == 1 {
                            let ty = dest_types.get(&dest.local).cloned().unwrap_or(Type::I64);
                            *rv = Rvalue::Constant(TypedLiteral::Int(0, ty)); folded += 1;
                        } else if m > 0
                            && l.projections.is_empty()
                            && resolve_mod_divisor(&l.local, &mod_divisor, &copy_of) == Some(m)
                        {
                            *rv = Rvalue::Use(l.clone()); folded += 1;
                        }
                    }
                }
                _ => {}
            }
            if let Rvalue::Constant(TypedLiteral::Int(v, ty)) = rv {
                if dest.projections.is_empty() {
                    block_consts.insert(dest.local.clone(), (*v, ty.clone()));
                }
            } else if let Rvalue::Use(p) = rv {
                if dest.projections.is_empty() {
                    if let Some((v, ty)) = block_consts.get(&p.local).cloned() {
                        block_consts.insert(dest.local.clone(), (v, ty));
                    } else {
                        block_consts.remove(&dest.local);
                    }
                }
            } else if dest.projections.is_empty() {
                block_consts.remove(&dest.local);
            }
        }
    }
    if folded > 0 { dce_mir_function(func); }
    folded
}

/// Final cleanup after the closed-form passes, which run after the normal
/// strength-reduction pipeline.  Removes producer loops made dead by a closed
/// form and simplifies the algebraic chain the closed-form emitter leaves.
pub fn post_collapse_cleanup_mir_program(program: &mut MirProgram) -> usize {
    let mut total = 0;
    for func in &mut program.functions {
        total += const_propagate_mir_function(func);
        total += eliminate_dead_loops_mir_function(func);
        // Run twice: the first pass folds `x + 0` into a copy, which the
        // second pass then recognises when chasing redundant-modulo chains.
        total += algebraic_identity_peephole_mir_function(func);
        total += algebraic_identity_peephole_mir_function(func);
        total += const_propagate_mir_function(func);
    }
    total
}

/// Applies strength reduction to every function in a MirProgram.
pub fn strength_reduce_mir_program(program: &mut MirProgram) -> usize {
    let mut total = 0;
    for func in &mut program.functions {
        total += strength_reduce_mir_function(func);
    }
    total
}
