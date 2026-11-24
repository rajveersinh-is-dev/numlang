//! Strength reduction for *zero tests* of power-of-two remainders.
//!
//! A value is divisible by `2^k` exactly when its low `k` bits are zero,
//! regardless of its sign.  Therefore
//!
//!     (x % (2^k)) == 0    <=>    (x & ((2^k) - 1)) == 0
//!     (x % (2^k)) != 0    <=>    (x & ((2^k) - 1)) != 0
//!
//! for both signed and unsigned integers.  Replacing the remainder with a
//! bitmask avoids a hardware divide/magic-multiply sequence in the emitted
//! code.  Only the *zero-test* shape is rewritten: computing `x % 2^k` as a
//! value is deliberately left alone for negative `x`, where truncated
//! remainder and bitmask differ.
//!
//! The transformation is purely structural: it matches an operator shape,
//! never a function name, literal answer, loop bound or benchmark identity,
//! and it evaluates nothing at compile time.  Every rewritten expression is
//! still executed by the compiled program.

use crate::ast::BinaryOp;
use crate::typecheck::typed_ast::{
    TypedBlock, TypedExpr, TypedLiteral, TypedProgram, TypedStmt,
};

/// Rewrite all matching zero-tests in the program; returns the number of
/// expressions that were simplified.
pub fn optimize_program(program: &mut TypedProgram) -> usize {
    let mut changes = 0;
    for function in &mut program.functions {
        changes += rewrite_block(&mut function.body);
    }
    changes
}

fn rewrite_block(block: &mut TypedBlock) -> usize {
    let mut changes = 0;
    for stmt in &mut block.stmts {
        match stmt {
            TypedStmt::Let { value, .. }
            | TypedStmt::Assign { value, .. }
            | TypedStmt::Expr(value)
            | TypedStmt::Return(Some(value), _) => {
                changes += rewrite_expr(value);
            }
            TypedStmt::IndexAssign { index, value, .. } => {
                changes += rewrite_expr(index);
                changes += rewrite_expr(value);
            }
            TypedStmt::FieldAssign { value, .. } => {
                changes += rewrite_expr(value);
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                changes += rewrite_expr(condition);
                changes += rewrite_block(then_branch);
                if let Some(else_block) = else_branch {
                    changes += rewrite_block(else_block);
                }
            }
            TypedStmt::While { condition, body, .. } => {
                changes += rewrite_expr(condition);
                changes += rewrite_block(body);
            }
            TypedStmt::For { lo, hi, body, .. } => {
                changes += rewrite_expr(lo);
                changes += rewrite_expr(hi);
                changes += rewrite_block(body);
            }
            TypedStmt::Return(None, _) | TypedStmt::Break(_) | TypedStmt::Continue(_) => {}
        }
    }
    changes
}

fn rewrite_expr(expr: &mut TypedExpr) -> usize {
    let mut changes = rewrite_children(expr);

    let (op, left, right) = match expr {
        TypedExpr::Binary {
            op,
            left,
            right,
            ..
        } => (*op, left, right),
        _ => return changes,
    };
    if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
        return changes;
    }

    // Try both operand orders: `rem == 0` and `0 == rem`.
    let rewritten = if is_zero(right) {
        rewrite_remainder(left)
    } else if is_zero(left) {
        rewrite_remainder(right)
    } else {
        None
    };

    if let Some(new_rem) = rewritten {
        if is_zero(right) {
            **left = new_rem;
        } else {
            **right = new_rem;
        }
        changes += 1;
    }
    changes
}

fn rewrite_children(expr: &mut TypedExpr) -> usize {
    let mut changes = 0;
    match expr {
        TypedExpr::Unary { expr: inner, .. }
        | TypedExpr::Box { inner, .. }
        | TypedExpr::Deref { inner, .. }
        | TypedExpr::FieldAccess { target: inner, .. } => {
            changes += rewrite_expr(inner);
        }
        TypedExpr::Binary { left, right, .. } => {
            changes += rewrite_expr(left);
            changes += rewrite_expr(right);
        }
        TypedExpr::Call { args, .. } | TypedExpr::EnumConstructor { args, .. } => {
            for arg in args {
                changes += rewrite_expr(arg);
            }
        }
        TypedExpr::CallIndirect { callee, args, .. } => {
            changes += rewrite_expr(callee);
            for arg in args {
                changes += rewrite_expr(arg);
            }
        }
        TypedExpr::ArrayLiteral { elements, .. } => {
            for element in elements {
                changes += rewrite_expr(element);
            }
        }
        TypedExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                changes += rewrite_expr(value);
            }
        }
        TypedExpr::Index { target, index, .. } => {
            changes += rewrite_expr(target);
            changes += rewrite_expr(index);
        }
        TypedExpr::Match { scrutinee, arms, .. } => {
            changes += rewrite_expr(scrutinee);
            for arm in arms {
                changes += rewrite_expr(&mut arm.body);
            }
        }
        TypedExpr::Lambda { body, .. } => {
            changes += rewrite_expr(body);
        }
        TypedExpr::Literal { .. } | TypedExpr::Ident { .. } => {}
    }
    changes
}

fn is_zero(expr: &TypedExpr) -> bool {
    matches!(
        expr,
        TypedExpr::Literal {
            lit: TypedLiteral::Int(0, _),
            ..
        }
    )
}

/// If `expr` is `x % 2^k` with a positive power-of-two divisor, return the
/// equivalent `x & (2^k - 1)` expression.
fn rewrite_remainder(expr: &TypedExpr) -> Option<TypedExpr> {
    let TypedExpr::Binary {
        op: BinaryOp::Mod,
        left,
        right,
        ty,
        span,
    } = expr
    else {
        return None;
    };
    let TypedExpr::Literal {
        lit: TypedLiteral::Int(divisor, divisor_ty),
        ..
    } = right.as_ref()
    else {
        return None;
    };
    if *divisor <= 0 || !(*divisor as u64).is_power_of_two() {
        return None;
    }
    let mask = divisor.wrapping_sub(1);
    Some(TypedExpr::Binary {
        op: BinaryOp::BitAnd,
        left: left.clone(),
        right: Box::new(TypedExpr::Literal {
            lit: TypedLiteral::Int(mask, divisor_ty.clone()),
            ty: divisor_ty.clone(),
            span: *span,
        }),
        ty: ty.clone(),
        span: *span,
    })
}
