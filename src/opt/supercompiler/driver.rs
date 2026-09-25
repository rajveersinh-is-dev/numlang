//! Symbolic driving for the NumLang supercompiler.
//!
//! Walks the typed AST under a partial environment, evaluating every
//! expression where all inputs are concretely known and leaving symbolic
//! residuals where they are not.
//!
//! The driving of `While` loops is the heart of the supercompiler:
//! it simulates iterations concretely, collects snapshots, and calls
//! the generalizer when the iteration cap or homeomorphic embedding fires.

use crate::ast::BinaryOp;
use crate::typecheck::typed_ast::{TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedProgram, TypedStmt};
use crate::typecheck::types::Type;
use super::env::Env;
use super::generalization::generalize_loop;
use super::termination::{detect_embedding, LoopSnapshot};
use super::value::{fold_binary, fold_binary_typed, fold_unary_typed, Value};
use std::collections::HashMap;

/// Hard cap on call stack depth.
const MAX_CALL_DEPTH: usize = 64;

/// The result of driving a block or statement.
#[derive(Debug)]
pub enum DriveResult {
    /// Block returned a value (via `return` statement).
    Returned(Box<Value>),
    /// Block hit `break`.
    Break,
    /// Block fell through without returning.
    Continue,
}

/// Drive a complete function with concrete argument values.
/// Returns the return value, or `Value::Symbolic` if driving failed.
pub fn drive_function(
    func: &TypedFunction,
    args: Vec<Value>,
    program: &TypedProgram,
    env: &mut Env,
) -> Value {
    if env.call_depth() >= MAX_CALL_DEPTH {
        env.mark_symbolic();
        return Value::Symbolic(super::value::SymExpr::Var(
            format!("_call_{}", func.name),
            func.return_ty.clone(),
        ));
    }

    // Check for recursive cycle
    if !env.push_call(func.name.clone(), args.clone()) {
        // Cycle detected — cannot evaluate symbolically
        env.mark_symbolic();
        return Value::Symbolic(super::value::SymExpr::Var(
            format!("_rec_{}", func.name),
            func.return_ty.clone(),
        ));
    }

    // Build child environment with params bound to args
    let params: Vec<(String, Value)> = func
        .params
        .iter()
        .zip(args)
        .map(|(p, v)| (p.name.clone(), v))
        .collect();

    let mut child_env = env.child_for_call(params);

    let result = drive_block(&func.body, program, &mut child_env);

    env.pop_call();

    if child_env.has_symbolic {
        env.mark_symbolic();
    }

    match result {
        DriveResult::Returned(v) => *v,
        _ => {
            env.mark_symbolic();
            Value::Symbolic(super::value::SymExpr::Var(
                format!("_sym_res_{}", func.name),
                func.return_ty.clone(),
            ))
        }
    }
}

/// Drive a block of statements sequentially.
pub fn drive_block(
    block: &TypedBlock,
    program: &TypedProgram,
    env: &mut Env,
) -> DriveResult {
    for stmt in &block.stmts {
        let res = drive_stmt(stmt, program, env);
        match res {
            DriveResult::Returned(_) | DriveResult::Break => return res,
            DriveResult::Continue => {}
        }
    }
    DriveResult::Continue
}

/// Drive a single statement.
pub fn drive_stmt(
    stmt: &TypedStmt,
    program: &TypedProgram,
    env: &mut Env,
) -> DriveResult {
    match stmt {
        TypedStmt::Let { name, value, .. } => {
            let val = drive_expr(value, program, env);
            env.set(name.clone(), val);
            DriveResult::Continue
        }

        TypedStmt::Assign { name, value, .. } => {
            let val = drive_expr(value, program, env);
            env.set(name.clone(), val);
            DriveResult::Continue
        }

        TypedStmt::IndexAssign { target, index, value, .. } => {
            let idx_val = drive_expr(index, program, env);
            let new_val = drive_expr(value, program, env);

            if let Some(idx) = idx_val.as_int() {
                if let Some(Value::Array(mut elems, ty)) = env.get(target).cloned() {
                    if idx >= 0 && (idx as usize) < elems.len() {
                        elems[idx as usize] = new_val;
                        env.set(target.clone(), Value::Array(elems, ty));
                        return DriveResult::Continue;
                    }
                }
            }
            // Cannot track — mark as symbolic
            env.mark_symbolic();
            env.set(
                target.clone(),
                Value::Symbolic(super::value::SymExpr::Var(
                    format!("_arr_{}", target),
                    Type::Array(Box::new(Type::I64), 0),
                )),
            );
            DriveResult::Continue
        }

        TypedStmt::Return(Some(expr), _) => {
            let val = drive_expr(expr, program, env);
            DriveResult::Returned(Box::new(val))
        }

        TypedStmt::Return(None, _) => DriveResult::Returned(Box::new(Value::Void)),

        TypedStmt::Break(_) => DriveResult::Break,

        TypedStmt::Expr(expr) => {
            drive_expr(expr, program, env);
            DriveResult::Continue
        }

        TypedStmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let cond_val = drive_expr(condition, program, env);
            match cond_val.as_bool() {
                Some(true) => drive_block(then_branch, program, env),
                Some(false) => {
                    if let Some(eb) = else_branch {
                        drive_block(eb, program, env)
                    } else {
                        DriveResult::Continue
                    }
                }
                None => {
                    // Symbolic condition — cannot know which branch was taken
                    env.mark_symbolic();
                    DriveResult::Continue
                }
            }
        }

        TypedStmt::While { condition, body, .. } => {
            drive_while(condition, body, program, env)
        }

        TypedStmt::Continue(..) => DriveResult::Continue,

        TypedStmt::For { var, lo, hi, inclusive, body, .. } => {
            let lo_val = drive_expr(lo, program, env);
            let hi_val = drive_expr(hi, program, env);
            if let (Some(lo_int), Some(hi_int)) = (lo_val.as_int(), hi_val.as_int()) {
                let limit = if *inclusive { hi_int + 1 } else { hi_int };
                let mut cur = lo_int;
                while cur < limit {
                    env.set(var.clone(), Value::Int(cur));
                    match drive_block(body, program, env) {
                        DriveResult::Returned(v) => return DriveResult::Returned(v),
                        DriveResult::Break => break,
                        DriveResult::Continue => {}
                    }
                    cur += 1;
                }
                DriveResult::Continue
            } else {
                env.mark_symbolic();
                DriveResult::Continue
            }
        }

        TypedStmt::FieldAssign { .. } => {
            env.mark_symbolic();
            DriveResult::Continue
        }
    }
}

/// Drive an expression to a Value.
pub fn drive_expr(
    expr: &TypedExpr,
    program: &TypedProgram,
    env: &mut Env,
) -> Value {
    match expr {
        TypedExpr::Literal { lit, .. } => match lit {
            TypedLiteral::Int(i, _) => Value::Int(*i),
            TypedLiteral::Float(f, _) => Value::Float(*f),
            TypedLiteral::Bool(b) => Value::Bool(*b),
            TypedLiteral::Str(_) => Value::Symbolic(super::value::SymExpr::Var(
                "_str".to_string(),
                crate::typecheck::types::Type::Str,
            )),
        },

        TypedExpr::Ident { name, ty, .. } => {
            match env.get(name).cloned() {
                Some(v) => v,
                None => {
                    env.mark_symbolic();
                    Value::Symbolic(super::value::SymExpr::Var(name.clone(), ty.clone()))
                }
            }
        }

        TypedExpr::Unary { op, expr, ty, .. } => {
            let val = drive_expr(expr, program, env);
            fold_unary_typed(*op, val, Some(ty))
        }

        TypedExpr::Binary { op, left, right, .. } => {
            let op_ty = left.ty();
            let lval = drive_expr(left, program, env);
            let rval = drive_expr(right, program, env);
            fold_binary_typed(*op, lval, rval, Some(&op_ty))
        }

        TypedExpr::Call { callee, args, ty, .. } => {
            // Drive all arguments
            let arg_vals: Vec<Value> = args.iter().map(|a| drive_expr(a, program, env)).collect();

            // Handle numeric intrinsics that aren't in the program's function list
            if let Some(result) = drive_intrinsic(callee, &arg_vals, ty) {
                return result;
            }

            // Look up user-defined function
            let func = program.functions.iter().find(|f| f.name == *callee);
            let func = match func {
                Some(f) => f.clone(),
                None => {
                    // Unknown external function — return symbolic
                    env.mark_symbolic();
                    return Value::Symbolic(super::value::SymExpr::Var(
                        format!("_ext_{}", callee),
                        ty.clone(),
                    ));
                }
            };

            drive_function(&func, arg_vals, program, env)
        }

        TypedExpr::ArrayLiteral { elements, ty, .. } => {
            let vals: Vec<Value> = elements.iter().map(|e| drive_expr(e, program, env)).collect();
            let elem_ty = match ty {
                Type::Array(et, _) => *et.clone(),
                _ => Type::I64,
            };
            Value::Array(vals, elem_ty)
        }

        TypedExpr::Index { target, index, ty, .. } => {
            let arr_val = drive_expr(target, program, env);
            let idx_val = drive_expr(index, program, env);

            if let (Value::Array(elems, _), Some(i)) = (&arr_val, idx_val.as_int()) {
                if i >= 0 && (i as usize) < elems.len() {
                    return elems[i as usize].clone();
                }
            }

            // Out-of-bounds or symbolic index
            env.mark_symbolic();
            Value::Symbolic(super::value::SymExpr::Var(
                "_idx".to_string(),
                ty.clone(),
            ))
        }

        TypedExpr::StructLiteral { .. } | TypedExpr::FieldAccess { .. } => {
            env.mark_symbolic();
            Value::Symbolic(super::value::SymExpr::Var("_struct".to_string(), expr.ty()))
        }

        TypedExpr::Match { scrutinee, arms, ty, .. } => {
            let scrut_val = drive_expr(scrutinee, program, env);
            match scrut_val {
                Value::Int(n) => {
                    for arm in arms {
                        for pat in &arm.patterns {
                            match pat {
                                crate::typecheck::typed_ast::TypedMatchPattern::Literal(crate::typecheck::typed_ast::TypedLiteral::Int(v, _)) if *v == n => {
                                    return drive_expr(&arm.body, program, env);
                                }
                                crate::typecheck::typed_ast::TypedMatchPattern::Wildcard => {
                                    return drive_expr(&arm.body, program, env);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Value::Bool(b) => {
                    for arm in arms {
                        for pat in &arm.patterns {
                            match pat {
                                crate::typecheck::typed_ast::TypedMatchPattern::Literal(crate::typecheck::typed_ast::TypedLiteral::Bool(v)) if *v == b => {
                                    return drive_expr(&arm.body, program, env);
                                }
                                crate::typecheck::typed_ast::TypedMatchPattern::Wildcard => {
                                    return drive_expr(&arm.body, program, env);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Value::Constructor {
                    ref variant_name,
                    tag,
                    ref fields,
                    ..
                } => {
                    for arm in arms {
                        for pat in &arm.patterns {
                            match pat {
                                crate::typecheck::typed_ast::TypedMatchPattern::Variant {
                                    variant_name: pat_var_name,
                                    tag: pat_tag,
                                    bindings,
                                    ..
                                } if pat_var_name == variant_name || *pat_tag == tag => {
                                    let mut arm_env = env.clone();
                                    for (i, (b_name, _b_ty)) in bindings.iter().enumerate() {
                                        if b_name != "_" {
                                            if let Some(field_val) = fields.get(i) {
                                                arm_env.set(b_name.clone(), field_val.clone());
                                            }
                                        }
                                    }
                                    let res = drive_expr(&arm.body, program, &mut arm_env);
                                    if arm_env.has_symbolic {
                                        env.mark_symbolic();
                                    }
                                    return res;
                                }
                                crate::typecheck::typed_ast::TypedMatchPattern::Wildcard => {
                                    return drive_expr(&arm.body, program, env);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
            env.mark_symbolic();
            Value::Symbolic(super::value::SymExpr::Var("_match".to_string(), ty.clone()))
        }

        TypedExpr::EnumConstructor {
            enum_name,
            variant_name,
            tag,
            args,
            ty,
            ..
        } => {
            let field_vals: Vec<Value> =
                args.iter().map(|a| drive_expr(a, program, env)).collect();
            Value::Constructor {
                enum_name: enum_name.clone(),
                variant_name: variant_name.clone(),
                tag: *tag,
                fields: field_vals,
                ty: ty.clone(),
            }
        }

        TypedExpr::Lambda { params, body, captured: captured_vars, .. } => {
            let mut captured = std::collections::HashMap::new();
            for (name, _ty) in captured_vars {
                if let Some(val) = env.get(name) {
                    captured.insert(name.clone(), val.clone());
                }
            }
            Value::Closure {
                params: params.iter().map(|(n, _)| n.clone()).collect(),
                body: *body.clone(),
                captured,
            }
        }

        TypedExpr::CallIndirect { callee, args, ty, .. } => {
            let callee_val = drive_expr(callee, program, env);
            match callee_val {
                Value::Closure { params, body, captured } => {
                    let arg_vals: Vec<Value> = args.iter()
                        .map(|a| drive_expr(a, program, env))
                        .collect();
                    let mut closure_bindings: Vec<(String, Value)> = captured.into_iter().collect();
                    for (p, v) in params.iter().zip(arg_vals) {
                        closure_bindings.push((p.clone(), v));
                    }
                    let mut closure_env = env.child_for_call(closure_bindings);
                    let result = drive_expr(&body, program, &mut closure_env);
                    if closure_env.has_symbolic {
                        env.mark_symbolic();
                    }
                    result
                }
                _ => {
                    env.mark_symbolic();
                    Value::Symbolic(super::value::SymExpr::Var(
                        "_indirect_call".to_string(),
                        ty.clone(),
                    ))
                }
            }
        }

        TypedExpr::Box { inner, .. } => {
            // In the symbolic driver, box is evaluated directly to its inner value
            drive_expr(inner, program, env)
        }

        TypedExpr::Deref { inner, .. } => {
            // Deref transparently evaluates the inner boxed value
            drive_expr(inner, program, env)
        }
    }
}

/// Drive a while loop.
/// Simulates iterations, collects snapshots during window, and calls the generalizer.
/// If generalizer succeeds, jumps to closed form.
/// If generalizer does not apply, but state is concrete, completes simulation directly.
fn extract_induction_var(
    condition: &TypedExpr,
    env: &Env,
    program: &TypedProgram,
) -> Option<(String, i64, BinaryOp)> {
    let (op, left, right) = match condition {
        TypedExpr::Binary { op, left, right, .. } => (*op, left.as_ref(), right.as_ref()),
        _ => return None,
    };
    if let TypedExpr::Ident { name, .. } = left {
        let lim = drive_expr(right, program, &mut env.clone()).as_int()?;
        return Some((name.clone(), lim, op));
    }
    None
}

fn find_step_jump(
    block: &TypedBlock,
    program: &TypedProgram,
    env: &Env,
    env1: &Env,
) -> Option<i64> {
    let mut min_jump = i64::MAX;
    scan_block_for_div_jumps(block, program, env, env1, &mut min_jump);
    if min_jump == i64::MAX { None } else { Some(min_jump) }
}

fn scan_block_for_div_jumps(
    block: &TypedBlock,
    program: &TypedProgram,
    env: &Env,
    env1: &Env,
    min_jump: &mut i64,
) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { value, .. } | TypedStmt::Assign { value, .. } | TypedStmt::Expr(value) => {
                scan_expr_for_div_jumps(value, program, env, env1, min_jump);
            }
            TypedStmt::If { then_branch, else_branch, .. } => {
                scan_block_for_div_jumps(then_branch, program, env, env1, min_jump);
                if let Some(eb) = else_branch {
                    scan_block_for_div_jumps(eb, program, env, env1, min_jump);
                }
            }
            TypedStmt::While { body, .. } => {
                scan_block_for_div_jumps(body, program, env, env1, min_jump);
            }
            _ => {}
        }
    }
}

fn scan_expr_for_div_jumps(
    expr: &TypedExpr,
    program: &TypedProgram,
    env: &Env,
    env1: &Env,
    min_jump: &mut i64,
) {
    match expr {
        TypedExpr::Binary { op: BinaryOp::Div, left, right, .. } => {
            let l0 = drive_expr(left, program, &mut env.clone()).as_int();
            let r0 = drive_expr(right, program, &mut env.clone()).as_int();
            let l1 = drive_expr(left, program, &mut env1.clone()).as_int();
            let r1 = drive_expr(right, program, &mut env1.clone()).as_int();
            if let (Some(l0), Some(r0), Some(l1), Some(r1)) = (l0, r0, l1, r1) {
                if r0 > 0 && r0 == r1 {
                    let dl = l1.wrapping_sub(l0);
                    if dl > 0 {
                        let q0 = l0 / r0;
                        let next_bound = (q0 + 1) * r0;
                        if next_bound > l0 {
                            let k = (next_bound - l0 + dl - 1) / dl;
                            if k > 0 && k < *min_jump {
                                *min_jump = k;
                            }
                        }
                    } else if dl < 0 {
                        let q0 = l0 / r0;
                        let prev_bound = q0 * r0 - 1;
                        if l0 > prev_bound {
                            let abs_dl = -dl;
                            let k = (l0 - prev_bound + abs_dl - 1) / abs_dl;
                            if k > 0 && k < *min_jump {
                                *min_jump = k;
                            }
                        }
                    }
                }
            }
            scan_expr_for_div_jumps(left, program, env, env1, min_jump);
            scan_expr_for_div_jumps(right, program, env, env1, min_jump);
        }
        TypedExpr::Binary { left, right, .. } => {
            scan_expr_for_div_jumps(left, program, env, env1, min_jump);
            scan_expr_for_div_jumps(right, program, env, env1, min_jump);
        }
        TypedExpr::Unary { expr, .. } => {
            scan_expr_for_div_jumps(expr, program, env, env1, min_jump);
        }
        _ => {}
    }
}

fn try_leapfrog_step(
    body: &TypedBlock,
    condition: &TypedExpr,
    program: &TypedProgram,
    env: &mut Env,
    loop_vars: &[String],
    var_modulos: &HashMap<String, i64>,
) -> Option<usize> {
    let (ind_var, limit, op) = extract_induction_var(condition, env, program)?;
    let v_i0 = env.get(&ind_var)?.as_int()?;

    let mut env1 = env.clone();
    let res1 = drive_block(body, program, &mut env1);
    if !matches!(res1, DriveResult::Continue) || env1.has_symbolic {
        return None;
    }
    let v_i1 = env1.get(&ind_var)?.as_int()?;
    let step_i = v_i1.wrapping_sub(v_i0);
    if step_i <= 0 {
        return None;
    }

    let jump = find_step_jump(body, program, env, &env1)?;
    if jump <= 2 {
        return None;
    }

    let rem_iters = compute_iters_for_step(op, v_i0, limit, step_i)?;
    let actual_jump = jump.min(rem_iters);
    if actual_jump <= 2 {
        return None;
    }

    let mut env2 = env1.clone();
    let res2 = drive_block(body, program, &mut env2);
    if !matches!(res2, DriveResult::Continue) || env2.has_symbolic {
        return None;
    }

    let mut deltas = HashMap::new();
    for var in loop_vars {
        let val0 = env.get(var)?.as_int()?;
        let val1 = env1.get(var)?.as_int()?;
        let val2 = env2.get(var)?.as_int()?;
        let d1 = val1.wrapping_sub(val0);
        let d2 = val2.wrapping_sub(val1);
        if d1 != d2 {
            return None;
        }
        deltas.insert(var.clone(), d1);
    }

    for var in loop_vars {
        let val0 = env.get(var)?.as_int()?;
        let dv = *deltas.get(var)?;
        let val_new = if let Some(&modulus) = var_modulos.get(var) {
            let j_mod = actual_jump % modulus;
            let term = (j_mod.wrapping_mul(dv % modulus)) % modulus;
            let total = (val0 % modulus).wrapping_add(term);
            ((total % modulus) + modulus) % modulus
        } else {
            val0.wrapping_add(actual_jump.wrapping_mul(dv))
        };
        env.set(var.clone(), Value::Int(val_new));
    }

    Some(actual_jump as usize)
}

fn drive_while(
    condition: &TypedExpr,
    body: &TypedBlock,
    program: &TypedProgram,
    env: &mut Env,
) -> DriveResult {
    let loop_vars: Vec<String> = collect_assigned_names_in_block(body);
    let var_modulos = collect_var_modulos(body, env);
    let can_generalize = !has_return_or_break(body) && !block_contains_transcendentals(body);

    let mut snapshots: Vec<LoopSnapshot> = Vec::new();
    let mut iter_count: usize = 0;

    snapshots.push(LoopSnapshot {
        state: env.snapshot_vars(&loop_vars),
        iteration: 0,
    });

    const SNAPSHOT_LIMIT: usize = 256;
    const MAX_CONCRETE_STEPS: usize = 100_000;

    loop {
        let cond_val = drive_expr(condition, program, env);
        match cond_val.as_bool() {
            Some(false) => {
                break;
            }
            None => {
                env.mark_symbolic();
                return DriveResult::Continue;
            }
            Some(true) => {}
        }

        // Check for closed form at powers of 2 or at SNAPSHOT_LIMIT (only if no early return/break and sufficient history)
        if can_generalize && iter_count >= 32 && (iter_count.is_power_of_two() || iter_count == SNAPSHOT_LIMIT) {
            let total_iters = infer_total_iters(condition, &snapshots, env, program);
            if total_iters.is_some_and(|tot| tot > iter_count as i64) {
                if let Some(final_state) = generalize_loop(&loop_vars, &snapshots, total_iters, &var_modulos) {
                    env.restore_vars(&final_state);
                    return DriveResult::Continue;
                } else if iter_count == SNAPSHOT_LIMIT && total_iters.is_some_and(|tot| tot > MAX_CONCRETE_STEPS as i64) {
                    env.mark_symbolic();
                    return DriveResult::Continue;
                }
            }
        }

        if can_generalize {
            if let Some(jump) = try_leapfrog_step(body, condition, program, env, &loop_vars, &var_modulos) {
                iter_count += jump;
                continue;
            }
        }

        let result = drive_block(body, program, env);
        match result {
            DriveResult::Returned(v) => return DriveResult::Returned(v),
            DriveResult::Break => break,
            DriveResult::Continue => {}
        }

        iter_count += 1;

        if iter_count <= SNAPSHOT_LIMIT {
            let snap = LoopSnapshot {
                state: env.snapshot_vars(&loop_vars),
                iteration: iter_count,
            };

            if can_generalize && iter_count >= 32 && detect_embedding(&snapshots, &snap) {
                let total_iters = infer_total_iters(condition, &snapshots, env, program);
                if total_iters.is_some_and(|tot| tot > iter_count as i64) {
                    if let Some(final_state) = generalize_loop(&loop_vars, &snapshots, total_iters, &var_modulos) {
                        env.restore_vars(&final_state);
                        return DriveResult::Continue;
                    }
                }
            }

            snapshots.push(snap);
        } else {
            // Beyond snapshot limit: free snapshot memory
            if !snapshots.is_empty() {
                snapshots.clear();
                snapshots.shrink_to_fit();
            }

            // Stop if tainted or exceeded step budget
            if env.has_symbolic || iter_count >= MAX_CONCRETE_STEPS {
                env.mark_symbolic();
                return DriveResult::Continue;
            }
        }
    }

    DriveResult::Continue
}

/// Collect modulo constraints applied to variables inside a loop block.
pub fn collect_var_modulos(block: &TypedBlock, env: &Env) -> HashMap<String, i64> {
    let mut modulos = HashMap::new();
    scan_block_modulos(block, env, &mut modulos);
    modulos
}

fn scan_block_modulos(block: &TypedBlock, env: &Env, modulos: &mut HashMap<String, i64>) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Assign { name, value, .. } | TypedStmt::Let { name, value, .. } => {
                if let TypedExpr::Binary { op: BinaryOp::Mod, right, .. } = value {
                    if let TypedExpr::Literal { lit: TypedLiteral::Int(m, _), .. } = &**right {
                        modulos.insert(name.clone(), *m);
                    } else if let TypedExpr::Ident { name: rname, .. } = &**right {
                        if let Some(Value::Int(m)) = env.get(rname) {
                            modulos.insert(name.clone(), *m);
                        }
                    }
                }
            }
            TypedStmt::If { then_branch, else_branch, .. } => {
                scan_block_modulos(then_branch, env, modulos);
                if let Some(eb) = else_branch {
                    scan_block_modulos(eb, env, modulos);
                }
            }
            TypedStmt::While { body, .. } => {
                scan_block_modulos(body, env, modulos);
            }
            _ => {}
        }
    }
}

fn has_return_or_break(block: &TypedBlock) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        TypedStmt::Return(_, _) | TypedStmt::Break(_) => true,
        TypedStmt::If { then_branch, else_branch, .. } => {
            has_return_or_break(then_branch) || else_branch.as_ref().is_some_and(has_return_or_break)
        }
        TypedStmt::While { body, .. } => has_return_or_break(body),
        _ => false,
    })
}

fn block_contains_transcendentals(block: &TypedBlock) -> bool {
    block.stmts.iter().any(stmt_contains_transcendentals)
}

fn stmt_contains_transcendentals(stmt: &TypedStmt) -> bool {
    match stmt {
        TypedStmt::Let { value, .. } | TypedStmt::Assign { value, .. } | TypedStmt::Expr(value) => {
            expr_contains_transcendentals(value)
        }
        TypedStmt::If { then_branch, else_branch, .. } => {
            block_contains_transcendentals(then_branch)
                || else_branch.as_ref().is_some_and(block_contains_transcendentals)
        }
        TypedStmt::While { body, .. } => block_contains_transcendentals(body),
        _ => false,
    }
}

fn expr_contains_transcendentals(expr: &TypedExpr) -> bool {
    match expr {
        TypedExpr::Call { callee, args, .. } => {
            matches!(
                callee.as_str(),
                "sin" | "cos" | "tan" | "exp" | "ln" | "log2" | "log10" | "pow"
            ) || args.iter().any(expr_contains_transcendentals)
        }
        TypedExpr::Binary { left, right, .. } => {
            expr_contains_transcendentals(left) || expr_contains_transcendentals(right)
        }
        TypedExpr::Unary { expr, .. } => expr_contains_transcendentals(expr),
        _ => false,
    }
}

/// Infer total iterations of a while loop from condition and observed progression.
fn infer_total_iters(
    condition: &TypedExpr,
    snapshots: &[LoopSnapshot],
    env: &Env,
    program: &TypedProgram,
) -> Option<i64> {
    if snapshots.len() < 2 {
        return None;
    }

    let (op, left, right) = match condition {
        TypedExpr::Binary { op, left, right, .. } => (*op, left.as_ref(), right.as_ref()),
        _ => return None,
    };

    let ident_name = |e: &TypedExpr| -> Option<String> {
        match e {
            TypedExpr::Ident { name, .. } => Some(name.clone()),
            _ => None,
        }
    };

    // Case 1: left is induction variable, right is limit
    if let Some(var_name) = ident_name(left) {
        let v0 = snapshots[0].state.get(&var_name)?.as_int()?;
        let v1 = snapshots[1].state.get(&var_name)?.as_int()?;
        let step = v1.wrapping_sub(v0);
        if step != 0 {
            // Try evaluating limit expression in current env or from snapshot
            let lim = drive_expr(right, program, &mut env.clone()).as_int()
                .or_else(|| snapshots[0].state.get(ident_name(right)?.as_str())?.as_int())?;
            return compute_iters_for_step(op, v0, lim, step);
        }
    }

    // Case 2: right is induction variable, left is limit
    if let Some(var_name) = ident_name(right) {
        let v0 = snapshots[0].state.get(&var_name)?.as_int()?;
        let v1 = snapshots[1].state.get(&var_name)?.as_int()?;
        let step = v1.wrapping_sub(v0);
        if step != 0 {
            let lim = drive_expr(left, program, &mut env.clone()).as_int()
                .or_else(|| snapshots[0].state.get(ident_name(left)?.as_str())?.as_int())?;
            let inv_op = match op {
                BinaryOp::Lt => BinaryOp::Gt,
                BinaryOp::Le => BinaryOp::Ge,
                BinaryOp::Gt => BinaryOp::Lt,
                BinaryOp::Ge => BinaryOp::Le,
                BinaryOp::Eq => BinaryOp::Eq,
                BinaryOp::Ne => BinaryOp::Ne,
                _ => return None,
            };
            return compute_iters_for_step(inv_op, v0, lim, step);
        }
    }

    None
}

fn compute_iters_for_step(op: BinaryOp, v0: i64, limit: i64, step: i64) -> Option<i64> {
    if step > 0 {
        match op {
            BinaryOp::Lt => {
                if limit <= v0 {
                    Some(0)
                } else {
                    Some((limit - v0 + step - 1) / step)
                }
            }
            BinaryOp::Le => {
                if limit < v0 {
                    Some(0)
                } else {
                    Some((limit - v0 + step) / step)
                }
            }
            BinaryOp::Ne => {
                if limit >= v0 && (limit - v0) % step == 0 {
                    Some((limit - v0) / step)
                } else {
                    None
                }
            }
            _ => None,
        }
    } else {
        let abs_step = -step;
        match op {
            BinaryOp::Gt => {
                if limit >= v0 {
                    Some(0)
                } else {
                    Some((v0 - limit + abs_step - 1) / abs_step)
                }
            }
            BinaryOp::Ge => {
                if limit > v0 {
                    Some(0)
                } else {
                    Some((v0 - limit + abs_step) / abs_step)
                }
            }
            BinaryOp::Ne => {
                if limit <= v0 && (v0 - limit) % abs_step == 0 {
                    Some((v0 - limit) / abs_step)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Collect all variable names assigned inside a block.
fn collect_assigned_names_in_block(block: &TypedBlock) -> Vec<String> {
    let mut names = std::collections::HashSet::new();
    collect_assigned_names(block, &mut names);
    let mut v: Vec<String> = names.into_iter().collect();
    v.sort();
    v
}

fn collect_assigned_names(block: &TypedBlock, names: &mut std::collections::HashSet<String>) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Assign { name, .. } => {
                names.insert(name.clone());
            }
            TypedStmt::IndexAssign { target, .. } => {
                names.insert(target.clone());
            }
            TypedStmt::Let { name, is_mutable, .. } if *is_mutable => {
                names.insert(name.clone());
            }
            TypedStmt::If { then_branch, else_branch, .. } => {
                collect_assigned_names(then_branch, names);
                if let Some(eb) = else_branch {
                    collect_assigned_names(eb, names);
                }
            }
            TypedStmt::While { body, .. } => {
                collect_assigned_names(body, names);
            }
            _ => {}
        }
    }
}

/// Check whether a function is impure (calls external I/O or modifying C routines).
pub fn is_impure(func: &TypedFunction) -> bool {
    calls_external_impure(&func.body)
}

fn calls_external_impure(block: &TypedBlock) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        TypedStmt::Expr(e) | TypedStmt::Let { value: e, .. } | TypedStmt::Assign { value: e, .. } => {
            expr_calls_external_impure(e)
        }
        TypedStmt::If { condition, then_branch, else_branch, .. } => {
            expr_calls_external_impure(condition)
                || calls_external_impure(then_branch)
                || else_branch.as_ref().is_some_and(calls_external_impure)
        }
        TypedStmt::While { condition, body, .. } => {
            expr_calls_external_impure(condition) || calls_external_impure(body)
        }
        TypedStmt::Return(Some(e), _) => expr_calls_external_impure(e),
        _ => false,
    })
}

fn expr_calls_external_impure(expr: &TypedExpr) -> bool {
    match expr {
        TypedExpr::Call { callee, args, .. } => {
            if is_known_pure_intrinsic(callee) {
                args.iter().any(expr_calls_external_impure)
            } else {
                callee == "print" || callee == "println" || callee == "exit"
            }
        }
        TypedExpr::Binary { left, right, .. } => {
            expr_calls_external_impure(left) || expr_calls_external_impure(right)
        }
        TypedExpr::Unary { expr, .. } => expr_calls_external_impure(expr),
        _ => false,
    }
}

pub fn is_known_pure_intrinsic(name: &str) -> bool {
    matches!(
        name,
        "abs" | "signum" | "isqrt" | "min" | "max" | "clamp" | "fma" | "hypot" | "lerp"
            | "gcd" | "lcm" | "sqrt" | "floor" | "ceil" | "round" | "trunc" | "sin" | "cos"
            | "tan" | "exp" | "ln" | "log2" | "log10" | "pow" | "popcnt" | "tzcnt" | "ctz"
            | "lzcnt" | "clz" | "rotl" | "rotr" | "dot" | "vec_norm" | "sum" | "vec_add"
            | "vec_sub" | "vec_mul" | "vec_scale" | "vec_div_scalar" | "mat_det2" | "mat_det3"
            | "mat_det4" | "mat_trace2" | "mat_trace3" | "mat_trace4" | "mat_inv2" | "mat_inv3"
            | "mat_inv4" | "mat_solve2" | "mat_solve3" | "mat_solve4" | "c_make" | "c_re"
            | "c_im" | "c_add" | "c_sub" | "c_mul" | "c_div" | "c_exp" | "c_abs" | "c_arg"
            | "c_conj" | "fft8" | "fft8_re" | "fft8_im" | "fft16" | "fft16_re" | "fft16_im"
            | "to_float" | "to_int" | "to_i64"
    )
}

/// Check whether a function directly calls itself.
pub fn is_recursive(func: &TypedFunction) -> bool {
    block_calls_function(&func.body, &func.name)
}

fn block_calls_function(block: &TypedBlock, name: &str) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        TypedStmt::Let { value, .. }
        | TypedStmt::Assign { value, .. }
        | TypedStmt::Expr(value) => expr_calls_function(value, name),
        TypedStmt::IndexAssign { index, value, .. } => {
            expr_calls_function(index, name) || expr_calls_function(value, name)
        }
        TypedStmt::Return(Some(e), _) => expr_calls_function(e, name),
        TypedStmt::If { condition, then_branch, else_branch, .. } => {
            expr_calls_function(condition, name)
                || block_calls_function(then_branch, name)
                || else_branch.as_ref().is_some_and(|eb| block_calls_function(eb, name))
        }
        TypedStmt::While { condition, body, .. } => {
            expr_calls_function(condition, name) || block_calls_function(body, name)
        }
        _ => false,
    })
}

fn expr_calls_function(expr: &TypedExpr, name: &str) -> bool {
    match expr {
        TypedExpr::Call { callee, args, .. } => {
            callee == name || args.iter().any(|a| expr_calls_function(a, name))
        }
        TypedExpr::Binary { left, right, .. } => {
            expr_calls_function(left, name) || expr_calls_function(right, name)
        }
        TypedExpr::Unary { expr, .. } => expr_calls_function(expr, name),
        TypedExpr::ArrayLiteral { elements, .. } => {
            elements.iter().any(|e| expr_calls_function(e, name))
        }
        TypedExpr::Index { target, index, .. } => {
            expr_calls_function(target, name) || expr_calls_function(index, name)
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Intrinsic Functions Evaluator
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn det3_helper(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64, g: f64, h: f64, i: f64) -> f64 {
    a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)
}

fn inv4_helper(m: &[f64; 16]) -> [f64; 16] {
    let c00 = det3_helper(m[5], m[6], m[7], m[9], m[10], m[11], m[13], m[14], m[15]);
    let c01 = -det3_helper(m[4], m[6], m[7], m[8], m[10], m[11], m[12], m[14], m[15]);
    let c02 = det3_helper(m[4], m[5], m[7], m[8], m[9], m[11], m[12], m[13], m[15]);
    let c03 = -det3_helper(m[4], m[5], m[6], m[8], m[9], m[10], m[12], m[13], m[14]);

    let c10 = -det3_helper(m[1], m[2], m[3], m[9], m[10], m[11], m[13], m[14], m[15]);
    let c11 = det3_helper(m[0], m[2], m[3], m[8], m[10], m[11], m[12], m[14], m[15]);
    let c12 = -det3_helper(m[0], m[1], m[3], m[8], m[9], m[11], m[12], m[13], m[15]);
    let c13 = det3_helper(m[0], m[1], m[2], m[8], m[9], m[10], m[12], m[13], m[14]);

    let c20 = det3_helper(m[1], m[2], m[3], m[5], m[6], m[7], m[13], m[14], m[15]);
    let c21 = -det3_helper(m[0], m[2], m[3], m[4], m[6], m[7], m[12], m[14], m[15]);
    let c22 = det3_helper(m[0], m[1], m[3], m[4], m[5], m[7], m[12], m[13], m[15]);
    let c23 = -det3_helper(m[0], m[1], m[2], m[4], m[5], m[6], m[12], m[13], m[14]);

    let c30 = -det3_helper(m[1], m[2], m[3], m[5], m[6], m[7], m[9], m[10], m[11]);
    let c31 = det3_helper(m[0], m[2], m[3], m[4], m[6], m[7], m[8], m[10], m[11]);
    let c32 = -det3_helper(m[0], m[1], m[3], m[4], m[5], m[7], m[8], m[9], m[11]);
    let c33 = det3_helper(m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]);

    let det = m[0] * c00 + m[1] * c01 + m[2] * c02 + m[3] * c03;
    let inv_det = 1.0 / det;

    [
        c00 * inv_det, c10 * inv_det, c20 * inv_det, c30 * inv_det,
        c01 * inv_det, c11 * inv_det, c21 * inv_det, c31 * inv_det,
        c02 * inv_det, c12 * inv_det, c22 * inv_det, c32 * inv_det,
        c03 * inv_det, c13 * inv_det, c23 * inv_det, c33 * inv_det,
    ]
}

fn fft_helper(re: &[f64], im: &[f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    let bits = if n == 8 { 3 } else { 4 };
    let mut r = vec![0.0; n];
    let mut i = vec![0.0; n];
    for k in 0..n {
        let mut rev = 0;
        for b in 0..bits {
            if (k & (1 << b)) != 0 {
                rev |= 1 << (bits - 1 - b);
            }
        }
        r[rev] = re[k];
        i[rev] = im[k];
    }
    let mut s = 1;
    while s < n {
        let len = s * 2;
        let mut group = 0;
        while group < n {
            for j in 0..s {
                let angle = -2.0 * std::f64::consts::PI * (j as f64) / (len as f64);
                let wr = angle.cos();
                let wi = angle.sin();
                let u_idx = group + j;
                let v_idx = group + j + s;
                let tr = wr * r[v_idx] - wi * i[v_idx];
                let ti = wr * i[v_idx] + wi * r[v_idx];
                r[v_idx] = r[u_idx] - tr;
                i[v_idx] = i[u_idx] - ti;
                r[u_idx] += tr;
                i[u_idx] += ti;
            }
            group += len;
        }
        s *= 2;
    }
    (r, i)
}

/// Handle numeric intrinsic functions that are not user-defined.
fn drive_intrinsic(name: &str, args: &[Value], _ty: &Type) -> Option<Value> {
    match name {
        "to_float" => {
            let i = args.first()?.as_float()?;
            Some(Value::Float(i))
        }
        "to_int" | "to_i64" => {
            let f = args.first()?.as_int().or_else(|| args.first()?.as_float().map(|x| x as i64))?;
            Some(Value::Int(f))
        }
        "abs" => {
            match args.first()? {
                Value::Int(i) => Some(Value::Int(i.wrapping_abs())),
                Value::Float(f) => Some(Value::Float(f.abs())),
                _ => None,
            }
        }
        "signum" => {
            match args.first()? {
                Value::Int(i) => {
                    let s = if *i > 0 { 1 } else if *i < 0 { -1 } else { 0 };
                    Some(Value::Int(s))
                }
                Value::Float(f) => {
                    let s = if *f > 0.0 { 1.0 } else if *f < 0.0 { -1.0 } else { 0.0 };
                    Some(Value::Float(s))
                }
                _ => None,
            }
        }
        "isqrt" => {
            let i = args.first()?.as_int()?;
            if i <= 0 {
                return Some(Value::Int(0));
            }
            let mut r = (i as f64).sqrt() as i64;
            while (r + 1).wrapping_mul(r + 1) <= i && r < 3037000499 {
                r += 1;
            }
            while r.wrapping_mul(r) > i {
                r -= 1;
            }
            Some(Value::Int(r))
        }
        "min" => {
            match (args.first()?, args.get(1)?) {
                (Value::Int(a), Value::Int(b)) => Some(Value::Int(*a.min(b))),
                (Value::Float(a), Value::Float(b)) => Some(Value::Float(a.min(*b))),
                _ => None,
            }
        }
        "max" => {
            match (args.first()?, args.get(1)?) {
                (Value::Int(a), Value::Int(b)) => Some(Value::Int(*a.max(b))),
                (Value::Float(a), Value::Float(b)) => Some(Value::Float(a.max(*b))),
                _ => None,
            }
        }
        "clamp" => {
            match (args.first()?, args.get(1)?, args.get(2)?) {
                (Value::Int(v), Value::Int(lo), Value::Int(hi)) => {
                    Some(Value::Int((*v).max(*lo).min(*hi)))
                }
                (Value::Float(v), Value::Float(lo), Value::Float(hi)) => {
                    Some(Value::Float(v.max(*lo).min(*hi)))
                }
                _ => None,
            }
        }
        "fma" => {
            let a = args.first()?.as_float()?;
            let b = args.get(1)?.as_float()?;
            let c = args.get(2)?.as_float()?;
            Some(Value::Float(a.mul_add(b, c)))
        }
        "hypot" => {
            let a = args.first()?.as_float()?;
            let b = args.get(1)?.as_float()?;
            Some(Value::Float(a.hypot(b)))
        }
        "lerp" => {
            let a = args.first()?.as_float()?;
            let b = args.get(1)?.as_float()?;
            let t = args.get(2)?.as_float()?;
            Some(Value::Float(a + t * (b - a)))
        }
        "gcd" => {
            let a = args.first()?.as_int()?.abs();
            let b = args.get(1)?.as_int()?.abs();
            let mut u = a;
            let mut v = b;
            while v != 0 {
                let t = v;
                v = u % v;
                u = t;
            }
            Some(Value::Int(u))
        }
        "lcm" => {
            let a = args.first()?.as_int()?.abs();
            let b = args.get(1)?.as_int()?.abs();
            if a == 0 || b == 0 {
                return Some(Value::Int(0));
            }
            let mut u = a;
            let mut v = b;
            while v != 0 {
                let t = v;
                v = u % v;
                u = t;
            }
            let g = u;
            Some(Value::Int((a / g) * b))
        }
        "sqrt"  => { let f = args.first()?.as_float()?; Some(Value::Float(f.sqrt())) }
        "floor" => { let f = args.first()?.as_float()?; Some(Value::Float(f.floor())) }
        "ceil"  => { let f = args.first()?.as_float()?; Some(Value::Float(f.ceil())) }
        "round" => { let f = args.first()?.as_float()?; Some(Value::Float(f.round())) }
        "trunc" => { let f = args.first()?.as_float()?; Some(Value::Float(f.trunc())) }
        "sin"   => { let f = args.first()?.as_float()?; Some(Value::Float(f.sin())) }
        "cos"   => { let f = args.first()?.as_float()?; Some(Value::Float(f.cos())) }
        "tan"   => { let f = args.first()?.as_float()?; Some(Value::Float(f.tan())) }
        "exp"   => { let f = args.first()?.as_float()?; Some(Value::Float(f.exp())) }
        "ln"    => { let f = args.first()?.as_float()?; Some(Value::Float(f.ln())) }
        "log2"  => { let f = args.first()?.as_float()?; Some(Value::Float(f.log2())) }
        "log10" => { let f = args.first()?.as_float()?; Some(Value::Float(f.log10())) }
        "pow" => {
            match (args.first()?, args.get(1)?) {
                (Value::Float(b), Value::Float(e)) => Some(Value::Float(b.powf(*e))),
                (Value::Float(b), Value::Int(e)) => Some(Value::Float(b.powi(*e as i32))),
                (Value::Int(b), Value::Int(e)) => {
                    if *e < 0 {
                        Some(Value::Int(0))
                    } else {
                        Some(Value::Int(b.wrapping_pow(*e as u32)))
                    }
                }
                _ => None,
            }
        }
        "popcnt" => {
            let i = args.first()?.as_int()?;
            Some(Value::Int(i.count_ones() as i64))
        }
        "bswap" => {
            let i = args.first()?.as_int()?;
            Some(Value::Int(i.swap_bytes()))
        }
        "i64_to_f64" => {
            let i = args.first()?.as_int()?;
            Some(Value::Float(i as f64))
        }
        "f64_to_i64" => {
            let f = args.first()?.as_float()?;
            Some(Value::Int(f as i64))
        }
        "i64_to_f32" => {
            let i = args.first()?.as_int()?;
            Some(Value::Float(i as f32 as f64))
        }
        "f32_to_f64" => {
            let f = args.first()?.as_float()?;
            Some(Value::Float(f as f32 as f64))
        }
        "f64_to_f32" => {
            let f = args.first()?.as_float()?;
            Some(Value::Float(f as f32 as f64))
        }
        "i64_to_i32" => {
            let i = args.first()?.as_int()?;
            Some(Value::Int(i as i32 as i64))
        }
        "i32_to_i64" => {
            let i = args.first()?.as_int()?;
            Some(Value::Int(i as i32 as i64))
        }
        "tzcnt" | "ctz" => {
            let i = args.first()?.as_int()?;
            Some(Value::Int(i.trailing_zeros() as i64))
        }
        "lzcnt" | "clz" => {
            let i = args.first()?.as_int()?;
            Some(Value::Int(i.leading_zeros() as i64))
        }
        "rotl" => {
            let val = args.first()?.as_int()?;
            let shift = (args.get(1)?.as_int()? & 63) as u32;
            Some(Value::Int(val.rotate_left(shift)))
        }
        "rotr" => {
            let val = args.first()?.as_int()?;
            let shift = (args.get(1)?.as_int()? & 63) as u32;
            Some(Value::Int(val.rotate_right(shift)))
        }
        "dot" => {
            if let (Value::Array(a, _), Value::Array(b, _)) = (args.first()?, args.get(1)?) {
                if a.len() == b.len() {
                    let is_float = a.first().is_some_and(|v| matches!(v, Value::Float(_)))
                        || b.first().is_some_and(|v| matches!(v, Value::Float(_)));
                    if is_float {
                        let mut sum = 0.0;
                        for (x, y) in a.iter().zip(b.iter()) {
                            let xf = x.as_float()?;
                            let yf = y.as_float()?;
                            sum += xf * yf;
                        }
                        return Some(Value::Float(sum));
                    } else {
                        let mut sum: i64 = 0;
                        for (x, y) in a.iter().zip(b.iter()) {
                            let xi = x.as_int()?;
                            let yi = y.as_int()?;
                            sum = sum.wrapping_add(xi.wrapping_mul(yi));
                        }
                        return Some(Value::Int(sum));
                    }
                }
            }
            None
        }
        "vec_norm" => {
            if let Value::Array(a, _) = args.first()? {
                let mut sum = 0.0;
                for x in a {
                    let xf = x.as_float()?;
                    sum += xf * xf;
                }
                return Some(Value::Float(sum.sqrt()));
            }
            None
        }
        "sum" => {
            if let Value::Array(a, _) = args.first()? {
                let is_float = a.first().is_some_and(|v| matches!(v, Value::Float(_)));
                if is_float {
                    let mut s = 0.0;
                    for x in a {
                        s += x.as_float()?;
                    }
                    return Some(Value::Float(s));
                } else {
                    let mut s: i64 = 0;
                    for x in a {
                        s = s.wrapping_add(x.as_int()?);
                    }
                    return Some(Value::Int(s));
                }
            }
            None
        }
        "vec_add" => {
            if let (Value::Array(a, ty), Value::Array(b, _)) = (args.first()?, args.get(1)?) {
                if a.len() == b.len() {
                    let mut res = Vec::new();
                    for (x, y) in a.iter().zip(b.iter()) {
                        res.push(fold_binary(BinaryOp::Add, x.clone(), y.clone()));
                    }
                    return Some(Value::Array(res, ty.clone()));
                }
            }
            None
        }
        "vec_sub" => {
            if let (Value::Array(a, ty), Value::Array(b, _)) = (args.first()?, args.get(1)?) {
                if a.len() == b.len() {
                    let mut res = Vec::new();
                    for (x, y) in a.iter().zip(b.iter()) {
                        res.push(fold_binary(BinaryOp::Sub, x.clone(), y.clone()));
                    }
                    return Some(Value::Array(res, ty.clone()));
                }
            }
            None
        }
        "vec_mul" => {
            if let (Value::Array(a, ty), Value::Array(b, _)) = (args.first()?, args.get(1)?) {
                if a.len() == b.len() {
                    let mut res = Vec::new();
                    for (x, y) in a.iter().zip(b.iter()) {
                        res.push(fold_binary(BinaryOp::Mul, x.clone(), y.clone()));
                    }
                    return Some(Value::Array(res, ty.clone()));
                }
            }
            None
        }
        "vec_scale" => {
            if let (Value::Array(a, ty), s) = (args.first()?, args.get(1)?) {
                let mut res = Vec::new();
                for x in a {
                    res.push(fold_binary(BinaryOp::Mul, x.clone(), s.clone()));
                }
                return Some(Value::Array(res, ty.clone()));
            }
            None
        }
        "vec_div_scalar" => {
            if let (Value::Array(a, ty), s) = (args.first()?, args.get(1)?) {
                let mut res = Vec::new();
                for x in a {
                    res.push(fold_binary(BinaryOp::Div, x.clone(), s.clone()));
                }
                return Some(Value::Array(res, ty.clone()));
            }
            None
        }
        "mat_det2" => {
            if let Value::Array(elems, _) = args.first()? {
                if elems.len() >= 4 {
                    let a = elems[0].as_float()?;
                    let b = elems[1].as_float()?;
                    let c = elems[2].as_float()?;
                    let d = elems[3].as_float()?;
                    return Some(Value::Float(a * d - b * c));
                }
            }
            None
        }
        "mat_det3" => {
            if let Value::Array(elems, _) = args.first()? {
                if elems.len() >= 9 {
                    let e: Vec<f64> = elems.iter().filter_map(|v| v.as_float()).collect();
                    if e.len() >= 9 {
                        return Some(Value::Float(det3_helper(e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7], e[8])));
                    }
                }
            }
            None
        }
        "mat_det4" => {
            if let Value::Array(elems, _) = args.first()? {
                if elems.len() >= 16 {
                    let m: Vec<f64> = elems.iter().filter_map(|v| v.as_float()).collect();
                    if m.len() >= 16 {
                        let c00 = det3_helper(m[5], m[6], m[7], m[9], m[10], m[11], m[13], m[14], m[15]);
                        let c01 = -det3_helper(m[4], m[6], m[7], m[8], m[10], m[11], m[12], m[14], m[15]);
                        let c02 = det3_helper(m[4], m[5], m[7], m[8], m[9], m[11], m[12], m[13], m[15]);
                        let c03 = -det3_helper(m[4], m[5], m[6], m[8], m[9], m[10], m[12], m[13], m[14]);
                        let det = m[0] * c00 + m[1] * c01 + m[2] * c02 + m[3] * c03;
                        return Some(Value::Float(det));
                    }
                }
            }
            None
        }
        "mat_trace2" => {
            if let Value::Array(elems, _) = args.first()? {
                if elems.len() >= 4 {
                    if let (Some(a), Some(b)) = (elems[0].as_float(), elems[3].as_float()) {
                        return Some(Value::Float(a + b));
                    }
                    if let (Some(a), Some(b)) = (elems[0].as_int(), elems[3].as_int()) {
                        return Some(Value::Int(a + b));
                    }
                }
            }
            None
        }
        "mat_trace3" => {
            if let Value::Array(elems, _) = args.first()? {
                if elems.len() >= 9 {
                    if let (Some(a), Some(b), Some(c)) = (elems[0].as_float(), elems[4].as_float(), elems[8].as_float()) {
                        return Some(Value::Float(a + b + c));
                    }
                    if let (Some(a), Some(b), Some(c)) = (elems[0].as_int(), elems[4].as_int(), elems[8].as_int()) {
                        return Some(Value::Int(a + b + c));
                    }
                }
            }
            None
        }
        "mat_trace4" => {
            if let Value::Array(elems, _) = args.first()? {
                if elems.len() >= 16 {
                    if let (Some(a), Some(b), Some(c), Some(d)) = (
                        elems[0].as_float(), elems[5].as_float(),
                        elems[10].as_float(), elems[15].as_float(),
                    ) {
                        return Some(Value::Float(a + b + c + d));
                    }
                    if let (Some(a), Some(b), Some(c), Some(d)) = (
                        elems[0].as_int(), elems[5].as_int(),
                        elems[10].as_int(), elems[15].as_int(),
                    ) {
                        return Some(Value::Int(a + b + c + d));
                    }
                }
            }
            None
        }
        "mat_inv4" => {
            if let Value::Array(elems, _) = args.first()? {
                if elems.len() >= 16 {
                    let m: Vec<f64> = elems.iter().filter_map(|v| v.as_float()).collect();
                    if m.len() >= 16 {
                        let mut arr = [0.0; 16];
                        arr.copy_from_slice(&m[..16]);
                        let inv = inv4_helper(&arr);
                        let res: Vec<Value> = inv.iter().map(|&x| Value::Float(x)).collect();
                        return Some(Value::Array(res, Type::F64));
                    }
                }
            }
            None
        }
        "mat_solve4" => {
            if let (Value::Array(mat_elems, _), Value::Array(b_elems, _)) = (args.first()?, args.get(1)?) {
                if mat_elems.len() >= 16 && b_elems.len() >= 4 {
                    let m: Vec<f64> = mat_elems.iter().filter_map(|v| v.as_float()).collect();
                    let b: Vec<f64> = b_elems.iter().filter_map(|v| v.as_float()).collect();
                    if m.len() >= 16 && b.len() >= 4 {
                        let mut arr = [0.0; 16];
                        arr.copy_from_slice(&m[..16]);
                        let inv = inv4_helper(&arr);
                        let mut res = Vec::with_capacity(4);
                        for r in 0..4 {
                            let entry = inv[r * 4] * b[0]
                                + inv[r * 4 + 1] * b[1]
                                + inv[r * 4 + 2] * b[2]
                                + inv[r * 4 + 3] * b[3];
                            res.push(Value::Float(entry));
                        }
                        return Some(Value::Array(res, Type::F64));
                    }
                }
            }
            None
        }
        "c_make" => {
            let re = args.first()?.as_float()?;
            let im = args.get(1)?.as_float()?;
            Some(Value::Array(vec![Value::Float(re), Value::Float(im)], Type::F64))
        }
        "c_re" => {
            if let Value::Array(elems, _) = args.first()? {
                return Some(elems.first()?.clone());
            }
            None
        }
        "c_im" => {
            if let Value::Array(elems, _) = args.first()? {
                return Some(elems.get(1)?.clone());
            }
            None
        }
        "c_add" => {
            if let (Value::Array(a, _), Value::Array(b, _)) = (args.first()?, args.get(1)?) {
                let a0 = a.first()?.as_float()?;
                let a1 = a.get(1)?.as_float()?;
                let b0 = b.first()?.as_float()?;
                let b1 = b.get(1)?.as_float()?;
                return Some(Value::Array(vec![Value::Float(a0 + b0), Value::Float(a1 + b1)], Type::F64));
            }
            None
        }
        "c_sub" => {
            if let (Value::Array(a, _), Value::Array(b, _)) = (args.first()?, args.get(1)?) {
                let a0 = a.first()?.as_float()?;
                let a1 = a.get(1)?.as_float()?;
                let b0 = b.first()?.as_float()?;
                let b1 = b.get(1)?.as_float()?;
                return Some(Value::Array(vec![Value::Float(a0 - b0), Value::Float(a1 - b1)], Type::F64));
            }
            None
        }
        "c_mul" => {
            if let (Value::Array(a, _), Value::Array(b, _)) = (args.first()?, args.get(1)?) {
                let a_re = a.first()?.as_float()?;
                let a_im = a.get(1)?.as_float()?;
                let b_re = b.first()?.as_float()?;
                let b_im = b.get(1)?.as_float()?;
                let re = a_re * b_re - a_im * b_im;
                let im = a_re * b_im + a_im * b_re;
                return Some(Value::Array(vec![Value::Float(re), Value::Float(im)], Type::F64));
            }
            None
        }
        "c_div" => {
            if let (Value::Array(a, _), Value::Array(b, _)) = (args.first()?, args.get(1)?) {
                let a_re = a.first()?.as_float()?;
                let a_im = a.get(1)?.as_float()?;
                let b_re = b.first()?.as_float()?;
                let b_im = b.get(1)?.as_float()?;
                let denom = b_re * b_re + b_im * b_im;
                let re = (a_re * b_re + a_im * b_im) / denom;
                let im = (a_im * b_re - a_re * b_im) / denom;
                return Some(Value::Array(vec![Value::Float(re), Value::Float(im)], Type::F64));
            }
            None
        }
        "c_conj" => {
            if let Value::Array(a, _) = args.first()? {
                let re = a.first()?.as_float()?;
                let im = a.get(1)?.as_float()?;
                return Some(Value::Array(vec![Value::Float(re), Value::Float(-im)], Type::F64));
            }
            None
        }
        "c_abs" => {
            if let Value::Array(a, _) = args.first()? {
                let re = a.first()?.as_float()?;
                let im = a.get(1)?.as_float()?;
                return Some(Value::Float(re.hypot(im)));
            }
            None
        }
        "c_arg" => {
            if let Value::Array(a, _) = args.first()? {
                let re = a.first()?.as_float()?;
                let im = a.get(1)?.as_float()?;
                return Some(Value::Float(im.atan2(re)));
            }
            None
        }
        "c_exp" => {
            if let Value::Array(a, _) = args.first()? {
                let x = a.first()?.as_float()?;
                let y = a.get(1)?.as_float()?;
                let r = x.exp();
                let re = r * y.cos();
                let im = r * y.sin();
                return Some(Value::Array(vec![Value::Float(re), Value::Float(im)], Type::F64));
            }
            None
        }
        "fft8" => {
            if let (Value::Array(re, _), Value::Array(im, _)) = (args.first()?, args.get(1)?) {
                let r_in: Vec<f64> = re.iter().filter_map(|v| v.as_float()).collect();
                let i_in: Vec<f64> = im.iter().filter_map(|v| v.as_float()).collect();
                if r_in.len() >= 8 && i_in.len() >= 8 {
                    let (r, i) = fft_helper(&r_in, &i_in, 8);
                    let mut all: Vec<Value> = r.iter().map(|&x| Value::Float(x)).collect();
                    all.extend(i.iter().map(|&x| Value::Float(x)));
                    return Some(Value::Array(all, Type::F64));
                }
            }
            None
        }
        "fft8_re" => {
            if let (Value::Array(re, _), Value::Array(im, _)) = (args.first()?, args.get(1)?) {
                let r_in: Vec<f64> = re.iter().filter_map(|v| v.as_float()).collect();
                let i_in: Vec<f64> = im.iter().filter_map(|v| v.as_float()).collect();
                if r_in.len() >= 8 && i_in.len() >= 8 {
                    let (r, _) = fft_helper(&r_in, &i_in, 8);
                    let res: Vec<Value> = r.iter().map(|&x| Value::Float(x)).collect();
                    return Some(Value::Array(res, Type::F64));
                }
            }
            None
        }
        "fft8_im" => {
            if let (Value::Array(re, _), Value::Array(im, _)) = (args.first()?, args.get(1)?) {
                let r_in: Vec<f64> = re.iter().filter_map(|v| v.as_float()).collect();
                let i_in: Vec<f64> = im.iter().filter_map(|v| v.as_float()).collect();
                if r_in.len() >= 8 && i_in.len() >= 8 {
                    let (_, i) = fft_helper(&r_in, &i_in, 8);
                    let res: Vec<Value> = i.iter().map(|&x| Value::Float(x)).collect();
                    return Some(Value::Array(res, Type::F64));
                }
            }
            None
        }
        "fft16" => {
            if let (Value::Array(re, _), Value::Array(im, _)) = (args.first()?, args.get(1)?) {
                let r_in: Vec<f64> = re.iter().filter_map(|v| v.as_float()).collect();
                let i_in: Vec<f64> = im.iter().filter_map(|v| v.as_float()).collect();
                if r_in.len() >= 16 && i_in.len() >= 16 {
                    let (r, i) = fft_helper(&r_in, &i_in, 16);
                    let mut all: Vec<Value> = r.iter().map(|&x| Value::Float(x)).collect();
                    all.extend(i.iter().map(|&x| Value::Float(x)));
                    return Some(Value::Array(all, Type::F64));
                }
            }
            None
        }
        "fft16_re" => {
            if let (Value::Array(re, _), Value::Array(im, _)) = (args.first()?, args.get(1)?) {
                let r_in: Vec<f64> = re.iter().filter_map(|v| v.as_float()).collect();
                let i_in: Vec<f64> = im.iter().filter_map(|v| v.as_float()).collect();
                if r_in.len() >= 16 && i_in.len() >= 16 {
                    let (r, _) = fft_helper(&r_in, &i_in, 16);
                    let res: Vec<Value> = r.iter().map(|&x| Value::Float(x)).collect();
                    return Some(Value::Array(res, Type::F64));
                }
            }
            None
        }
        "fft16_im" => {
            if let (Value::Array(re, _), Value::Array(im, _)) = (args.first()?, args.get(1)?) {
                let r_in: Vec<f64> = re.iter().filter_map(|v| v.as_float()).collect();
                let i_in: Vec<f64> = im.iter().filter_map(|v| v.as_float()).collect();
                if r_in.len() >= 16 && i_in.len() >= 16 {
                    let (_, i) = fft_helper(&r_in, &i_in, 16);
                    let res: Vec<Value> = i.iter().map(|&x| Value::Float(x)).collect();
                    return Some(Value::Array(res, Type::F64));
                }
            }
            None
        }
        _ => None,
    }
}
