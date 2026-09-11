//! Bounds Check Elimination (BCE) Optimization Pass
//!
//! Statically proves that array indexing operations (both reads `arr[i]` and
//! writes `arr[i] = v`) are guaranteed within bounds [0, len) through interval
//! analysis and symbolic loop bound propagation, eliminating redundant runtime
//! bounds check branches and panic traps.

use std::collections::HashMap;
use crate::ast::BinaryOp;
use crate::typecheck::types::Type;
use crate::typecheck::typed_ast::{
    TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedProgram, TypedStmt,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueRange {
    pub min: i64,
    pub max: i64, // inclusive
}

impl ValueRange {
    pub fn new(min: i64, max: i64) -> Self {
        Self { min, max }
    }

    pub fn point(val: i64) -> Self {
        Self { min: val, max: val }
    }

    pub fn union(&self, other: &ValueRange) -> ValueRange {
        ValueRange {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn add(&self, other: &ValueRange) -> ValueRange {
        ValueRange {
            min: self.min.saturating_add(other.min),
            max: self.max.saturating_add(other.max),
        }
    }

    pub fn sub(&self, other: &ValueRange) -> ValueRange {
        ValueRange {
            min: self.min.saturating_sub(other.max),
            max: self.max.saturating_sub(other.min),
        }
    }

    pub fn mul_const(&self, c: i64) -> ValueRange {
        if c >= 0 {
            ValueRange {
                min: self.min.saturating_mul(c),
                max: self.max.saturating_mul(c),
            }
        } else {
            ValueRange {
                min: self.max.saturating_mul(c),
                max: self.min.saturating_mul(c),
            }
        }
    }

    pub fn div_const(&self, c: i64) -> Option<ValueRange> {
        if c > 0 {
            Some(ValueRange {
                min: self.min / c,
                max: self.max / c,
            })
        } else {
            None
        }
    }

    pub fn is_within(&self, min_bound: i64, max_exclusive: i64) -> bool {
        self.min >= min_bound && self.max < max_exclusive
    }
}

#[derive(Debug, Clone)]
pub struct BceContext {
    pub var_ranges: HashMap<String, ValueRange>,
    pub array_lens: HashMap<String, usize>,
}

impl BceContext {
    pub fn new() -> Self {
        Self {
            var_ranges: HashMap::new(),
            array_lens: HashMap::new(),
        }
    }
}

pub fn optimize_program(program: &mut TypedProgram) {
    // Step 1: Collect known call site argument ranges for function parameters
    let param_ranges = collect_call_param_ranges(program);

    // Step 2: Perform interval analysis and BCE across all functions
    for func in &mut program.functions {
        optimize_function(func, &param_ranges);
    }
}

fn collect_call_param_ranges(program: &TypedProgram) -> HashMap<String, Vec<Option<ValueRange>>> {
    let mut param_ranges: HashMap<String, Vec<Option<ValueRange>>> = HashMap::new();

    // Initialize map with empty vectors
    for func in &program.functions {
        param_ranges.insert(func.name.clone(), vec![None; func.params.len()]);
    }

    // Traverse all calls
    for func in &program.functions {
        collect_calls_in_block(&func.body, &mut param_ranges);
    }

    param_ranges
}

fn collect_calls_in_block(
    block: &TypedBlock,
    param_ranges: &mut HashMap<String, Vec<Option<ValueRange>>>,
) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { value, .. } => collect_calls_in_expr(value, param_ranges),
            TypedStmt::Assign { value, .. } => collect_calls_in_expr(value, param_ranges),
            TypedStmt::IndexAssign { index, value, .. } => {
                collect_calls_in_expr(index, param_ranges);
                collect_calls_in_expr(value, param_ranges);
            }
            TypedStmt::Return(Some(e), _) => collect_calls_in_expr(e, param_ranges),
            TypedStmt::Expr(e) => collect_calls_in_expr(e, param_ranges),
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                collect_calls_in_expr(condition, param_ranges);
                collect_calls_in_block(then_branch, param_ranges);
                if let Some(eb) = else_branch {
                    collect_calls_in_block(eb, param_ranges);
                }
            }
            TypedStmt::While { condition, body, .. } => {
                collect_calls_in_expr(condition, param_ranges);
                collect_calls_in_block(body, param_ranges);
            }
            _ => {}
        }
    }
}

fn collect_calls_in_expr(
    expr: &TypedExpr,
    param_ranges: &mut HashMap<String, Vec<Option<ValueRange>>>,
) {
    match expr {
        TypedExpr::Call { callee, args, .. } => {
            if let Some(fn_params) = param_ranges.get_mut(callee) {
                for (i, arg) in args.iter().enumerate() {
                    if i < fn_params.len() {
                        if let Some(n) = as_int_const(arg) {
                            let r = ValueRange::point(n);
                            fn_params[i] = Some(match fn_params[i] {
                                Some(existing) => existing.union(&r),
                                None => r,
                            });
                        }
                    }
                }
            }
            for arg in args {
                collect_calls_in_expr(arg, param_ranges);
            }
        }
        TypedExpr::Binary { left, right, .. } => {
            collect_calls_in_expr(left, param_ranges);
            collect_calls_in_expr(right, param_ranges);
        }
        TypedExpr::Unary { expr: inner, .. } => {
            collect_calls_in_expr(inner, param_ranges);
        }
        TypedExpr::Index { target, index, .. } => {
            collect_calls_in_expr(target, param_ranges);
            collect_calls_in_expr(index, param_ranges);
        }
        TypedExpr::ArrayLiteral { elements, .. } => {
            for elem in elements {
                collect_calls_in_expr(elem, param_ranges);
            }
        }
        _ => {}
    }
}

fn optimize_function(
    func: &mut TypedFunction,
    param_ranges: &HashMap<String, Vec<Option<ValueRange>>>,
) {
    let mut ctx = BceContext::new();

    // Register parameter ranges if known
    if let Some(known_params) = param_ranges.get(&func.name) {
        for (i, param) in func.params.iter().enumerate() {
            if let Some(Some(range)) = known_params.get(i) {
                ctx.var_ranges.insert(param.name.clone(), *range);
            }
        }
    }

    // Register array parameters
    for param in &func.params {
        if let Type::Array(_, len) = &param.ty {
            ctx.array_lens.insert(param.name.clone(), *len);
        }
    }

    // Collect local array declarations
    collect_local_arrays(&func.body, &mut ctx);

    // Process blocks and statements
    process_block(&mut func.body, &mut ctx);
}

fn collect_local_arrays(block: &TypedBlock, ctx: &mut BceContext) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { name, ty, .. } => {
                if let Type::Array(_, len) = ty {
                    ctx.array_lens.insert(name.clone(), *len);
                }
            }
            TypedStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_local_arrays(then_branch, ctx);
                if let Some(eb) = else_branch {
                    collect_local_arrays(eb, ctx);
                }
            }
            TypedStmt::While { body, .. } => {
                collect_local_arrays(body, ctx);
            }
            _ => {}
        }
    }
}

fn process_block(block: &mut TypedBlock, ctx: &mut BceContext) {
    for stmt in &mut block.stmts {
        process_stmt(stmt, ctx);
    }
}

fn process_stmt(stmt: &mut TypedStmt, ctx: &mut BceContext) {
    match stmt {
        TypedStmt::Let { name, value, .. } => {
            process_expr(value, ctx);
            if let Some(r) = eval_range(value, ctx) {
                ctx.var_ranges.insert(name.clone(), r);
            }
        }

        TypedStmt::Assign { name, value, .. } => {
            process_expr(value, ctx);
            if let Some(r) = eval_range(value, ctx) {
                ctx.var_ranges
                    .entry(name.clone())
                    .and_modify(|existing| {
                        *existing = existing.union(&r);
                    })
                    .or_insert(r);
            }
        }

        TypedStmt::IndexAssign {
            target,
            index,
            value,
            is_safe,
            ..
        } => {
            process_expr(index, ctx);
            process_expr(value, ctx);

            if let Some(&len) = ctx.array_lens.get(target) {
                if let Some(r) = eval_range(index, ctx) {
                    if r.is_within(0, len as i64) {
                        *is_safe = true;
                    }
                }
            }
        }

        TypedStmt::Expr(expr) => {
            process_expr(expr, ctx);
        }

        TypedStmt::Return(Some(expr), _) => {
            process_expr(expr, ctx);
        }

        TypedStmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            process_expr(condition, ctx);

            let mut then_ctx = ctx.clone();
            refine_condition(condition, &mut then_ctx, true);
            process_block(then_branch, &mut then_ctx);

            if let Some(eb) = else_branch {
                let mut else_ctx = ctx.clone();
                refine_condition(condition, &mut else_ctx, false);
                process_block(eb, &mut else_ctx);
            }
        }

        TypedStmt::While { condition, body, .. } => {
            process_expr(condition, ctx);

            let mut loop_ctx = ctx.clone();
            refine_loop_condition(condition, &mut loop_ctx);
            process_block(body, &mut loop_ctx);
        }

        _ => {}
    }
}

fn process_expr(expr: &mut TypedExpr, ctx: &BceContext) {
    match expr {
        TypedExpr::Index {
            target,
            index,
            is_safe,
            ..
        } => {
            process_expr(target, ctx);
            process_expr(index, ctx);

            let target_len = match target.ty() {
                Type::Array(_, len) => Some(len),
                _ => {
                    if let TypedExpr::Ident { name, .. } = &**target {
                        ctx.array_lens.get(name).copied()
                    } else {
                        None
                    }
                }
            };

            if let Some(len) = target_len {
                if let Some(r) = eval_range(index, ctx) {
                    if r.is_within(0, len as i64) {
                        *is_safe = true;
                    }
                }
            }
        }

        TypedExpr::Binary { left, right, .. } => {
            process_expr(left, ctx);
            process_expr(right, ctx);
        }

        TypedExpr::Unary { expr: inner, .. } => {
            process_expr(inner, ctx);
        }

        TypedExpr::Call { args, .. } => {
            for arg in args {
                process_expr(arg, ctx);
            }
        }

        TypedExpr::ArrayLiteral { elements, .. } => {
            for elem in elements {
                process_expr(elem, ctx);
            }
        }

        _ => {}
    }
}

fn refine_condition(condition: &TypedExpr, ctx: &mut BceContext, is_then: bool) {
    match condition {
        TypedExpr::Binary {
            op: BinaryOp::Lt,
            left,
            right,
            ..
        } => {
            let left_r = eval_range(left, ctx);
            let right_r = eval_range(right, ctx);
            if is_then {
                if let (TypedExpr::Ident { name: l_var, .. }, Some(r_range)) = (&**left, right_r) {
                    let upper = r_range.max - 1;
                    let lower = ctx.var_ranges.get(l_var).map(|r| r.min).unwrap_or(0);
                    ctx.var_ranges.insert(l_var.clone(), ValueRange::new(lower, upper));
                }
                if let (TypedExpr::Ident { name: r_var, .. }, Some(l_range)) = (&**right, left_r) {
                    let lower = l_range.min + 1;
                    let upper = ctx.var_ranges.get(r_var).map(|r| r.max).unwrap_or(i64::MAX);
                    ctx.var_ranges.insert(r_var.clone(), ValueRange::new(lower, upper));
                }
            } else {
                if let (TypedExpr::Ident { name: l_var, .. }, Some(r_range)) = (&**left, right_r) {
                    let lower = r_range.min;
                    let upper = ctx.var_ranges.get(l_var).map(|r| r.max).unwrap_or(i64::MAX);
                    ctx.var_ranges.insert(l_var.clone(), ValueRange::new(lower, upper));
                }
                if let (TypedExpr::Ident { name: r_var, .. }, Some(l_range)) = (&**right, left_r) {
                    let upper = l_range.max;
                    let lower = ctx.var_ranges.get(r_var).map(|r| r.min).unwrap_or(0);
                    ctx.var_ranges.insert(r_var.clone(), ValueRange::new(lower, upper));
                }
            }
        }
        TypedExpr::Binary {
            op: BinaryOp::Le,
            left,
            right,
            ..
        } => {
            let left_r = eval_range(left, ctx);
            let right_r = eval_range(right, ctx);
            if is_then {
                if let (TypedExpr::Ident { name: l_var, .. }, Some(r_range)) = (&**left, right_r) {
                    let upper = r_range.max;
                    let lower = ctx.var_ranges.get(l_var).map(|r| r.min).unwrap_or(0);
                    ctx.var_ranges.insert(l_var.clone(), ValueRange::new(lower, upper));
                }
                if let (TypedExpr::Ident { name: r_var, .. }, Some(l_range)) = (&**right, left_r) {
                    let lower = l_range.min;
                    let upper = ctx.var_ranges.get(r_var).map(|r| r.max).unwrap_or(i64::MAX);
                    ctx.var_ranges.insert(r_var.clone(), ValueRange::new(lower, upper));
                }
            } else {
                if let (TypedExpr::Ident { name: l_var, .. }, Some(r_range)) = (&**left, right_r) {
                    let lower = r_range.min + 1;
                    let upper = ctx.var_ranges.get(l_var).map(|r| r.max).unwrap_or(i64::MAX);
                    ctx.var_ranges.insert(l_var.clone(), ValueRange::new(lower, upper));
                }
                if let (TypedExpr::Ident { name: r_var, .. }, Some(l_range)) = (&**right, left_r) {
                    let upper = l_range.max - 1;
                    let lower = ctx.var_ranges.get(r_var).map(|r| r.min).unwrap_or(0);
                    ctx.var_ranges.insert(r_var.clone(), ValueRange::new(lower, upper));
                }
            }
        }
        TypedExpr::Binary {
            op: BinaryOp::Ge,
            left,
            right,
            ..
        } => {
            let left_r = eval_range(left, ctx);
            let right_r = eval_range(right, ctx);
            if is_then {
                if let (TypedExpr::Ident { name: l_var, .. }, Some(r_range)) = (&**left, right_r) {
                    let lower = r_range.min;
                    let upper = ctx.var_ranges.get(l_var).map(|r| r.max).unwrap_or(i64::MAX);
                    ctx.var_ranges.insert(l_var.clone(), ValueRange::new(lower, upper));
                }
                if let (TypedExpr::Ident { name: r_var, .. }, Some(l_range)) = (&**right, left_r) {
                    let upper = l_range.max;
                    let lower = ctx.var_ranges.get(r_var).map(|r| r.min).unwrap_or(0);
                    ctx.var_ranges.insert(r_var.clone(), ValueRange::new(lower, upper));
                }
            }
        }
        TypedExpr::Binary {
            op: BinaryOp::Gt,
            left,
            right,
            ..
        } => {
            let left_r = eval_range(left, ctx);
            let right_r = eval_range(right, ctx);
            if is_then {
                if let (TypedExpr::Ident { name: l_var, .. }, Some(r_range)) = (&**left, right_r) {
                    let lower = r_range.min + 1;
                    let upper = ctx.var_ranges.get(l_var).map(|r| r.max).unwrap_or(i64::MAX);
                    ctx.var_ranges.insert(l_var.clone(), ValueRange::new(lower, upper));
                }
                if let (TypedExpr::Ident { name: r_var, .. }, Some(l_range)) = (&**right, left_r) {
                    let upper = l_range.max - 1;
                    let lower = ctx.var_ranges.get(r_var).map(|r| r.min).unwrap_or(0);
                    ctx.var_ranges.insert(r_var.clone(), ValueRange::new(lower, upper));
                }
            }
        }
        _ => {}
    }
}

fn refine_loop_condition(condition: &TypedExpr, ctx: &mut BceContext) {
    match condition {
        TypedExpr::Binary {
            op: BinaryOp::Lt,
            left,
            right,
            ..
        } => {
            let left_r = eval_range(left, ctx);
            let right_r = eval_range(right, ctx);
            if let (TypedExpr::Ident { name: l_var, .. }, Some(r_range)) = (&**left, right_r) {
                let upper = r_range.max - 1;
                let lower = ctx.var_ranges.get(l_var).map(|r| r.min).unwrap_or(0).max(0);
                ctx.var_ranges.insert(l_var.clone(), ValueRange::new(lower, upper));
            }
            if let (TypedExpr::Ident { name: r_var, .. }, Some(l_range)) = (&**right, left_r) {
                let lower = (l_range.min + 1).max(0);
                let upper = ctx.var_ranges.get(r_var).map(|r| r.max).unwrap_or(i64::MAX);
                ctx.var_ranges.insert(r_var.clone(), ValueRange::new(lower, upper));
            }
        }

        TypedExpr::Binary {
            op: BinaryOp::Le,
            left,
            right,
            ..
        } => {
            let left_r = eval_range(left, ctx);
            let right_r = eval_range(right, ctx);
            if let (TypedExpr::Ident { name: l_var, .. }, Some(r_range)) = (&**left, right_r) {
                let upper = r_range.max;
                let lower = ctx.var_ranges.get(l_var).map(|r| r.min).unwrap_or(0).max(0);
                ctx.var_ranges.insert(l_var.clone(), ValueRange::new(lower, upper));
            }
            if let (TypedExpr::Ident { name: r_var, .. }, Some(l_range)) = (&**right, left_r) {
                let lower = l_range.min.max(0);
                let upper = ctx.var_ranges.get(r_var).map(|r| r.max).unwrap_or(i64::MAX);
                ctx.var_ranges.insert(r_var.clone(), ValueRange::new(lower, upper));
            }
        }

        TypedExpr::Binary {
            op: BinaryOp::Gt,
            left,
            right,
            ..
        } => {
            let left_r = eval_range(left, ctx);
            let right_r = eval_range(right, ctx);
            if let (TypedExpr::Ident { name: r_var, .. }, Some(l_range)) = (&**right, left_r) {
                let upper = l_range.max - 1;
                let lower = ctx.var_ranges.get(r_var).map(|r| r.min).unwrap_or(0).max(0);
                ctx.var_ranges.insert(r_var.clone(), ValueRange::new(lower, upper));
            }
            if let (TypedExpr::Ident { name: l_var, .. }, Some(r_range)) = (&**left, right_r) {
                let lower = (r_range.min + 1).max(0);
                let upper = ctx.var_ranges.get(l_var).map(|r| r.max).unwrap_or(i64::MAX);
                ctx.var_ranges.insert(l_var.clone(), ValueRange::new(lower, upper));
            }
        }

        TypedExpr::Binary {
            op: BinaryOp::Ge,
            left,
            right,
            ..
        } => {
            let left_r = eval_range(left, ctx);
            let right_r = eval_range(right, ctx);
            if let (TypedExpr::Ident { name: r_var, .. }, Some(l_range)) = (&**right, left_r) {
                let upper = l_range.max;
                let lower = ctx.var_ranges.get(r_var).map(|r| r.min).unwrap_or(0).max(0);
                ctx.var_ranges.insert(r_var.clone(), ValueRange::new(lower, upper));
            }
            if let (TypedExpr::Ident { name: l_var, .. }, Some(r_range)) = (&**left, right_r) {
                let lower = r_range.min.max(0);
                let upper = ctx.var_ranges.get(l_var).map(|r| r.max).unwrap_or(i64::MAX);
                ctx.var_ranges.insert(l_var.clone(), ValueRange::new(lower, upper));
            }
        }

        _ => {}
    }
}

fn eval_range(expr: &TypedExpr, ctx: &BceContext) -> Option<ValueRange> {
    match expr {
        TypedExpr::Literal {
            lit: TypedLiteral::Int(n, _),
            ..
        } => Some(ValueRange::point(*n)),

        TypedExpr::Ident { name, .. } => ctx.var_ranges.get(name).copied(),

        TypedExpr::Binary {
            op, left, right, ..
        } => {
            let l_const = as_int_const(left);
            let r_const = as_int_const(right);
            let l = eval_range(left, ctx);
            let r = eval_range(right, ctx);

            match op {
                BinaryOp::Add => {
                    if let (Some(l_range), Some(r_range)) = (l, r) {
                        Some(l_range.add(&r_range))
                    } else {
                        None
                    }
                }
                BinaryOp::Sub => {
                    if let (Some(l_range), Some(r_range)) = (l, r) {
                        Some(l_range.sub(&r_range))
                    } else {
                        None
                    }
                }
                BinaryOp::Mul => {
                    if let (Some(l_range), Some(c)) = (l, r_const) {
                        Some(l_range.mul_const(c))
                    } else if let (Some(c), Some(r_range)) = (l_const, r) {
                        Some(r_range.mul_const(c))
                    } else {
                        None
                    }
                }
                BinaryOp::Div => {
                    if let (Some(l_range), Some(c)) = (l, r_const) {
                        l_range.div_const(c)
                    } else {
                        None
                    }
                }
                BinaryOp::Mod => {
                    if let Some(c) = r_const {
                        if c > 0 {
                            Some(ValueRange::new(0, c - 1))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                BinaryOp::BitAnd => {
                    if let Some(c) = r_const {
                        if c >= 0 {
                            Some(ValueRange::new(0, c))
                        } else {
                            None
                        }
                    } else if let Some(c) = l_const {
                        if c >= 0 {
                            Some(ValueRange::new(0, c))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        TypedExpr::Call { callee, args, .. } if callee == "abs" && args.len() == 1 => {
            if let Some(r) = eval_range(&args[0], ctx) {
                let max_abs = r.min.abs().max(r.max.abs());
                Some(ValueRange::new(0, max_abs))
            } else {
                None
            }
        }

        _ => None,
    }
}

fn as_int_const(expr: &TypedExpr) -> Option<i64> {
    if let TypedExpr::Literal {
        lit: TypedLiteral::Int(n, _),
        ..
    } = expr
    {
        Some(*n)
    } else {
        None
    }
}
