//! Loop escape analysis and induction variable analysis.

use std::collections::HashSet;
use crate::ast::BinaryOp;
use crate::typecheck::{TypedBlock, TypedExpr, TypedLiteral, TypedStmt};
use super::ast_stmt::FunctionTranslationState;

impl<'a> FunctionTranslationState<'a> {
    pub(crate) fn should_reset_loop_iteration(&self, body: &TypedBlock) -> bool {
        let outer_vars: HashSet<String> = self.variables.keys().cloned().collect();
        if !Self::block_has_allocations(body) {
            return false;
        }
        !Self::block_allocations_escape(body, &outer_vars)
    }

    pub(crate) fn block_has_allocations(body: &TypedBlock) -> bool {
        for stmt in &body.stmts {
            match stmt {
                TypedStmt::Let { value, .. } | TypedStmt::Assign { value, .. } | TypedStmt::Expr(value) => {
                    if Self::expr_has_allocations(value) {
                        return true;
                    }
                }
                TypedStmt::If { condition, then_branch, else_branch, .. } => {
                    if Self::expr_has_allocations(condition)
                        || Self::block_has_allocations(then_branch)
                        || else_branch.as_ref().is_some_and(Self::block_has_allocations)
                    {
                        return true;
                    }
                }
                TypedStmt::While { condition, body, .. } => {
                    if Self::expr_has_allocations(condition) || Self::block_has_allocations(body) {
                        return true;
                    }
                }
                TypedStmt::For { lo, hi, body, .. }
                    if Self::expr_has_allocations(lo)
                        || Self::expr_has_allocations(hi)
                        || Self::block_has_allocations(body) =>
                {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    pub(crate) fn expr_has_allocations(expr: &TypedExpr) -> bool {
        match expr {
            TypedExpr::Box { .. } | TypedExpr::Lambda { .. } => true,
            TypedExpr::Call { .. } | TypedExpr::CallIndirect { .. } => true,
            TypedExpr::Binary { left, right, .. } => {
                Self::expr_has_allocations(left) || Self::expr_has_allocations(right)
            }
            TypedExpr::Unary { expr, .. } => Self::expr_has_allocations(expr),
            TypedExpr::StructLiteral { fields, .. } => {
                fields.iter().any(|(_, e)| Self::expr_has_allocations(e))
            }
            TypedExpr::EnumConstructor { args, .. } => {
                args.iter().any(Self::expr_has_allocations)
            }
            TypedExpr::FieldAccess { target, .. } => Self::expr_has_allocations(target),
            TypedExpr::Index { target, index, .. } => {
                Self::expr_has_allocations(target) || Self::expr_has_allocations(index)
            }
            TypedExpr::Match { scrutinee, arms, .. } => {
                Self::expr_has_allocations(scrutinee)
                    || arms.iter().any(|arm| Self::expr_has_allocations(&arm.body))
            }
            _ => false,
        }
    }

    pub(crate) fn block_allocations_escape(body: &TypedBlock, outer_vars: &HashSet<String>) -> bool {
        let mut local_vars = HashSet::new();
        Self::stmt_allocations_escape_inner(body, outer_vars, &mut local_vars)
    }

    pub(crate) fn stmt_allocations_escape_inner(
        body: &TypedBlock,
        outer_vars: &HashSet<String>,
        local_vars: &mut HashSet<String>,
    ) -> bool {
        for stmt in &body.stmts {
            match stmt {
                TypedStmt::Let { name, .. } => {
                    local_vars.insert(name.clone());
                }
                TypedStmt::Assign { name, value, .. } => {
                    if !local_vars.contains(name) && outer_vars.contains(name) && value.ty().contains_heap() {
                        return true;
                    }
                }
                TypedStmt::IndexAssign { target, value, .. } => {
                    if !local_vars.contains(target) && outer_vars.contains(target) && value.ty().contains_heap() {
                        return true;
                    }
                }
                TypedStmt::FieldAssign { target, value, .. } => {
                    if !local_vars.contains(target) && outer_vars.contains(target) && value.ty().contains_heap() {
                        return true;
                    }
                }
                TypedStmt::If { then_branch, else_branch, .. } => {
                    if Self::stmt_allocations_escape_inner(then_branch, outer_vars, local_vars) {
                        return true;
                    }
                    if let Some(eb) = else_branch {
                        if Self::stmt_allocations_escape_inner(eb, outer_vars, local_vars) {
                            return true;
                        }
                    }
                }
                TypedStmt::While { body, .. } => {
                    if Self::stmt_allocations_escape_inner(body, outer_vars, local_vars) {
                        return true;
                    }
                }
                TypedStmt::For { var, body, .. } => {
                    local_vars.insert(var.clone());
                    if Self::stmt_allocations_escape_inner(body, outer_vars, local_vars) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }


    pub(crate) fn get_small_constant_loop_info(condition: &TypedExpr) -> Option<(&str, usize)> {
        match condition {
            TypedExpr::Binary { op: BinaryOp::Lt, left, right, .. } => {
                if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(n, _), .. }) = (&**left, &**right) {
                    if *n > 0 && *n <= 16 {
                        return Some((name.as_str(), *n as usize));
                    }
                }
                None
            }
            TypedExpr::Binary { op: BinaryOp::Le, left, right, .. } => {
                if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(n, _), .. }) = (&**left, &**right) {
                    let limit = *n + 1;
                    if limit > 0 && limit <= 16 {
                        return Some((name.as_str(), limit as usize));
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn is_var_initialized_to_zero(stmt: &TypedStmt, var_name: &str) -> bool {
        match stmt {
            TypedStmt::Let { name, value, .. } | TypedStmt::Assign { name, value, .. } => {
                if name == var_name {
                    if let TypedExpr::Literal { lit: TypedLiteral::Int(0, _), .. } = value {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    pub(crate) fn var_mutations_in_block(block: &TypedBlock, var_name: &str) -> usize {
        let mut count = 0;
        for s in &block.stmts {
            match s {
                TypedStmt::Assign { name, .. } if name == var_name => count += 1,
                TypedStmt::If { then_branch, else_branch, .. } => {
                    count += Self::var_mutations_in_block(then_branch, var_name);
                    if let Some(eb) = else_branch {
                        count += Self::var_mutations_in_block(eb, var_name);
                    }
                }
                TypedStmt::While { body, .. } => {
                    count += Self::var_mutations_in_block(body, var_name);
                }
                _ => {}
            }
        }
        count
    }

    pub(crate) fn is_simple_induction_body(body: &TypedBlock, var_name: &str) -> bool {
        if Self::block_has_allocations(body) {
            return false;
        }
        if Self::var_mutations_in_block(body, var_name) != 1 {
            return false;
        }
        let mut has_increment = false;
        for s in &body.stmts {
            match s {
                TypedStmt::While { .. } | TypedStmt::For { .. } | TypedStmt::Return(..) | TypedStmt::Break(..) | TypedStmt::Continue(..) | TypedStmt::If { .. } => return false,
                TypedStmt::Assign { name, value, .. } if name == var_name => {
                    match value {
                        TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } => {
                            let is_plus_one = match (&**left, &**right) {
                                (TypedExpr::Ident { name: l, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) => l == var_name,
                                (TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }, TypedExpr::Ident { name: r, .. }) => r == var_name,
                                _ => false,
                            };
                            if is_plus_one && !has_increment {
                                has_increment = true;
                            } else {
                                return false;
                            }
                        }
                        _ => return false,
                    }
                }
                _ => {}
            }
        }
        has_increment
    }

    pub(crate) fn match_shl_imm(expr: &TypedExpr) -> Option<(&TypedExpr, i64)> {
        if let TypedExpr::Binary { op: BinaryOp::Shl, left, right, .. } = expr {
            if let TypedExpr::Literal { lit: TypedLiteral::Int(k, _), .. } = &**right {
                return Some((&**left, *k));
            }
        }
        None
    }

    pub(crate) fn match_shr_masked(expr: &TypedExpr) -> Option<(&TypedExpr, i64)> {
        if let TypedExpr::Binary { op: BinaryOp::Shr, left, right, .. } = expr {
            if let TypedExpr::Literal { lit: TypedLiteral::Int(k, _), .. } = &**right {
                return Some((&**left, *k));
            }
        }
        if let TypedExpr::Binary { op: BinaryOp::BitAnd, left, right, .. } = expr {
            if let TypedExpr::Binary { op: BinaryOp::Shr, left: shr_l, right: shr_r, .. } = &**left {
                if let TypedExpr::Literal { lit: TypedLiteral::Int(k, _), .. } = &**shr_r {
                    let is_mask = match &**right {
                        TypedExpr::Literal { lit: TypedLiteral::Int(m, _), .. } => {
                            let expected = if *k > 0 && *k < 64 { ((1u64 << (64 - *k)) - 1) as i64 } else { 0 };
                            *m == expected
                        }
                        TypedExpr::Ident { name, .. } => name == "mask",
                        _ => false,
                    };
                    if is_mask {
                        return Some((&**shr_l, *k));
                    }
                }
            }
        }
        None
    }

    pub(crate) fn expr_has_same_target(e1: &TypedExpr, e2: &TypedExpr) -> bool {
        if let (TypedExpr::Ident { name: n1, .. }, TypedExpr::Ident { name: n2, .. }) = (e1, e2) {
            return n1 == n2;
        }
        false
    }

    pub(crate) fn try_match_rotate<'e>(l_expr: &'e TypedExpr, r_expr: &'e TypedExpr) -> Option<(&'e TypedExpr, bool, i64)> {
        if let (Some((x1, k1)), Some((x2, k2))) = (Self::match_shl_imm(l_expr), Self::match_shr_masked(r_expr)) {
            if Self::expr_has_same_target(x1, x2) && k1 + k2 == 64 && k1 > 0 && k1 < 64 {
                return Some((x1, true, k1));
            }
        }
        if let (Some((x1, k1)), Some((x2, k2))) = (Self::match_shl_imm(r_expr), Self::match_shr_masked(l_expr)) {
            if Self::expr_has_same_target(x1, x2) && k1 + k2 == 64 && k1 > 0 && k1 < 64 {
                return Some((x1, true, k1));
            }
        }
        if let (Some((x1, k1)), Some((x2, k2))) = (Self::match_shr_masked(l_expr), Self::match_shl_imm(r_expr)) {
            if Self::expr_has_same_target(x1, x2) && k1 + k2 == 64 && k1 > 0 && k1 < 64 {
                return Some((x1, false, k1));
            }
        }
        if let (Some((x1, k1)), Some((x2, k2))) = (Self::match_shr_masked(r_expr), Self::match_shl_imm(l_expr)) {
            if Self::expr_has_same_target(x1, x2) && k1 + k2 == 64 && k1 > 0 && k1 < 64 {
                return Some((x1, false, k1));
            }
        }
        None
    }


}
