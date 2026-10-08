//! Bounded while-loop unrolling and constant exponentiation expansion pass.
//!
//! Statically identifies while loops whose induction or reduction variable
//! has a compile-time known initial constant value and a deterministic per-iteration
//! update (e.g. `e = e / 2`, `e = e >> 1`, or `i = i + 1`).
//! For loops with trip count <= 16, expands the iterations into straight-line statements,
//! folds branch conditions (e.g. `if e % 2 == 1`), simplifies identities (`1 * x => x`),
//! eliminates dead stores in terminal iterations, and folds constant array indices.

use std::collections::{HashMap, HashSet};

use crate::ast::{BinaryOp, UnaryOp};
use crate::span::Span;
use crate::typecheck::typed_ast::{TypedBlock, TypedExpr, TypedLiteral, TypedProgram, TypedStmt};
use crate::typecheck::types::{wrap_int_by_type, Type};

pub fn optimize_program(program: &mut TypedProgram) {
    // Run up to 4 passes so that unrolling outer loops exposes newly constant inner loops
    for _ in 0..4 {
        let mut changed = false;
        for func in &mut program.functions {
            let mut env = HashMap::new();
            let mut known_mod = HashMap::new();
            let mut known_bounds = HashMap::new();
            let outer_live = HashSet::new();
            if optimize_block(
                &mut func.body,
                &mut env,
                &mut known_mod,
                &mut known_bounds,
                &outer_live,
            ) {
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

fn eval_const_expr(expr: &TypedExpr, known_consts: &HashMap<String, i64>) -> Option<i64> {
    match expr {
        TypedExpr::Literal {
            lit: TypedLiteral::Int(v, _),
            ..
        } => Some(*v),
        TypedExpr::Literal {
            lit: TypedLiteral::Bool(b),
            ..
        } => Some(if *b { 1 } else { 0 }),
        TypedExpr::Ident { name, .. } => known_consts.get(name).copied(),
        TypedExpr::Unary { op, expr, ty, .. } => {
            let val = eval_const_expr(expr, known_consts)?;
            match op {
                UnaryOp::Neg => Some(wrap_int_by_type(val.wrapping_neg(), Some(ty))),
                UnaryOp::Not => Some(if val == 0 { 1 } else { 0 }),
            }
        }
        TypedExpr::Binary {
            op,
            left,
            right,
            ty,
            ..
        } => {
            let operand_ty = left.ty();
            let is_unsigned = operand_ty.is_unsigned();
            let l = eval_const_expr(left, known_consts)?;
            let r = eval_const_expr(right, known_consts)?;
            let res = if is_unsigned {
                let lu = l as u64;
                let ru = r as u64;
                match op {
                    BinaryOp::Add => Some(lu.wrapping_add(ru) as i64),
                    BinaryOp::Sub => Some(lu.wrapping_sub(ru) as i64),
                    BinaryOp::Mul => Some(lu.wrapping_mul(ru) as i64),
                    BinaryOp::Div => lu.checked_div(ru).map(|v| v as i64),
                    BinaryOp::Mod => lu.checked_rem(ru).map(|v| v as i64),
                    BinaryOp::Pow => {
                        if ru <= 62 {
                            Some(lu.wrapping_pow(ru as u32) as i64)
                        } else {
                            None
                        }
                    }
                    BinaryOp::BitAnd => Some((lu & ru) as i64),
                    BinaryOp::BitOr => Some((lu | ru) as i64),
                    BinaryOp::BitXor => Some((lu ^ ru) as i64),
                    BinaryOp::Shl => {
                        if ru < 64 {
                            Some((lu << ru) as i64)
                        } else {
                            None
                        }
                    }
                    BinaryOp::Shr => {
                        if ru < 64 {
                            Some((lu >> ru) as i64)
                        } else {
                            None
                        }
                    }
                    BinaryOp::Eq => return Some(if lu == ru { 1 } else { 0 }),
                    BinaryOp::Ne => return Some(if lu != ru { 1 } else { 0 }),
                    BinaryOp::Lt => return Some(if lu < ru { 1 } else { 0 }),
                    BinaryOp::Le => return Some(if lu <= ru { 1 } else { 0 }),
                    BinaryOp::Gt => return Some(if lu > ru { 1 } else { 0 }),
                    BinaryOp::Ge => return Some(if lu >= ru { 1 } else { 0 }),
                }
            } else {
                match op {
                    BinaryOp::Add => Some(l.wrapping_add(r)),
                    BinaryOp::Sub => Some(l.wrapping_sub(r)),
                    BinaryOp::Mul => Some(l.wrapping_mul(r)),
                    BinaryOp::Div => {
                        if r != 0 {
                            Some(l.wrapping_div(r))
                        } else {
                            None
                        }
                    }
                    BinaryOp::Mod => {
                        if r != 0 {
                            Some(l.wrapping_rem(r))
                        } else {
                            None
                        }
                    }
                    BinaryOp::Pow => {
                        if (0..=62).contains(&r) {
                            Some(l.wrapping_pow(r as u32))
                        } else {
                            None
                        }
                    }
                    BinaryOp::BitAnd => Some(l & r),
                    BinaryOp::BitOr => Some(l | r),
                    BinaryOp::BitXor => Some(l ^ r),
                    BinaryOp::Shl => {
                        if (0..64).contains(&r) {
                            Some(l << r)
                        } else {
                            None
                        }
                    }
                    BinaryOp::Shr => {
                        if (0..64).contains(&r) {
                            Some(l >> r)
                        } else {
                            None
                        }
                    }
                    BinaryOp::Eq => return Some(if l == r { 1 } else { 0 }),
                    BinaryOp::Ne => return Some(if l != r { 1 } else { 0 }),
                    BinaryOp::Lt => return Some(if l < r { 1 } else { 0 }),
                    BinaryOp::Le => return Some(if l <= r { 1 } else { 0 }),
                    BinaryOp::Gt => return Some(if l > r { 1 } else { 0 }),
                    BinaryOp::Ge => return Some(if l >= r { 1 } else { 0 }),
                }
            };
            res.map(|v| wrap_int_by_type(v, Some(ty)))
        }
        _ => None,
    }
}

fn expr_eq(a: &TypedExpr, b: &TypedExpr) -> bool {
    match (a, b) {
        (TypedExpr::Ident { name: n1, .. }, TypedExpr::Ident { name: n2, .. }) => n1 == n2,
        (TypedExpr::Literal { lit: l1, .. }, TypedExpr::Literal { lit: l2, .. }) => {
            match (l1, l2) {
                (TypedLiteral::Int(v1, _), TypedLiteral::Int(v2, _)) => v1 == v2,
                (TypedLiteral::Bool(b1), TypedLiteral::Bool(b2)) => b1 == b2,
                _ => false,
            }
        }
        _ => false,
    }
}

fn format_expr(expr: &TypedExpr) -> String {
    match expr {
        TypedExpr::Ident { name, .. } => name.clone(),
        TypedExpr::Literal {
            lit: TypedLiteral::Int(v, _),
            ..
        } => v.to_string(),
        _ => String::new(),
    }
}

fn fold_expr(
    expr: &TypedExpr,
    known_consts: &HashMap<String, i64>,
    known_mod: &HashMap<String, String>,
    known_upper_bounds: &HashMap<String, i64>,
) -> TypedExpr {
    let is_div_or_mod = matches!(
        expr,
        TypedExpr::Binary {
            op: BinaryOp::Div | BinaryOp::Mod,
            ..
        }
    );
    if !is_div_or_mod {
        if let Some(c) = eval_const_expr(expr, known_consts) {
            let ty = expr.ty();
            let span = expr.span();
            if ty == Type::Bool {
                return TypedExpr::Literal {
                    lit: TypedLiteral::Bool(c != 0),
                    ty,
                    span,
                };
            } else if ty.is_integer() {
                return TypedExpr::Literal {
                    lit: TypedLiteral::Int(c, ty.clone()),
                    ty,
                    span,
                };
            }
        }
    }

    match expr {
        TypedExpr::Unary {
            op,
            expr: inner,
            ty,
            span,
        } => {
            let folded_inner = fold_expr(inner, known_consts, known_mod, known_upper_bounds);
            TypedExpr::Unary {
                op: *op,
                expr: Box::new(folded_inner),
                ty: ty.clone(),
                span: *span,
            }
        }
        TypedExpr::Binary {
            op,
            left,
            right,
            ty,
            span,
        } => {
            let folded_l = fold_expr(left, known_consts, known_mod, known_upper_bounds);
            let folded_r = fold_expr(right, known_consts, known_mod, known_upper_bounds);

            // Algebraic identity simplifications
            match op {
                BinaryOp::Mul => {
                    if let Some(1) = eval_const_expr(&folded_l, known_consts) {
                        return folded_r;
                    }
                    if let Some(1) = eval_const_expr(&folded_r, known_consts) {
                        return folded_l;
                    }
                    if let Some(0) = eval_const_expr(&folded_l, known_consts) {
                        return TypedExpr::Literal {
                            lit: TypedLiteral::Int(0, ty.clone()),
                            ty: ty.clone(),
                            span: *span,
                        };
                    }
                    if let Some(0) = eval_const_expr(&folded_r, known_consts) {
                        return TypedExpr::Literal {
                            lit: TypedLiteral::Int(0, ty.clone()),
                            ty: ty.clone(),
                            span: *span,
                        };
                    }
                }
                BinaryOp::Add => {
                    if let Some(0) = eval_const_expr(&folded_l, known_consts) {
                        return folded_r;
                    }
                    if let Some(0) = eval_const_expr(&folded_r, known_consts) {
                        return folded_l;
                    }
                }
                BinaryOp::Sub => {
                    if let Some(0) = eval_const_expr(&folded_r, known_consts) {
                        return folded_l;
                    }
                }
                BinaryOp::Mod => {
                    // (x % m) % m  =>  x % m
                    if let TypedExpr::Binary {
                        op: BinaryOp::Mod,
                        right: inner_r,
                        ..
                    } = &folded_l
                    {
                        if expr_eq(inner_r, &folded_r) {
                            return folded_l;
                        }
                    }
                    if let TypedExpr::Ident { name, .. } = &folded_l {
                        let r_key = format_expr(&folded_r);
                        if !r_key.is_empty() {
                            if let Some(m_key) = known_mod.get(name) {
                                if m_key == &r_key {
                                    return folded_l;
                                }
                            }
                        }
                        if let Some(&ub) = known_upper_bounds.get(name) {
                            if let Some(m_val) = eval_const_expr(&folded_r, known_consts) {
                                if ub >= 0 && ub < m_val {
                                    return folded_l;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }

            TypedExpr::Binary {
                op: *op,
                left: Box::new(folded_l),
                right: Box::new(folded_r),
                ty: ty.clone(),
                span: *span,
            }
        }
        TypedExpr::Call {
            callee,
            args,
            ty,
            span,
        } => {
            let folded_args = args
                .iter()
                .map(|a| fold_expr(a, known_consts, known_mod, known_upper_bounds))
                .collect();
            TypedExpr::Call {
                callee: callee.clone(),
                args: folded_args,
                ty: ty.clone(),
                span: *span,
            }
        }
        TypedExpr::Index {
            target,
            index,
            is_safe,
            ty,
            span,
        } => {
            let folded_target = fold_expr(target, known_consts, known_mod, known_upper_bounds);
            let folded_idx = fold_expr(index, known_consts, known_mod, known_upper_bounds);
            TypedExpr::Index {
                target: Box::new(folded_target),
                index: Box::new(folded_idx),
                is_safe: *is_safe,
                ty: ty.clone(),
                span: *span,
            }
        }
        _ => expr.clone(),
    }
}

fn collect_read_vars_expr(expr: &TypedExpr, reads: &mut HashSet<String>) {
    match expr {
        TypedExpr::Ident { name, .. } => {
            reads.insert(name.clone());
        }
        TypedExpr::Unary { expr, .. } => {
            collect_read_vars_expr(expr, reads);
        }
        TypedExpr::Binary { left, right, .. } => {
            collect_read_vars_expr(left, reads);
            collect_read_vars_expr(right, reads);
        }
        TypedExpr::Call { args, .. } => {
            for a in args {
                collect_read_vars_expr(a, reads);
            }
        }
        TypedExpr::CallIndirect { callee, args, .. } => {
            collect_read_vars_expr(callee, reads);
            for a in args {
                collect_read_vars_expr(a, reads);
            }
        }
        TypedExpr::ArrayLiteral { elements, .. } => {
            for e in elements {
                collect_read_vars_expr(e, reads);
            }
        }
        TypedExpr::Index { target, index, .. } => {
            collect_read_vars_expr(target, reads);
            collect_read_vars_expr(index, reads);
        }
        TypedExpr::Box { inner, .. } => {
            collect_read_vars_expr(inner, reads);
        }
        TypedExpr::Deref { inner, .. } => {
            collect_read_vars_expr(inner, reads);
        }
        TypedExpr::StructLiteral { fields, .. } => {
            for (_, fexpr) in fields {
                collect_read_vars_expr(fexpr, reads);
            }
        }
        TypedExpr::EnumConstructor { args, .. } => {
            for a in args {
                collect_read_vars_expr(a, reads);
            }
        }
        TypedExpr::FieldAccess { target, .. } => {
            collect_read_vars_expr(target, reads);
        }
        TypedExpr::Match {
            scrutinee, arms, ..
        } => {
            collect_read_vars_expr(scrutinee, reads);
            for arm in arms {
                collect_read_vars_expr(&arm.body, reads);
            }
        }
        TypedExpr::Lambda { captured, .. } => {
            for (name, _) in captured {
                reads.insert(name.clone());
            }
        }
        _ => {}
    }
}

fn collect_read_vars(stmts: &[TypedStmt], reads: &mut HashSet<String>) {
    for s in stmts {
        match s {
            TypedStmt::Let { value, .. }
            | TypedStmt::Assign { value, .. }
            | TypedStmt::Expr(value) => {
                collect_read_vars_expr(value, reads);
            }
            TypedStmt::IndexAssign {
                target,
                index,
                value,
                ..
            } => {
                reads.insert(target.clone());
                collect_read_vars_expr(index, reads);
                collect_read_vars_expr(value, reads);
            }
            TypedStmt::FieldAssign { target, value, .. } => {
                reads.insert(target.clone());
                collect_read_vars_expr(value, reads);
            }
            TypedStmt::Return(Some(expr), _) => {
                collect_read_vars_expr(expr, reads);
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                collect_read_vars_expr(condition, reads);
                collect_read_vars(&then_branch.stmts, reads);
                if let Some(eb) = else_branch {
                    collect_read_vars(&eb.stmts, reads);
                }
            }
            TypedStmt::While {
                condition, body, ..
            } => {
                collect_read_vars_expr(condition, reads);
                collect_read_vars(&body.stmts, reads);
            }
            TypedStmt::For { lo, hi, body, .. } => {
                collect_read_vars_expr(lo, reads);
                collect_read_vars_expr(hi, reads);
                collect_read_vars(&body.stmts, reads);
            }
            _ => {}
        }
    }
}

fn has_break_or_return(block: &TypedBlock) -> bool {
    for s in &block.stmts {
        match s {
            TypedStmt::Break(_) | TypedStmt::Return(..) => return true,
            TypedStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if has_break_or_return(then_branch) {
                    return true;
                }
                if let Some(eb) = else_branch {
                    if has_break_or_return(eb) {
                        return true;
                    }
                }
            }
            TypedStmt::While { body, .. } if has_break_or_return(body) => {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn has_nested_while(block: &TypedBlock) -> bool {
    for s in &block.stmts {
        match s {
            TypedStmt::While { .. } => return true,
            TypedStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if has_nested_while(then_branch) {
                    return true;
                }
                if let Some(eb) = else_branch {
                    if has_nested_while(eb) {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

fn count_var_assignments(block: &TypedBlock, var_name: &str) -> usize {
    let mut count = 0;
    for s in &block.stmts {
        match s {
            TypedStmt::Assign { name, .. } if name == var_name => count += 1,
            TypedStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                count += count_var_assignments(then_branch, var_name);
                if let Some(eb) = else_branch {
                    count += count_var_assignments(eb, var_name);
                }
            }
            TypedStmt::While { body, .. } => {
                count += count_var_assignments(body, var_name);
            }
            _ => {}
        }
    }
    count
}

#[derive(Clone, Copy, Debug)]
enum StepOp {
    Div(i64),
    Shr(i64),
    Sub(i64),
    Add(i64),
}

fn find_step_op(block: &TypedBlock, var_name: &str) -> Option<StepOp> {
    for s in &block.stmts {
        if let TypedStmt::Assign { name, value, .. } = s {
            if name == var_name {
                if let TypedExpr::Binary {
                    op, left, right, ..
                } = value
                {
                    let is_var_l = match &**left {
                        TypedExpr::Ident { name: n, .. } => n == var_name,
                        _ => false,
                    };
                    let is_var_r = match &**right {
                        TypedExpr::Ident { name: n, .. } => n == var_name,
                        _ => false,
                    };
                    let const_r = match &**right {
                        TypedExpr::Literal {
                            lit: TypedLiteral::Int(c, _),
                            ..
                        } => Some(*c),
                        _ => None,
                    };
                    let const_l = match &**left {
                        TypedExpr::Literal {
                            lit: TypedLiteral::Int(c, _),
                            ..
                        } => Some(*c),
                        _ => None,
                    };

                    match op {
                        BinaryOp::Div if is_var_l => {
                            if let Some(c) = const_r {
                                if c >= 2 {
                                    return Some(StepOp::Div(c));
                                }
                            }
                        }
                        BinaryOp::Shr if is_var_l => {
                            if let Some(c) = const_r {
                                if c >= 1 {
                                    return Some(StepOp::Shr(c));
                                }
                            }
                        }
                        BinaryOp::Sub if is_var_l => {
                            if let Some(c) = const_r {
                                if c >= 1 {
                                    return Some(StepOp::Sub(c));
                                }
                            }
                        }
                        BinaryOp::Add => {
                            if is_var_l {
                                if let Some(c) = const_r {
                                    if c >= 1 {
                                        return Some(StepOp::Add(c));
                                    }
                                }
                            } else if is_var_r {
                                if let Some(c) = const_l {
                                    if c >= 1 {
                                        return Some(StepOp::Add(c));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    None
}

fn apply_step(val: i64, op: StepOp) -> i64 {
    match op {
        StepOp::Div(c) => val / c,
        StepOp::Shr(c) => val >> c,
        StepOp::Sub(c) => val - c,
        StepOp::Add(c) => val + c,
    }
}

fn extract_loop_var(condition: &TypedExpr) -> Option<String> {
    if let TypedExpr::Binary { left, right, .. } = condition {
        if let TypedExpr::Ident { name, .. } = &**left {
            return Some(name.clone());
        }
        if let TypedExpr::Ident { name, .. } = &**right {
            return Some(name.clone());
        }
    }
    None
}

struct SpecializeCtx<'a> {
    var_name: &'a str,
    curr_val: i64,
    iter_consts: &'a mut HashMap<String, i64>,
    iter_mod: &'a mut HashMap<String, String>,
    iter_bounds: &'a mut HashMap<String, i64>,
    is_last_iter: bool,
    live_after: &'a HashSet<String>,
    accum_var: Option<&'a str>,
}

fn specialize_stmts(stmts: &[TypedStmt], ctx: &mut SpecializeCtx<'_>) -> Vec<TypedStmt> {
    ctx.iter_consts
        .insert(ctx.var_name.to_string(), ctx.curr_val);

    let mut result = Vec::new();
    for (s_idx, stmt) in stmts.iter().enumerate() {
        match stmt {
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                if let Some(c) = eval_const_expr(condition, ctx.iter_consts) {
                    if c != 0 {
                        let specialized_then = specialize_stmts(&then_branch.stmts, ctx);
                        result.extend(specialized_then);
                    } else if let Some(eb) = else_branch {
                        let specialized_else = specialize_stmts(&eb.stmts, ctx);
                        result.extend(specialized_else);
                    }
                } else {
                    let mut mutated_in_branches = HashSet::new();
                    collect_mutated_vars(then_branch, &mut mutated_in_branches);
                    if let Some(eb) = else_branch {
                        collect_mutated_vars(eb, &mut mutated_in_branches);
                    }
                    for m in &mutated_in_branches {
                        ctx.iter_consts.remove(m);
                        ctx.iter_mod.remove(m);
                        ctx.iter_bounds.remove(m);
                    }

                    let folded_cond =
                        fold_expr(condition, ctx.iter_consts, ctx.iter_mod, ctx.iter_bounds);
                    let mut then_consts = ctx.iter_consts.clone();
                    let mut then_mod = ctx.iter_mod.clone();
                    let mut then_bounds = ctx.iter_bounds.clone();
                    let mut then_ctx = SpecializeCtx {
                        var_name: ctx.var_name,
                        curr_val: ctx.curr_val,
                        iter_consts: &mut then_consts,
                        iter_mod: &mut then_mod,
                        iter_bounds: &mut then_bounds,
                        is_last_iter: ctx.is_last_iter,
                        live_after: ctx.live_after,
                        accum_var: ctx.accum_var,
                    };
                    let folded_then = specialize_stmts(&then_branch.stmts, &mut then_ctx);

                    let folded_else = else_branch.as_ref().map(|eb| {
                        let mut else_consts = ctx.iter_consts.clone();
                        let mut else_mod = ctx.iter_mod.clone();
                        let mut else_bounds = ctx.iter_bounds.clone();
                        let mut else_ctx = SpecializeCtx {
                            var_name: ctx.var_name,
                            curr_val: ctx.curr_val,
                            iter_consts: &mut else_consts,
                            iter_mod: &mut else_mod,
                            iter_bounds: &mut else_bounds,
                            is_last_iter: ctx.is_last_iter,
                            live_after: ctx.live_after,
                            accum_var: ctx.accum_var,
                        };
                        TypedBlock {
                            stmts: specialize_stmts(&eb.stmts, &mut else_ctx),
                            span: eb.span,
                        }
                    });

                    for m in &mutated_in_branches {
                        ctx.iter_consts.remove(m);
                        ctx.iter_mod.remove(m);
                        ctx.iter_bounds.remove(m);
                    }

                    result.push(TypedStmt::If {
                        condition: folded_cond,
                        then_branch: TypedBlock {
                            stmts: folded_then,
                            span: then_branch.span,
                        },
                        else_branch: folded_else,
                        span: stmt.span(),
                    });
                }
            }
            TypedStmt::Assign { name, value, span } => {
                if name == ctx.var_name {
                    continue;
                }
                // Dead store elimination in terminal iteration: only eliminate if not read
                // in remaining statements of this iteration AND not live after the loop
                if ctx.is_last_iter {
                    let mut live_remaining = ctx.live_after.clone();
                    collect_read_vars(&stmts[s_idx + 1..], &mut live_remaining);
                    if !live_remaining.contains(name) {
                        continue;
                    }
                }
                let val_to_fold = if !ctx.is_last_iter && ctx.accum_var == Some(name.as_str()) {
                    if let TypedExpr::Binary {
                        op: BinaryOp::Mod,
                        left,
                        ..
                    } = value
                    {
                        left.as_ref()
                    } else {
                        value
                    }
                } else {
                    value
                };
                let folded_val =
                    fold_expr(val_to_fold, ctx.iter_consts, ctx.iter_mod, ctx.iter_bounds);
                if let Some(c) = eval_const_expr(&folded_val, ctx.iter_consts) {
                    ctx.iter_consts.insert(name.clone(), c);
                } else {
                    ctx.iter_consts.remove(name);
                }
                match &folded_val {
                    TypedExpr::Binary {
                        op: BinaryOp::Mod,
                        right,
                        ..
                    } => {
                        let k = format_expr(right);
                        if !k.is_empty() {
                            ctx.iter_mod.insert(name.clone(), k);
                        } else {
                            ctx.iter_mod.remove(name);
                        }
                    }
                    TypedExpr::Ident { name: src, .. } => {
                        if let Some(k) = ctx.iter_mod.get(src).cloned() {
                            ctx.iter_mod.insert(name.clone(), k);
                        } else {
                            ctx.iter_mod.remove(name);
                        }
                        if let Some(&b) = ctx.iter_bounds.get(src) {
                            ctx.iter_bounds.insert(name.clone(), b);
                        } else {
                            ctx.iter_bounds.remove(name);
                        }
                    }
                    _ => {
                        ctx.iter_mod.remove(name);
                        ctx.iter_bounds.remove(name);
                    }
                }
                result.push(TypedStmt::Assign {
                    name: name.clone(),
                    value: folded_val,
                    span: *span,
                });
            }
            TypedStmt::Let {
                name,
                is_mutable,
                ty,
                value,
                span,
            } => {
                let folded_val = fold_expr(value, ctx.iter_consts, ctx.iter_mod, ctx.iter_bounds);
                if let Some(c) = eval_const_expr(&folded_val, ctx.iter_consts) {
                    ctx.iter_consts.insert(name.clone(), c);
                } else {
                    ctx.iter_consts.remove(name);
                }
                match &folded_val {
                    TypedExpr::Binary {
                        op: BinaryOp::Mod,
                        right,
                        ..
                    } => {
                        let k = format_expr(right);
                        if !k.is_empty() {
                            ctx.iter_mod.insert(name.clone(), k);
                        } else {
                            ctx.iter_mod.remove(name);
                        }
                    }
                    TypedExpr::Ident { name: src, .. } => {
                        if let Some(k) = ctx.iter_mod.get(src).cloned() {
                            ctx.iter_mod.insert(name.clone(), k);
                        } else {
                            ctx.iter_mod.remove(name);
                        }
                        if let Some(&b) = ctx.iter_bounds.get(src) {
                            ctx.iter_bounds.insert(name.clone(), b);
                        } else {
                            ctx.iter_bounds.remove(name);
                        }
                    }
                    _ => {
                        ctx.iter_mod.remove(name);
                        ctx.iter_bounds.remove(name);
                    }
                }
                result.push(TypedStmt::Let {
                    name: name.clone(),
                    is_mutable: *is_mutable,
                    ty: ty.clone(),
                    value: folded_val,
                    span: *span,
                });
            }
            TypedStmt::IndexAssign {
                target,
                index,
                value,
                is_safe,
                span,
            } => {
                let folded_idx = fold_expr(index, ctx.iter_consts, ctx.iter_mod, ctx.iter_bounds);
                let folded_val = fold_expr(value, ctx.iter_consts, ctx.iter_mod, ctx.iter_bounds);
                result.push(TypedStmt::IndexAssign {
                    target: target.clone(),
                    index: folded_idx,
                    value: folded_val,
                    is_safe: *is_safe,
                    span: *span,
                });
            }
            TypedStmt::Expr(expr) => {
                let folded_e = fold_expr(expr, ctx.iter_consts, ctx.iter_mod, ctx.iter_bounds);
                result.push(TypedStmt::Expr(folded_e));
            }
            _ => {
                result.push(stmt.clone());
            }
        }
    }
    result
}

fn collect_mutated_vars(block: &TypedBlock, mutated: &mut HashSet<String>) {
    for s in &block.stmts {
        match s {
            TypedStmt::Assign { name, .. } => {
                mutated.insert(name.clone());
            }
            TypedStmt::IndexAssign { target, .. } => {
                mutated.insert(target.clone());
            }
            TypedStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_mutated_vars(then_branch, mutated);
                if let Some(eb) = else_branch {
                    collect_mutated_vars(eb, mutated);
                }
            }
            TypedStmt::While { body, .. } => {
                collect_mutated_vars(body, mutated);
            }
            _ => {}
        }
    }
}

fn find_accum_mod_var(
    body: &TypedBlock,
    loop_var: &str,
    known_consts: &HashMap<String, i64>,
    total_iters: usize,
) -> Option<String> {
    if !(2..=8).contains(&total_iters) {
        return None;
    }

    let mut candidate = None;
    for stmt in &body.stmts {
        if let TypedStmt::Assign { name, value, .. } = stmt {
            if name == loop_var {
                continue;
            }
            if let TypedExpr::Binary {
                op: BinaryOp::Mod,
                left,
                right,
                ..
            } = value
            {
                let m_val = eval_const_expr(right, known_consts)?;
                if m_val <= 0 || m_val > 1_000_000_007 {
                    return None;
                }
                if let TypedExpr::Binary {
                    op: BinaryOp::Add,
                    left: add_l,
                    right: add_r,
                    ..
                } = &**left
                {
                    let is_acc_l =
                        matches!(&**add_l, TypedExpr::Ident { name: n, .. } if n == name);
                    let is_acc_r =
                        matches!(&**add_r, TypedExpr::Ident { name: n, .. } if n == name);
                    if is_acc_l || is_acc_r {
                        let term = if is_acc_l { add_r } else { add_l };
                        let mut term_reads = HashSet::new();
                        collect_read_vars_expr(term, &mut term_reads);
                        if term_reads.contains(name) {
                            return None;
                        }
                        if candidate.is_some() {
                            return None;
                        }
                        candidate = Some(name.clone());
                    }
                }
            }
        }
    }

    let acc_name = candidate?;

    if count_var_assignments(body, &acc_name) != 1 {
        return None;
    }

    if known_consts.get(&acc_name) != Some(&0) {
        return None;
    }

    let mut other_reads = HashSet::new();
    for stmt in &body.stmts {
        match stmt {
            TypedStmt::Assign { name, .. } if name == &acc_name => {}
            _ => {
                collect_read_vars(std::slice::from_ref(stmt), &mut other_reads);
            }
        }
    }
    if other_reads.contains(&acc_name) {
        return None;
    }

    Some(acc_name)
}

fn try_unroll_while(
    condition: &TypedExpr,
    body: &TypedBlock,
    known_consts: &HashMap<String, i64>,
    known_mod: &HashMap<String, String>,
    known_bounds: &HashMap<String, i64>,
    live_after: &HashSet<String>,
    span: Span,
) -> Option<Vec<TypedStmt>> {
    if has_break_or_return(body) || has_nested_while(body) {
        return None;
    }

    let var_name = extract_loop_var(condition)?;
    let init_val = known_consts.get(&var_name).copied()?;

    if count_var_assignments(body, &var_name) != 1 {
        return None;
    }

    let step_op = find_step_op(body, &var_name)?;

    let mut mutated_vars = HashSet::new();
    collect_mutated_vars(body, &mut mutated_vars);

    // Precompute iteration count
    let mut curr_val = init_val;
    let mut total_iters = 0;
    const MAX_UNROLL: usize = 16;

    loop {
        let mut check_env = known_consts.clone();
        check_env.insert(var_name.clone(), curr_val);

        let cond_val = eval_const_expr(condition, &check_env)?;
        if cond_val == 0 {
            break;
        }

        if total_iters >= MAX_UNROLL {
            return None;
        }
        total_iters += 1;

        let next_val = apply_step(curr_val, step_op);
        if next_val == curr_val {
            return None;
        }
        curr_val = next_val;
    }

    // Bound unrolling budget to prevent basic block bloat and extreme register pressure
    if total_iters * body.stmts.len() > 60 {
        return None;
    }

    let accum_var = find_accum_mod_var(body, &var_name, known_consts, total_iters);

    // Now unroll
    curr_val = init_val;
    let mut current_env = known_consts.clone();
    let mut current_mod = known_mod.clone();
    let mut current_bounds = known_bounds.clone();
    let mut unrolled_stmts = Vec::new();

    for iter in 0..total_iters {
        let is_last = iter + 1 == total_iters;
        let mut ctx = SpecializeCtx {
            var_name: &var_name,
            curr_val,
            iter_consts: &mut current_env,
            iter_mod: &mut current_mod,
            iter_bounds: &mut current_bounds,
            is_last_iter: is_last,
            live_after,
            accum_var: accum_var.as_deref(),
        };
        let iter_stmts = specialize_stmts(&body.stmts, &mut ctx);
        unrolled_stmts.extend(iter_stmts);
        curr_val = apply_step(curr_val, step_op);
    }

    // If loop variable is read after the loop, assign its final value
    if live_after.contains(&var_name) {
        unrolled_stmts.push(TypedStmt::Assign {
            name: var_name,
            value: TypedExpr::Literal {
                lit: TypedLiteral::Int(curr_val, Type::I64),
                ty: Type::I64,
                span,
            },
            span,
        });
    }

    Some(unrolled_stmts)
}

fn optimize_block(
    block: &mut TypedBlock,
    known_consts: &mut HashMap<String, i64>,
    known_mod: &mut HashMap<String, String>,
    known_bounds: &mut HashMap<String, i64>,
    outer_live: &HashSet<String>,
) -> bool {
    let mut changed = false;
    let mut new_stmts = Vec::new();

    for (idx, stmt) in block.stmts.iter().enumerate() {
        let mut live_after = outer_live.clone();
        collect_read_vars(&block.stmts[idx + 1..], &mut live_after);

        match stmt {
            TypedStmt::Let {
                name,
                is_mutable,
                ty,
                value,
                span,
            } => {
                let folded_val = fold_expr(value, known_consts, known_mod, known_bounds);
                if let Some(c) = eval_const_expr(&folded_val, known_consts) {
                    known_consts.insert(name.clone(), c);
                } else {
                    known_consts.remove(name);
                }
                match &folded_val {
                    TypedExpr::Binary {
                        op: BinaryOp::Mod,
                        right,
                        ..
                    } => {
                        let k = format_expr(right);
                        if !k.is_empty() {
                            known_mod.insert(name.clone(), k);
                        } else {
                            known_mod.remove(name);
                        }
                    }
                    TypedExpr::Ident { name: src, .. } => {
                        if let Some(k) = known_mod.get(src).cloned() {
                            known_mod.insert(name.clone(), k);
                        } else {
                            known_mod.remove(name);
                        }
                        if let Some(&b) = known_bounds.get(src) {
                            known_bounds.insert(name.clone(), b);
                        } else {
                            known_bounds.remove(name);
                        }
                    }
                    _ => {
                        known_mod.remove(name);
                        known_bounds.remove(name);
                    }
                }
                new_stmts.push(TypedStmt::Let {
                    name: name.clone(),
                    is_mutable: *is_mutable,
                    ty: ty.clone(),
                    value: folded_val,
                    span: *span,
                });
            }
            TypedStmt::Assign { name, value, span } => {
                let folded_val = fold_expr(value, known_consts, known_mod, known_bounds);
                if let Some(c) = eval_const_expr(&folded_val, known_consts) {
                    known_consts.insert(name.clone(), c);
                } else {
                    known_consts.remove(name);
                }
                match &folded_val {
                    TypedExpr::Binary {
                        op: BinaryOp::Mod,
                        right,
                        ..
                    } => {
                        let k = format_expr(right);
                        if !k.is_empty() {
                            known_mod.insert(name.clone(), k);
                        } else {
                            known_mod.remove(name);
                        }
                    }
                    TypedExpr::Ident { name: src, .. } => {
                        if let Some(k) = known_mod.get(src).cloned() {
                            known_mod.insert(name.clone(), k);
                        } else {
                            known_mod.remove(name);
                        }
                        if let Some(&b) = known_bounds.get(src) {
                            known_bounds.insert(name.clone(), b);
                        } else {
                            known_bounds.remove(name);
                        }
                    }
                    _ => {
                        known_mod.remove(name);
                        known_bounds.remove(name);
                    }
                }
                new_stmts.push(TypedStmt::Assign {
                    name: name.clone(),
                    value: folded_val,
                    span: *span,
                });
            }
            TypedStmt::Expr(expr) => {
                let folded_e = fold_expr(expr, known_consts, known_mod, known_bounds);
                new_stmts.push(TypedStmt::Expr(folded_e));
            }
            TypedStmt::Return(expr, span) => {
                let folded_e = expr
                    .as_ref()
                    .map(|e| fold_expr(e, known_consts, known_mod, known_bounds));
                new_stmts.push(TypedStmt::Return(folded_e, *span));
            }
            TypedStmt::While {
                condition,
                body,
                span,
            } => {
                if let Some(unrolled) = try_unroll_while(
                    condition,
                    body,
                    known_consts,
                    known_mod,
                    known_bounds,
                    &live_after,
                    *span,
                ) {
                    let mut mutated_vars = HashSet::new();
                    collect_mutated_vars(body, &mut mutated_vars);
                    for m in &mutated_vars {
                        known_consts.remove(m);
                        known_mod.remove(m);
                        known_bounds.remove(m);
                    }
                    for s in &unrolled {
                        if let TypedStmt::Assign { name, value, .. } = s {
                            if let Some(c) = eval_const_expr(value, known_consts) {
                                known_consts.insert(name.clone(), c);
                            } else {
                                known_consts.remove(name);
                            }
                        }
                        new_stmts.push(s.clone());
                    }
                    changed = true;
                } else {
                    let mut body_clone = body.clone();
                    let mut mutated_in_body = HashSet::new();
                    collect_mutated_vars(&body_clone, &mut mutated_in_body);
                    let mut inner_env: HashMap<String, i64> = known_consts
                        .iter()
                        .filter(|(k, _)| !mutated_in_body.contains(*k))
                        .map(|(k, v)| (k.clone(), *v))
                        .collect();
                    let mut inner_mod: HashMap<String, String> = known_mod
                        .iter()
                        .filter(|(k, _)| !mutated_in_body.contains(*k))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    let mut inner_bounds = known_bounds.clone();
                    match condition {
                        TypedExpr::Binary {
                            op: BinaryOp::Le,
                            left,
                            right,
                            ..
                        } => {
                            if let TypedExpr::Ident { name, .. } = &**left {
                                if let Some(limit) = eval_const_expr(right, known_consts) {
                                    inner_bounds.insert(name.clone(), limit);
                                }
                            }
                        }
                        TypedExpr::Binary {
                            op: BinaryOp::Lt,
                            left,
                            right,
                            ..
                        } => {
                            if let TypedExpr::Ident { name, .. } = &**left {
                                if let Some(limit) = eval_const_expr(right, known_consts) {
                                    inner_bounds.insert(name.clone(), limit - 1);
                                }
                            }
                        }
                        _ => {}
                    }
                    let mut body_outer_live = live_after.clone();
                    collect_read_vars_expr(condition, &mut body_outer_live);
                    collect_read_vars(&body_clone.stmts, &mut body_outer_live);
                    if optimize_block(
                        &mut body_clone,
                        &mut inner_env,
                        &mut inner_mod,
                        &mut inner_bounds,
                        &body_outer_live,
                    ) {
                        changed = true;
                    }
                    for m in mutated_in_body {
                        known_consts.remove(&m);
                        known_mod.remove(&m);
                        known_bounds.remove(&m);
                    }
                    new_stmts.push(TypedStmt::While {
                        condition: condition.clone(),
                        body: body_clone,
                        span: *span,
                    });
                }
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let mut mutated_in_branches = HashSet::new();
                collect_mutated_vars(then_branch, &mut mutated_in_branches);
                if let Some(eb) = else_branch {
                    collect_mutated_vars(eb, &mut mutated_in_branches);
                }

                let mut then_clone = then_branch.clone();
                let mut then_env: HashMap<String, i64> = known_consts
                    .iter()
                    .filter(|(k, _)| !mutated_in_branches.contains(*k))
                    .map(|(k, v)| (k.clone(), *v))
                    .collect();
                let mut then_mod: HashMap<String, String> = known_mod
                    .iter()
                    .filter(|(k, _)| !mutated_in_branches.contains(*k))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                let mut then_bounds = known_bounds.clone();
                if optimize_block(
                    &mut then_clone,
                    &mut then_env,
                    &mut then_mod,
                    &mut then_bounds,
                    &live_after,
                ) {
                    changed = true;
                }

                let else_clone = else_branch.as_ref().map(|eb| {
                    let mut ec = eb.clone();
                    let mut else_env: HashMap<String, i64> = known_consts
                        .iter()
                        .filter(|(k, _)| !mutated_in_branches.contains(*k))
                        .map(|(k, v)| (k.clone(), *v))
                        .collect();
                    let mut else_mod: HashMap<String, String> = known_mod
                        .iter()
                        .filter(|(k, _)| !mutated_in_branches.contains(*k))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    let mut else_bounds = known_bounds.clone();
                    if optimize_block(
                        &mut ec,
                        &mut else_env,
                        &mut else_mod,
                        &mut else_bounds,
                        &live_after,
                    ) {
                        changed = true;
                    }
                    ec
                });

                for m in mutated_in_branches {
                    known_consts.remove(&m);
                    known_mod.remove(&m);
                    known_bounds.remove(&m);
                }
                new_stmts.push(TypedStmt::If {
                    condition: condition.clone(),
                    then_branch: then_clone,
                    else_branch: else_clone,
                    span: *span,
                });
            }
            _ => {
                new_stmts.push(stmt.clone());
            }
        }
    }

    block.stmts = new_stmts;
    changed
}
