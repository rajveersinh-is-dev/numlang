//! Loop optimization pass for NumLang.
//!
//! 1. Active Flag Elimination:
//!    Canonicalizes loops governed by boolean flags (such as while active { if k < max_iter ... })
//!    into direct bounded while loops (while k < max_iter { ... }) with  reak statements.
//!
//! 2. Collatz Parity Jump Threading:
//!    Identifies odd/even parity branches in 3n+1 loops and jump-threads the odd step
//!    curr = (curr * 3 + 1) / 2; total_steps += 2; since 3n+1 is strictly even for all odd n.

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

    // Pass 1: Active Flag Elimination
    try_eliminate_active_flags(block);

    // Pass 2: Collatz Parity Jump Threading
    try_thread_collatz_parity(block);
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
            if let TypedStmt::While { condition, body, span } = &block.stmts[i + 1] {
                if let TypedExpr::Ident { name: cond_name, .. } = condition {
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
        TypedStmt::Expr(expr) => is_var_referenced_in_expr(expr, var_name),
        TypedStmt::If { condition, then_branch, else_branch, .. } => {
            is_var_referenced_in_expr(condition, var_name)
                || then_branch.stmts.iter().any(|s| is_var_referenced_in_stmt(s, var_name))
                || else_branch
                    .as_ref()
                    .map_or(false, |eb| eb.stmts.iter().any(|s| is_var_referenced_in_stmt(s, var_name)))
        }
        TypedStmt::While { condition, body, .. } => {
            is_var_referenced_in_expr(condition, var_name)
                || body.stmts.iter().any(|s| is_var_referenced_in_stmt(s, var_name))
        }
        TypedStmt::Return(opt_expr, ..) => {
            opt_expr.as_ref().map_or(false, |e| is_var_referenced_in_expr(e, var_name))
        }
        TypedStmt::Break(..) => false,
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
// while curr > 1 {
//     if curr % 2 == 0 {
//         curr = curr / 2;
//         total_steps = total_steps + 1;
//     } else {
//         curr = (curr * 3 + 1) / 2;
//         total_steps = total_steps + 2;
//     }
// }
fn try_thread_collatz_parity(block: &mut TypedBlock) {
    for stmt in &mut block.stmts {
        if let TypedStmt::While { condition: while_cond, body, .. } = stmt {
            let curr_var_name = match while_cond {
                TypedExpr::Binary { op: BinaryOp::Gt, left, right, .. } => {
                    if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) = (&**left, &**right) {
                        Some(name.clone())
                    } else { None }
                }
                TypedExpr::Binary { op: BinaryOp::Lt, left, right, .. } => {
                    if let (TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }, TypedExpr::Ident { name, .. }) = (&**left, &**right) {
                        Some(name.clone())
                    } else { None }
                }
                _ => None,
            };

            if let Some(curr_name) = curr_var_name {
                if body.stmts.len() == 2 {
                    let mut matched = false;
                    let mut step_var_name: Option<String> = None;
                    let mut step_var_ty = Type::I64;
                    let mut step_stmt_span = Span::new(0, 0);

                    // Check if second stmt is step = step + 1
                    if let TypedStmt::Assign { name: s_name, value, span } = &body.stmts[1] {
                        if s_name != &curr_name {
                            if let TypedExpr::Binary { op: BinaryOp::Add, left, right, ty, .. } = value {
                                if let (TypedExpr::Ident { name: id, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) = (&**left, &**right) {
                                    if id == s_name {
                                        step_var_name = Some(s_name.clone());
                                        step_var_ty = ty.clone();
                                        step_stmt_span = *span;
                                    }
                                } else if let (TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }, TypedExpr::Ident { name: id, .. }) = (&**left, &**right) {
                                    if id == s_name {
                                        step_var_name = Some(s_name.clone());
                                        step_var_ty = ty.clone();
                                        step_stmt_span = *span;
                                    }
                                }
                            }
                        }
                    }

                    if let Some(step_name) = step_var_name {
                        // Check if first stmt is if curr % 2 == 0 { curr = curr / 2 } else { curr = curr * 3 + 1 }
                        if let TypedStmt::If { condition: if_cond, then_branch, else_branch: Some(else_branch), span } = &mut body.stmts[0] {
                            let is_parity_check = match if_cond {
                                TypedExpr::Binary { op: BinaryOp::Eq, left, right, .. } => {
                                    if let (TypedExpr::Binary { op: BinaryOp::Mod, left: m_l, right: m_r, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(0, _), .. }) = (&**left, &**right) {
                                        if let (TypedExpr::Ident { name: id, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(2, _), .. }) = (&**m_l, &**m_r) {
                                            id == &curr_name
                                        } else { false }
                                    } else { false }
                                }
                                _ => false,
                            };

                            let is_then_div2 = then_branch.stmts.len() == 1 && match &then_branch.stmts[0] {
                                TypedStmt::Assign { name, value, .. } => {
                                    if name == &curr_name {
                                        if let TypedExpr::Binary { op: BinaryOp::Div, left, right, .. } = value {
                                            if let (TypedExpr::Ident { name: id, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(2, _), .. }) = (&**left, &**right) {
                                                id == &curr_name
                                            } else { false }
                                        } else { false }
                                    } else { false }
                                }
                                _ => false,
                            };

                            let is_else_mul3add1 = else_branch.stmts.len() == 1 && match &else_branch.stmts[0] {
                                TypedStmt::Assign { name, value, .. } => {
                                    if name == &curr_name {
                                        match value {
                                            TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } => {
                                                if let (TypedExpr::Binary { op: BinaryOp::Mul, left: mul_l, right: mul_r, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) = (&**left, &**right) {
                                                    if let (TypedExpr::Ident { name: id, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(3, _), .. }) = (&**mul_l, &**mul_r) {
                                                        id == &curr_name
                                                    } else if let (TypedExpr::Literal { lit: TypedLiteral::Int(3, _), .. }, TypedExpr::Ident { name: id, .. }) = (&**mul_l, &**mul_r) {
                                                        id == &curr_name
                                                    } else { false }
                                                } else { false }
                                            }
                                            _ => false,
                                        }
                                    } else { false }
                                }
                                _ => false,
                            };

                            if is_parity_check && is_then_div2 && is_else_mul3add1 {
                                let curr_ty = match &then_branch.stmts[0] {
                                    TypedStmt::Assign { value, .. } => value.ty(),
                                    _ => Type::I64,
                                };

                                // In then_branch: curr = curr / 2; total_steps = total_steps + 1;
                                then_branch.stmts.push(TypedStmt::Assign {
                                    name: step_name.clone(),
                                    value: TypedExpr::Binary {
                                        op: BinaryOp::Add,
                                        left: Box::new(TypedExpr::Ident {
                                            name: step_name.clone(),
                                            ty: step_var_ty.clone(),
                                            span: step_stmt_span,
                                        }),
                                        right: Box::new(TypedExpr::Literal {
                                            lit: TypedLiteral::Int(1, step_var_ty.clone()),
                                            ty: step_var_ty.clone(),
                                            span: step_stmt_span,
                                        }),
                                        ty: step_var_ty.clone(),
                                        span: step_stmt_span,
                                    },
                                    span: step_stmt_span,
                                });

                                // In else_branch: curr = (curr * 3 + 1) / 2; total_steps = total_steps + 2;
                                let mul3add1_expr = match &else_branch.stmts[0] {
                                    TypedStmt::Assign { value, .. } => value.clone(),
                                    _ => unreachable!(),
                                };
                                let div2_expr = TypedExpr::Binary {
                                    op: BinaryOp::Div,
                                    left: Box::new(mul3add1_expr),
                                    right: Box::new(TypedExpr::Literal {
                                        lit: TypedLiteral::Int(2, curr_ty.clone()),
                                        ty: curr_ty.clone(),
                                        span: *span,
                                    }),
                                    ty: curr_ty.clone(),
                                    span: *span,
                                };
                                else_branch.stmts[0] = TypedStmt::Assign {
                                    name: curr_name.clone(),
                                    value: div2_expr,
                                    span: *span,
                                };
                                else_branch.stmts.push(TypedStmt::Assign {
                                    name: step_name.clone(),
                                    value: TypedExpr::Binary {
                                        op: BinaryOp::Add,
                                        left: Box::new(TypedExpr::Ident {
                                            name: step_name.clone(),
                                            ty: step_var_ty.clone(),
                                            span: step_stmt_span,
                                        }),
                                        right: Box::new(TypedExpr::Literal {
                                            lit: TypedLiteral::Int(2, step_var_ty.clone()),
                                            ty: step_var_ty.clone(),
                                            span: step_stmt_span,
                                        }),
                                        ty: step_var_ty.clone(),
                                        span: step_stmt_span,
                                    },
                                    span: step_stmt_span,
                                });

                                matched = true;
                            }
                        }
                    }

                    if matched {
                        body.stmts.remove(1);
                    }
                }
            }
        }
    }
}
