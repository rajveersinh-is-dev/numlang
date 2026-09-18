//! Loop optimization pass for NumLang.
//!
//! 1. Active Flag Elimination:
//!    Canonicalizes loops governed by boolean flags (such as while active { if k < max_iter ... })
//!    into direct bounded while loops (while k < max_iter { ... }) with  reak statements.
//!
//! 2. Collatz Parity Jump Threading:
//!    Identifies odd/even parity branches in 3n+1 loops and jump-threads the odd step
//!    curr = (curr * 3 + 1) / 2; total_steps += 2; since 3n+1 is strictly even for all odd n.

use std::collections::HashSet;

use crate::ast::BinaryOp;
use crate::span::Span;
use crate::typecheck::types::Type;
use crate::typecheck::typed_ast::{
    TypedBlock, TypedExpr, TypedLiteral, TypedProgram, TypedStmt,
};

pub fn optimize_program(program: &mut TypedProgram) {
    for func in &mut program.functions {
        optimize_block(&mut func.body);
    }
}

pub fn optimize_block(block: &mut TypedBlock) {
    // Recursively optimize nested blocks first
    for stmt in &mut block.stmts {
        match stmt {
            TypedStmt::If { then_branch, else_branch, .. } => {
                optimize_block(then_branch);
                if let Some(eb) = else_branch {
                    optimize_block(eb);
                }
            }
            TypedStmt::While { body, .. } => {
                optimize_block(body);
            }
            _ => {}
        }
    }

    // Pass 1: Active Flag Elimination (canonical while condition simplification)
    try_eliminate_active_flags(block);

    // Pass 2: Split Isolated Accumulator If
    try_split_isolated_accumulator_if(block);

    // Pass 3: Trailing Zero Shift Loops -> ctz
    try_optimize_trailing_zero_loops(block);

    // Pass 4: Brian Kernighan Popcount Loops -> popcnt
    try_optimize_brian_kernighan_popcount(block);
}

// Transform:
// let mut active: bool = true;
// while active {
//     if bound_cond {
//         ...
//         if exit_cond { active = false; } else { ... }
//     } else {
//         active = false;
//     }
// }
// into:
// while bound_cond {
//     ...
//     if exit_cond { break; } else { ... }
// }
fn try_eliminate_active_flags(block: &mut TypedBlock) {
    let mut i = 0;
    while i + 1 < block.stmts.len() {
        let active_var_opt = match &block.stmts[i] {
            TypedStmt::Let { name, value, .. } | TypedStmt::Assign { name, value, .. } => {
                if let TypedExpr::Literal { lit: TypedLiteral::Bool(true), .. } = value {
                    Some(name.clone())
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(active_name) = active_var_opt {
            let mut transformed_while = None;
            if let TypedStmt::While {
                condition: TypedExpr::Ident { name: cond_name, .. },
                body,
                span,
            } = &block.stmts[i + 1] {
                if cond_name == &active_name && body.stmts.len() == 1 {
                    if let TypedStmt::If {
                        condition: inner_cond,
                        then_branch,
                        else_branch: Some(else_branch),
                        ..
                    } = &body.stmts[0]
                    {
                        if is_single_assign_bool_false(else_branch, &active_name) {
                            let mut new_then = (*then_branch).clone();
                            replace_active_assign_with_break(&mut new_then, &active_name);
                            transformed_while = Some(TypedStmt::While {
                                condition: (*inner_cond).clone(),
                                body: new_then,
                                span: *span,
                            });
                        }
                    }
                }
            }

            if let Some(new_while) = transformed_while {
                block.stmts[i + 1] = new_while;
                // Check if active_name is used in any subsequent statements
                let is_used_later = block.stmts[i + 2..]
                    .iter()
                    .any(|s| is_var_referenced_in_stmt(s, &active_name));
                if !is_used_later {
                    block.stmts.remove(i);
                } else {
                    i += 1;
                }
                continue;
            }
        }
        i += 1;
    }
}

fn is_single_assign_bool_false(block: &TypedBlock, var_name: &str) -> bool {
    if block.stmts.len() != 1 {
        return false;
    }
    match &block.stmts[0] {
        TypedStmt::Assign { name, value, .. } => {
            if name == var_name {
                if let TypedExpr::Literal { lit: TypedLiteral::Bool(false), .. } = value {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

fn replace_active_assign_with_break(block: &mut TypedBlock, var_name: &str) {
    for stmt in &mut block.stmts {
        match stmt {
            TypedStmt::Assign { name, value, span } => {
                if name == var_name {
                    if let TypedExpr::Literal { lit: TypedLiteral::Bool(false), .. } = value {
                        *stmt = TypedStmt::Break(*span);
                    }
                }
            }
            TypedStmt::If { then_branch, else_branch, .. } => {
                replace_active_assign_with_break(then_branch, var_name);
                if let Some(eb) = else_branch {
                    replace_active_assign_with_break(eb, var_name);
                }
            }
            TypedStmt::While { body, .. } => {
                replace_active_assign_with_break(body, var_name);
            }
            _ => {}
        }
    }
}

fn is_var_referenced_in_stmt(stmt: &TypedStmt, var_name: &str) -> bool {
    match stmt {
        TypedStmt::Let { value, .. } => is_var_referenced_in_expr(value, var_name),
        TypedStmt::Assign { name, value, .. } => {
            name == var_name || is_var_referenced_in_expr(value, var_name)
        }
        TypedStmt::IndexAssign { target, index, value, .. } => {
            target == var_name
                || is_var_referenced_in_expr(index, var_name)
                || is_var_referenced_in_expr(value, var_name)
        }
        TypedStmt::FieldAssign { target, value, .. } => {
            target == var_name || is_var_referenced_in_expr(value, var_name)
        }
        TypedStmt::Expr(expr) => is_var_referenced_in_expr(expr, var_name),
        TypedStmt::If { condition, then_branch, else_branch, .. } => {
            is_var_referenced_in_expr(condition, var_name)
                || then_branch.stmts.iter().any(|s| is_var_referenced_in_stmt(s, var_name))
                || else_branch
                    .as_ref()
                    .is_some_and(|eb| eb.stmts.iter().any(|s| is_var_referenced_in_stmt(s, var_name)))
        }
        TypedStmt::While { condition, body, .. } => {
            is_var_referenced_in_expr(condition, var_name)
                || body.stmts.iter().any(|s| is_var_referenced_in_stmt(s, var_name))
        }
        TypedStmt::Return(opt_expr, ..) => {
            opt_expr.as_ref().is_some_and(|e| is_var_referenced_in_expr(e, var_name))
        }
        TypedStmt::Break(..) | TypedStmt::Continue(..) => false,
        TypedStmt::For { var, lo, hi, body, .. } => {
            var == var_name
                || is_var_referenced_in_expr(lo, var_name)
                || is_var_referenced_in_expr(hi, var_name)
                || body.stmts.iter().any(|s| is_var_referenced_in_stmt(s, var_name))
        }
    }
}

fn is_var_referenced_in_expr(expr: &TypedExpr, var_name: &str) -> bool {
    match expr {
        TypedExpr::Ident { name, .. } => name == var_name,
        TypedExpr::Unary { expr, .. } => is_var_referenced_in_expr(expr, var_name),
        TypedExpr::Binary { left, right, .. } => {
            is_var_referenced_in_expr(left, var_name) || is_var_referenced_in_expr(right, var_name)
        }
        TypedExpr::Call { args, .. } => {
            args.iter().any(|a| is_var_referenced_in_expr(a, var_name))
        }
        TypedExpr::ArrayLiteral { elements, .. } => {
            elements.iter().any(|e| is_var_referenced_in_expr(e, var_name))
        }
        TypedExpr::Index { target, index, .. } => {
            is_var_referenced_in_expr(target, var_name) || is_var_referenced_in_expr(index, var_name)
        }
        TypedExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, e)| is_var_referenced_in_expr(e, var_name))
        }
        TypedExpr::FieldAccess { target, .. } => {
            is_var_referenced_in_expr(target, var_name)
        }
        TypedExpr::Match { scrutinee, arms, .. } => {
            is_var_referenced_in_expr(scrutinee, var_name)
                || arms.iter().any(|arm| is_var_referenced_in_expr(&arm.body, var_name))
        }
        _ => false,
    }
}

// Transform:
// while curr > 1 {
//     if curr % 2 == 0 {
//         curr = curr / 2;
//     } else {
//         curr = curr * 3 + 1;
//     }
//     total_steps = total_steps + 1;
// }
// into:

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
        TypedExpr::ArrayLiteral { elements, .. } => {
            for e in elements {
                collect_read_vars_expr(e, reads);
            }
        }
        TypedExpr::Index { target, index, .. } => {
            collect_read_vars_expr(target, reads);
            collect_read_vars_expr(index, reads);
        }
        TypedExpr::StructLiteral { fields, .. } => {
            for (_, e) in fields {
                collect_read_vars_expr(e, reads);
            }
        }
        TypedExpr::FieldAccess { target, .. } => {
            collect_read_vars_expr(target, reads);
        }
        TypedExpr::Match { scrutinee, arms, .. } => {
            collect_read_vars_expr(scrutinee, reads);
            for arm in arms {
                collect_read_vars_expr(&arm.body, reads);
            }
        }
        _ => {}
    }
}

fn collect_read_vars_stmts(stmts: &[TypedStmt], reads: &mut HashSet<String>) {
    for s in stmts {
        match s {
            TypedStmt::Let { value, .. } | TypedStmt::Assign { value, .. } => {
                collect_read_vars_expr(value, reads);
            }
            TypedStmt::IndexAssign { index, value, .. } => {
                collect_read_vars_expr(index, reads);
                collect_read_vars_expr(value, reads);
            }
            TypedStmt::FieldAssign { target, value, .. } => {
                reads.insert(target.clone());
                collect_read_vars_expr(value, reads);
            }
            TypedStmt::Expr(expr) => {
                collect_read_vars_expr(expr, reads);
            }
            TypedStmt::If { condition, then_branch, else_branch, .. } => {
                collect_read_vars_expr(condition, reads);
                collect_read_vars_stmts(&then_branch.stmts, reads);
                if let Some(eb) = else_branch {
                    collect_read_vars_stmts(&eb.stmts, reads);
                }
            }
            TypedStmt::While { condition, body, .. } => {
                collect_read_vars_expr(condition, reads);
                collect_read_vars_stmts(&body.stmts, reads);
            }
            TypedStmt::Return(Some(e), ..) => {
                collect_read_vars_expr(e, reads);
            }
            _ => {}
        }
    }
}

fn collect_mutated_vars_stmts(stmts: &[TypedStmt], mutated: &mut HashSet<String>) {
    for s in stmts {
        match s {
            TypedStmt::Let { name, .. } | TypedStmt::Assign { name, .. } => {
                mutated.insert(name.clone());
            }
            TypedStmt::IndexAssign { target, .. } => {
                mutated.insert(target.clone());
            }
            TypedStmt::FieldAssign { target, .. } => {
                mutated.insert(target.clone());
            }
            TypedStmt::If { then_branch, else_branch, .. } => {
                collect_mutated_vars_stmts(&then_branch.stmts, mutated);
                if let Some(eb) = else_branch {
                    collect_mutated_vars_stmts(&eb.stmts, mutated);
                }
            }
            TypedStmt::While { body, .. } => {
                collect_mutated_vars_stmts(&body.stmts, mutated);
            }
            _ => {}
        }
    }
}

fn is_simple_inc_dec(stmt: &TypedStmt) -> Option<String> {
    if let TypedStmt::Assign { name, value: TypedExpr::Binary { op, left, right, ty, .. }, .. } = stmt {
        if !ty.is_integer() {
            return None;
        }
        let is_add = *op == BinaryOp::Add;
        let is_sub = *op == BinaryOp::Sub;
        if !is_add && !is_sub {
            return None;
        }
        let is_one = |e: &TypedExpr| -> bool {
            matches!(e, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. })
        };
        let is_name = |e: &TypedExpr| -> bool {
            matches!(e, TypedExpr::Ident { name: n, .. } if n == name)
        };
        let is_inc = (is_name(left) && is_one(right)) || (is_add && is_one(left) && is_name(right));
        let is_dec = is_sub && is_name(left) && is_one(right);
        if is_inc || is_dec {
            return Some(name.clone());
        }
    }
    None
}

fn try_split_isolated_accumulator_if(block: &mut TypedBlock) {
    let mut i = 0;
    while i < block.stmts.len() {
        let should_split = if let TypedStmt::If { condition, then_branch, else_branch, .. } = &block.stmts[i] {
            if then_branch.stmts.len() > 1 {
                if let Some(var_name) = is_simple_inc_dec(&then_branch.stmts[0]) {
                    let mut cond_reads = HashSet::new();
                    collect_read_vars_expr(condition, &mut cond_reads);

                    let mut rem_reads = HashSet::new();
                    collect_read_vars_stmts(&then_branch.stmts[1..], &mut rem_reads);

                    let mut rem_writes = HashSet::new();
                    collect_mutated_vars_stmts(&then_branch.stmts[1..], &mut rem_writes);

                    let mut else_reads = HashSet::new();
                    let mut else_writes = HashSet::new();
                    if let Some(eb) = else_branch {
                        collect_read_vars_stmts(&eb.stmts, &mut else_reads);
                        collect_mutated_vars_stmts(&eb.stmts, &mut else_writes);
                    }

                    !cond_reads.contains(&var_name)
                        && !rem_reads.contains(&var_name)
                        && !rem_writes.contains(&var_name)
                        && !else_reads.contains(&var_name)
                        && !else_writes.contains(&var_name)
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if should_split {
            if let TypedStmt::If { condition, then_branch, span, .. } = &mut block.stmts[i] {
                let first_stmt = then_branch.stmts.remove(0);
                let if_inc = TypedStmt::If {
                    condition: condition.clone(),
                    then_branch: TypedBlock {
                        stmts: vec![first_stmt],
                        span: then_branch.span,
                    },
                    else_branch: None,
                    span: *span,
                };
                block.stmts.insert(i, if_inc);
                i += 2;
                continue;
            }
        }
        i += 1;
    }
}

fn is_int_zero(expr: &TypedExpr) -> bool {
    matches!(expr, TypedExpr::Literal { lit: TypedLiteral::Int(0, _), .. })
}

fn is_int_one(expr: &TypedExpr) -> bool {
    matches!(expr, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. })
}

fn match_bit_and_one(expr: &TypedExpr) -> Option<&TypedExpr> {
    if let TypedExpr::Binary { op: BinaryOp::BitAnd, left, right, .. } = expr {
        if is_int_one(right) {
            return Some(&**left);
        } else if is_int_one(left) {
            return Some(&**right);
        }
    }
    None
}

fn try_optimize_trailing_zero_loops(block: &mut TypedBlock) {
    let mut i = 0;
    while i < block.stmts.len() {
        let mut replacement: Option<Vec<TypedStmt>> = None;
        if let TypedStmt::While { condition: TypedExpr::Binary { op: BinaryOp::Eq, left, right, .. }, body, span } = &block.stmts[i] {
                let inner = if is_int_zero(right) {
                    match_bit_and_one(left)
                } else if is_int_zero(left) {
                    match_bit_and_one(right)
                } else {
                    None
                };

                if let Some(inner_expr) = inner {
                    // Pattern 1: Single-variable shift loop
                    // while (v & 1) == 0 { v = v >> 1; }
                    if let TypedExpr::Ident { name: v_name, ty: v_ty, .. } = inner_expr {
                        if body.stmts.len() == 1 {
                            if let TypedStmt::Assign { name: assign_name, value, .. } = &body.stmts[0] {
                                if assign_name == v_name {
                                    if let TypedExpr::Binary { op: BinaryOp::Shr, left: shr_left, right: shr_right, .. } = value {
                                        if let TypedExpr::Ident { name: shr_name, .. } = &**shr_left {
                                            if shr_name == v_name && is_int_one(shr_right) {
                                                // Replace with: v = v >> ctz(v);
                                                let assign = TypedStmt::Assign {
                                                    name: v_name.clone(),
                                                    value: TypedExpr::Binary {
                                                        op: BinaryOp::Shr,
                                                        left: Box::new(TypedExpr::Ident {
                                                            name: v_name.clone(),
                                                            ty: v_ty.clone(),
                                                            span: *span,
                                                        }),
                                                        right: Box::new(TypedExpr::Call {
                                                            callee: "ctz".to_string(),
                                                            args: vec![TypedExpr::Ident {
                                                                name: v_name.clone(),
                                                                ty: v_ty.clone(),
                                                                span: *span,
                                                            }],
                                                            ty: v_ty.clone(),
                                                            span: *span,
                                                        }),
                                                        ty: v_ty.clone(),
                                                        span: *span,
                                                    },
                                                    span: *span,
                                                };
                                                replacement = Some(vec![assign]);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Pattern 2: Two-variable shift loop with shared shift counter
                    // while ((u | v) & 1) == 0 { u = u >> 1; v = v >> 1; shift = shift + 1; }
                    if replacement.is_none() {
                        if let TypedExpr::Binary { op: BinaryOp::BitOr, left: or_left, right: or_right, .. } = inner_expr {
                            if let (TypedExpr::Ident { name: u_name, ty: u_ty, .. }, TypedExpr::Ident { name: v_name, ty: v_ty, .. }) = (&**or_left, &**or_right) {
                                if body.stmts.len() == 3 {
                                    let mut has_u_shr = false;
                                    let mut has_v_shr = false;
                                    let mut shift_var: Option<(String, Type)> = None;

                                    for stmt in &body.stmts {
                                        if let TypedStmt::Assign { name, value, .. } = stmt {
                                            if name == u_name {
                                                if let TypedExpr::Binary { op: BinaryOp::Shr, left: sl, right: sr, .. } = value {
                                                    if let TypedExpr::Ident { name: id, .. } = &**sl {
                                                        if id == u_name && is_int_one(sr) {
                                                            has_u_shr = true;
                                                            continue;
                                                        }
                                                    }
                                                }
                                            }
                                            if name == v_name {
                                                if let TypedExpr::Binary { op: BinaryOp::Shr, left: sl, right: sr, .. } = value {
                                                    if let TypedExpr::Ident { name: id, .. } = &**sl {
                                                        if id == v_name && is_int_one(sr) {
                                                            has_v_shr = true;
                                                            continue;
                                                        }
                                                    }
                                                }
                                            }
                                            // Check shift = shift + 1 or 1 + shift
                                            if let TypedExpr::Binary { op: BinaryOp::Add, left: al, right: ar, ty, .. } = value {
                                                let is_inc = (matches!(&**al, TypedExpr::Ident { name: id, .. } if id == name) && is_int_one(ar))
                                                    || (is_int_one(al) && matches!(&**ar, TypedExpr::Ident { name: id, .. } if id == name));
                                                if is_inc {
                                                    shift_var = Some((name.clone(), ty.clone()));
                                                }
                                            }
                                        }
                                    }

                                    if has_u_shr && has_v_shr {
                                        if let Some((shift_name, shift_ty)) = shift_var {
                                            let tz_k_name = format!("__tz_{}_{}", u_name, v_name);
                                            let tz_let = TypedStmt::Let {
                                                name: tz_k_name.clone(),
                                                is_mutable: false,
                                                ty: Type::I64,
                                                value: TypedExpr::Call {
                                                    callee: "ctz".to_string(),
                                                    args: vec![TypedExpr::Binary {
                                                        op: BinaryOp::BitOr,
                                                        left: Box::new(TypedExpr::Ident { name: u_name.clone(), ty: u_ty.clone(), span: *span }),
                                                        right: Box::new(TypedExpr::Ident { name: v_name.clone(), ty: v_ty.clone(), span: *span }),
                                                        ty: u_ty.clone(),
                                                        span: *span,
                                                    }],
                                                    ty: Type::I64,
                                                    span: *span,
                                                },
                                                span: *span,
                                            };
                                            let shift_assign = TypedStmt::Assign {
                                                name: shift_name.clone(),
                                                value: TypedExpr::Binary {
                                                    op: BinaryOp::Add,
                                                    left: Box::new(TypedExpr::Ident { name: shift_name, ty: shift_ty.clone(), span: *span }),
                                                    right: Box::new(TypedExpr::Ident { name: tz_k_name.clone(), ty: Type::I64, span: *span }),
                                                    ty: shift_ty,
                                                    span: *span,
                                                },
                                                span: *span,
                                            };
                                            let u_assign = TypedStmt::Assign {
                                                name: u_name.clone(),
                                                value: TypedExpr::Binary {
                                                    op: BinaryOp::Shr,
                                                    left: Box::new(TypedExpr::Ident { name: u_name.clone(), ty: u_ty.clone(), span: *span }),
                                                    right: Box::new(TypedExpr::Ident { name: tz_k_name.clone(), ty: Type::I64, span: *span }),
                                                    ty: u_ty.clone(),
                                                    span: *span,
                                                },
                                                span: *span,
                                            };
                                            let v_assign = TypedStmt::Assign {
                                                name: v_name.clone(),
                                                value: TypedExpr::Binary {
                                                    op: BinaryOp::Shr,
                                                    left: Box::new(TypedExpr::Ident { name: v_name.clone(), ty: v_ty.clone(), span: *span }),
                                                    right: Box::new(TypedExpr::Ident { name: tz_k_name, ty: Type::I64, span: *span }),
                                                    ty: v_ty.clone(),
                                                    span: *span,
                                                },
                                                span: *span,
                                            };
                                            replacement = Some(vec![tz_let, shift_assign, u_assign, v_assign]);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

        if let Some(repl) = replacement {
            let repl_len = repl.len();
            block.stmts.splice(i..=i, repl);
            i += repl_len;
        } else {
            i += 1;
        }
    }
}


fn is_name_ident(expr: &TypedExpr, name: &str) -> bool {
    matches!(expr, TypedExpr::Ident { name: n, .. } if n == name)
}

fn is_name_sub_one(expr: &TypedExpr, name: &str) -> bool {
    if let TypedExpr::Binary { op: BinaryOp::Sub, left, right, .. } = expr {
        is_name_ident(left, name) && is_int_one(right)
    } else {
        false
    }
}

fn try_optimize_brian_kernighan_popcount(block: &mut TypedBlock) {
    let mut i = 0;
    while i < block.stmts.len() {
        let mut replacement: Option<Vec<TypedStmt>> = None;
        if let TypedStmt::While { condition, body, span: _ } = &block.stmts[i] {
            let num_info = match condition {
                TypedExpr::Binary { op: BinaryOp::Ne, left, right, .. } => {
                    if is_int_zero(right) {
                        if let TypedExpr::Ident { name, ty, span } = &**left {
                            Some((name.clone(), ty.clone(), *span))
                        } else { None }
                    } else if is_int_zero(left) {
                        if let TypedExpr::Ident { name, ty, span } = &**right {
                            Some((name.clone(), ty.clone(), *span))
                        } else { None }
                    } else { None }
                }
                TypedExpr::Binary { op: BinaryOp::Gt, left, right, .. } => {
                    if is_int_zero(right) {
                        if let TypedExpr::Ident { name, ty, span } = &**left {
                            Some((name.clone(), ty.clone(), *span))
                        } else { None }
                    } else { None }
                }
                _ => None,
            };

            if let Some((num_name, num_ty, num_span)) = num_info {
                if body.stmts.len() == 2 {
                    let mut is_num_update = false;
                    let mut count_info: Option<(String, Type, Span)> = None;

                    for stmt in &body.stmts {
                        if let TypedStmt::Assign { name, value, span } = stmt {
                            if name == &num_name {
                                if let TypedExpr::Binary { op: BinaryOp::BitAnd, left, right, .. } = value {
                                    let matches_and = (is_name_sub_one(left, &num_name) && is_name_ident(right, &num_name))
                                        || (is_name_ident(left, &num_name) && is_name_sub_one(right, &num_name));
                                    if matches_and {
                                        is_num_update = true;
                                    }
                                }
                            } else {
                                if let TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } = value {
                                    let is_count_add = (is_name_ident(left, name) && is_int_one(right))
                                        || (is_int_one(left) && is_name_ident(right, name));
                                    if is_count_add {
                                        count_info = Some((name.clone(), value.ty(), *span));
                                    }
                                }
                            }
                        }
                    }

                    if is_num_update {
                        if let Some((count_name, count_ty, count_span)) = count_info {
                            let assign_count = TypedStmt::Assign {
                                name: count_name.clone(),
                                value: TypedExpr::Binary {
                                    op: BinaryOp::Add,
                                    left: Box::new(TypedExpr::Ident {
                                        name: count_name.clone(),
                                        ty: count_ty.clone(),
                                        span: count_span,
                                    }),
                                    right: Box::new(TypedExpr::Call {
                                        callee: "popcnt".to_string(),
                                        args: vec![TypedExpr::Ident {
                                            name: num_name.clone(),
                                            ty: num_ty.clone(),
                                            span: num_span,
                                        }],
                                        ty: count_ty.clone(),
                                        span: count_span,
                                    }),
                                    ty: count_ty.clone(),
                                    span: count_span,
                                },
                                span: count_span,
                            };
                            let assign_num = TypedStmt::Assign {
                                name: num_name.clone(),
                                value: TypedExpr::Literal {
                                    lit: TypedLiteral::Int(0, num_ty.clone()),
                                    ty: num_ty.clone(),
                                    span: num_span,
                                },
                                span: num_span,
                            };
                            replacement = Some(vec![assign_count, assign_num]);
                        }
                    }
                }
            }
        }

        if let Some(repl) = replacement {
            let repl_len = repl.len();
            block.stmts.splice(i..=i, repl);
            i += repl_len;
        } else {
            i += 1;
        }
    }
}
