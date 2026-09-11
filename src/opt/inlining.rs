//! Whole-program interprocedural function inlining pass.
//!
//! Replaces call sites of eligible non-recursive functions with their inlined bodies,
//! eliminating call frame overhead and exposing argument constants to downstream
//! strength reduction and loop optimizations.

use std::collections::{HashMap, HashSet};

use crate::ast::UnaryOp;
use crate::span::Span;
use crate::typecheck::types::Type;
use crate::typecheck::typed_ast::{
    TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedProgram, TypedStmt,
};

pub fn optimize_program(program: &mut TypedProgram) {
    // 1. Build call graph and identify recursive functions.
    let recursive_funcs = find_recursive_functions(program);

    // 2. Normalize early/multiple returns in eligible non-recursive functions.
    let mut norm_counter = 0;
    for func in &mut program.functions {
        if !recursive_funcs.contains(&func.name) && func.name != "main" {
            normalize_function_returns(func, &mut norm_counter);
        }
    }

    // 3. Collect function definitions mapped by name.
    let func_defs: HashMap<String, TypedFunction> = program
        .functions
        .iter()
        .map(|f| (f.name.clone(), f.clone()))
        .collect();

    // 4. Inline eligible functions iteratively (up to 4 passes for nested inlinings).
    let mut call_counter: usize = 0;
    for _ in 0..4 {
        let mut changed = false;
        for func in &mut program.functions {
            if inline_in_function(func, &func_defs, &recursive_funcs, &mut call_counter) {
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

fn find_recursive_functions(program: &TypedProgram) -> HashSet<String> {
    let mut call_graph: HashMap<String, HashSet<String>> = HashMap::new();
    let func_names: HashSet<String> = program.functions.iter().map(|f| f.name.clone()).collect();

    for f in &program.functions {
        let mut callees = HashSet::new();
        collect_callees_block(&f.body, &func_names, &mut callees);
        call_graph.insert(f.name.clone(), callees);
    }

    let mut recursive = HashSet::new();
    for name in &func_names {
        if is_reachable(name, name, &call_graph, &mut HashSet::new()) {
            recursive.insert(name.clone());
        }
    }
    recursive
}

fn collect_callees_block(
    block: &TypedBlock,
    func_names: &HashSet<String>,
    callees: &mut HashSet<String>,
) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { value, .. }
            | TypedStmt::Assign { value, .. }
            | TypedStmt::Expr(value) => {
                collect_callees_expr(value, func_names, callees);
            }
            TypedStmt::IndexAssign { index, value, .. } => {
                collect_callees_expr(index, func_names, callees);
                collect_callees_expr(value, func_names, callees);
            }
            TypedStmt::Return(Some(expr), _) => {
                collect_callees_expr(expr, func_names, callees);
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                collect_callees_expr(condition, func_names, callees);
                collect_callees_block(then_branch, func_names, callees);
                if let Some(eb) = else_branch {
                    collect_callees_block(eb, func_names, callees);
                }
            }
            TypedStmt::While { condition, body, .. } => {
                collect_callees_expr(condition, func_names, callees);
                collect_callees_block(body, func_names, callees);
            }
            _ => {}
        }
    }
}

fn collect_callees_expr(
    expr: &TypedExpr,
    func_names: &HashSet<String>,
    callees: &mut HashSet<String>,
) {
    match expr {
        TypedExpr::Call { callee, args, .. } => {
            if func_names.contains(callee) {
                callees.insert(callee.clone());
            }
            for arg in args {
                collect_callees_expr(arg, func_names, callees);
            }
        }
        TypedExpr::Unary { expr, .. } => collect_callees_expr(expr, func_names, callees),
        TypedExpr::Binary { left, right, .. } => {
            collect_callees_expr(left, func_names, callees);
            collect_callees_expr(right, func_names, callees);
        }
        TypedExpr::ArrayLiteral { elements, .. } => {
            for el in elements {
                collect_callees_expr(el, func_names, callees);
            }
        }
        TypedExpr::Index { target, index, .. } => {
            collect_callees_expr(target, func_names, callees);
            collect_callees_expr(index, func_names, callees);
        }
        _ => {}
    }
}

fn is_reachable(
    current: &str,
    target: &str,
    graph: &HashMap<String, HashSet<String>>,
    visited: &mut HashSet<String>,
) -> bool {
    if let Some(neighbors) = graph.get(current) {
        if neighbors.contains(target) {
            return true;
        }
        for n in neighbors {
            if visited.insert(n.clone()) {
                if is_reachable(n, target, graph, visited) {
                    return true;
                }
            }
        }
    }
    false
}

fn is_inlinable(func: &TypedFunction, recursive_funcs: &HashSet<String>) -> bool {
    if recursive_funcs.contains(&func.name) {
        return false;
    }
    // Check statement count (limit to 120 statements to avoid code bloat).
    let stmt_count = count_stmts_block(&func.body);
    if stmt_count > 120 {
        return false;
    }
    // Function must have a single return at the end of its top-level body.
    has_single_return_at_end(&func.body)
}

fn count_stmts_block(block: &TypedBlock) -> usize {
    let mut count = block.stmts.len();
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                count += count_stmts_block(then_branch);
                if let Some(eb) = else_branch {
                    count += count_stmts_block(eb);
                }
            }
            TypedStmt::While { body, .. } => {
                count += count_stmts_block(body);
            }
            _ => {}
        }
    }
    count
}

fn has_single_return_at_end(block: &TypedBlock) -> bool {
    if block.stmts.is_empty() {
        return false;
    }
    // Check that the last statement is Return(Some(_))
    match block.stmts.last() {
        Some(TypedStmt::Return(Some(_), _)) => {}
        _ => return false,
    }
    // Check that no other statement in the block is a Return
    let len = block.stmts.len();
    for stmt in &block.stmts[..len - 1] {
        if contains_return(stmt) {
            return false;
        }
    }
    true
}

fn contains_return(stmt: &TypedStmt) -> bool {
    match stmt {
        TypedStmt::Return(..) => true,
        TypedStmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            then_branch.stmts.iter().any(contains_return)
                || else_branch
                    .as_ref()
                    .map_or(false, |b| b.stmts.iter().any(contains_return))
        }
        TypedStmt::While { body, .. } => body.stmts.iter().any(contains_return),
        _ => false,
    }
}

fn normalize_function_returns(func: &mut TypedFunction, counter: &mut usize) {
    if func.return_ty == Type::Void {
        return;
    }
    if has_single_return_at_end(&func.body) {
        return;
    }
    let ret_count = count_returns_block(&func.body);
    if ret_count == 0 {
        return;
    }

    *counter += 1;
    let ret_var = format!("__ret_val_{}_{}", func.name, counter);

    flatten_block_early_returns(&mut func.body, &ret_var, counter);
    replace_returns_with_assign(&mut func.body, &ret_var);

    let default_expr = default_expr_for_type(&func.return_ty, func.span);
    func.body.stmts.insert(
        0,
        TypedStmt::Let {
            name: ret_var.clone(),
            is_mutable: true,
            ty: func.return_ty.clone(),
            value: default_expr,
            span: func.span,
        },
    );
    func.body.stmts.push(TypedStmt::Return(
        Some(TypedExpr::Ident {
            name: ret_var,
            ty: func.return_ty.clone(),
            span: func.span,
        }),
        func.span,
    ));
}

fn count_returns_block(block: &TypedBlock) -> usize {
    let mut count = 0;
    for s in &block.stmts {
        count += count_returns_stmt(s);
    }
    count
}

fn count_returns_stmt(stmt: &TypedStmt) -> usize {
    match stmt {
        TypedStmt::Return(..) => 1,
        TypedStmt::If { then_branch, else_branch, .. } => {
            count_returns_block(then_branch) + else_branch.as_ref().map_or(0, count_returns_block)
        }
        TypedStmt::While { body, .. } => count_returns_block(body),
        _ => 0,
    }
}

fn block_terminates_with_return(block: &TypedBlock) -> bool {
    if let Some(last) = block.stmts.last() {
        match last {
            TypedStmt::Return(..) => true,
            TypedStmt::If { then_branch, else_branch: Some(eb), .. } => {
                block_terminates_with_return(then_branch) && block_terminates_with_return(eb)
            }
            _ => false,
        }
    } else {
        false
    }
}

fn default_expr_for_type(ty: &Type, span: Span) -> TypedExpr {
    match ty {
        Type::I64 => TypedExpr::Literal {
            lit: TypedLiteral::Int(0, Type::I64),
            ty: Type::I64,
            span,
        },
        Type::I32 => TypedExpr::Literal {
            lit: TypedLiteral::Int(0, Type::I32),
            ty: Type::I32,
            span,
        },
        Type::F64 => TypedExpr::Literal {
            lit: TypedLiteral::Float(0.0, Type::F64),
            ty: Type::F64,
            span,
        },
        Type::F32 => TypedExpr::Literal {
            lit: TypedLiteral::Float(0.0, Type::F32),
            ty: Type::F32,
            span,
        },
        Type::Bool => TypedExpr::Literal {
            lit: TypedLiteral::Bool(false),
            ty: Type::Bool,
            span,
        },
        _ => TypedExpr::Literal {
            lit: TypedLiteral::Int(0, Type::I64),
            ty: Type::I64,
            span,
        },
    }
}

fn flatten_block_early_returns(
    block: &mut TypedBlock,
    ret_var: &str,
    counter: &mut usize,
) {
    let mut i = 0;
    while i < block.stmts.len() {
        let should_drain_else = match &block.stmts[i] {
            TypedStmt::If { then_branch, else_branch: None, .. } => {
                block_terminates_with_return(then_branch) && i + 1 < block.stmts.len()
            }
            _ => false,
        };

        if should_drain_else {
            let remaining: Vec<TypedStmt> = block.stmts.drain((i + 1)..).collect();
            if let TypedStmt::If { then_branch, else_branch, span, .. } = &mut block.stmts[i] {
                flatten_block_early_returns(then_branch, ret_var, counter);
                let mut new_else = TypedBlock {
                    stmts: remaining,
                    span: *span,
                };
                flatten_block_early_returns(&mut new_else, ret_var, counter);
                *else_branch = Some(new_else);
            }
            break;
        }

        let should_truncate = match &block.stmts[i] {
            TypedStmt::If { then_branch, else_branch: Some(eb), .. } => {
                block_terminates_with_return(then_branch) && block_terminates_with_return(eb) && i + 1 < block.stmts.len()
            }
            _ => false,
        };

        if should_truncate {
            if let TypedStmt::If { then_branch, else_branch: Some(eb), .. } = &mut block.stmts[i] {
                flatten_block_early_returns(then_branch, ret_var, counter);
                flatten_block_early_returns(eb, ret_var, counter);
            }
            block.stmts.truncate(i + 1);
            break;
        }

        let should_rewrite_while = match &block.stmts[i] {
            TypedStmt::While { body, .. } => count_returns_block(body) > 0,
            _ => false,
        };

        if should_rewrite_while {
            *counter += 1;
            let has_ret_var = format!("__has_ret_{}", counter);
            let while_span = block.stmts[i].span();

            if let TypedStmt::While { body, .. } = &mut block.stmts[i] {
                rewrite_loop_returns(body, ret_var, &has_ret_var);
            }

            block.stmts.insert(
                i,
                TypedStmt::Let {
                    name: has_ret_var.clone(),
                    is_mutable: true,
                    ty: Type::Bool,
                    value: TypedExpr::Literal {
                        lit: TypedLiteral::Bool(false),
                        ty: Type::Bool,
                        span: while_span,
                    },
                    span: while_span,
                },
            );
            i += 1;

            if i + 1 < block.stmts.len() {
                let remaining: Vec<TypedStmt> = block.stmts.drain((i + 1)..).collect();
                let mut then_b = TypedBlock {
                    stmts: remaining,
                    span: while_span,
                };
                flatten_block_early_returns(&mut then_b, ret_var, counter);
                block.stmts.push(TypedStmt::If {
                    condition: TypedExpr::Unary {
                        op: UnaryOp::Not,
                        expr: Box::new(TypedExpr::Ident {
                            name: has_ret_var,
                            ty: Type::Bool,
                            span: while_span,
                        }),
                        ty: Type::Bool,
                        span: while_span,
                    },
                    then_branch: then_b,
                    else_branch: None,
                    span: while_span,
                });
                break;
            }
            i += 1;
            continue;
        }

        match &mut block.stmts[i] {
            TypedStmt::If { then_branch, else_branch, .. } => {
                flatten_block_early_returns(then_branch, ret_var, counter);
                if let Some(eb) = else_branch {
                    flatten_block_early_returns(eb, ret_var, counter);
                }
            }
            TypedStmt::While { body, .. } => {
                flatten_block_early_returns(body, ret_var, counter);
            }
            _ => {}
        }
        i += 1;
    }
}

fn rewrite_loop_returns(block: &mut TypedBlock, ret_var: &str, has_ret_var: &str) {
    let mut new_stmts = Vec::with_capacity(block.stmts.len() * 2);
    for stmt in block.stmts.drain(..) {
        match stmt {
            TypedStmt::Return(Some(ret_expr), span) => {
                new_stmts.push(TypedStmt::Assign {
                    name: ret_var.to_string(),
                    value: ret_expr,
                    span,
                });
                new_stmts.push(TypedStmt::Assign {
                    name: has_ret_var.to_string(),
                    value: TypedExpr::Literal {
                        lit: TypedLiteral::Bool(true),
                        ty: Type::Bool,
                        span,
                    },
                    span,
                });
                new_stmts.push(TypedStmt::Break(span));
            }
            TypedStmt::If { condition, mut then_branch, mut else_branch, span } => {
                rewrite_loop_returns(&mut then_branch, ret_var, has_ret_var);
                if let Some(eb) = &mut else_branch {
                    rewrite_loop_returns(eb, ret_var, has_ret_var);
                }
                new_stmts.push(TypedStmt::If { condition, then_branch, else_branch, span });
            }
            other => new_stmts.push(other),
        }
    }
    block.stmts = new_stmts;
}

fn replace_returns_with_assign(block: &mut TypedBlock, ret_var: &str) {
    for stmt in &mut block.stmts {
        match stmt {
            TypedStmt::Return(Some(expr), span) => {
                *stmt = TypedStmt::Assign {
                    name: ret_var.to_string(),
                    value: expr.clone(),
                    span: *span,
                };
            }
            TypedStmt::If { then_branch, else_branch, .. } => {
                replace_returns_with_assign(then_branch, ret_var);
                if let Some(eb) = else_branch {
                    replace_returns_with_assign(eb, ret_var);
                }
            }
            TypedStmt::While { body, .. } => {
                replace_returns_with_assign(body, ret_var);
            }
            _ => {}
        }
    }
}


fn inline_in_function(
    func: &mut TypedFunction,
    func_defs: &HashMap<String, TypedFunction>,
    recursive_funcs: &HashSet<String>,
    call_counter: &mut usize,
) -> bool {
    // Phase A: Call Lifting - extract calls inside expressions to preceding `let` bindings
    lift_calls_in_block(&mut func.body, func_defs, recursive_funcs, call_counter);

    // Phase B: Statement-level inlining
    inline_block(&mut func.body, func_defs, recursive_funcs, call_counter)
}

fn lift_calls_in_block(
    block: &mut TypedBlock,
    func_defs: &HashMap<String, TypedFunction>,
    recursive_funcs: &HashSet<String>,
    counter: &mut usize,
) {
    let mut new_stmts = Vec::with_capacity(block.stmts.len());

    for mut stmt in block.stmts.drain(..) {
        match &mut stmt {
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                lift_calls_expr(condition, &mut new_stmts, func_defs, recursive_funcs, counter);
                lift_calls_in_block(then_branch, func_defs, recursive_funcs, counter);
                if let Some(eb) = else_branch {
                    lift_calls_in_block(eb, func_defs, recursive_funcs, counter);
                }
                new_stmts.push(stmt);
            }
            TypedStmt::While { body, .. } => {
                // Notice: calls in while condition must re-evaluate every iteration;
                // do not lift out of while condition, but recurse into while body.
                lift_calls_in_block(body, func_defs, recursive_funcs, counter);
                new_stmts.push(stmt);
            }
            TypedStmt::Let { value, .. } => {
                // If value is a bare Call, we don't need to lift it; it's already a statement root.
                if let TypedExpr::Call { callee, .. } = value {
                    if let Some(target) = func_defs.get(callee) {
                        if is_inlinable(target, recursive_funcs) {
                            new_stmts.push(stmt);
                            continue;
                        }
                    }
                }
                lift_calls_expr(value, &mut new_stmts, func_defs, recursive_funcs, counter);
                new_stmts.push(stmt);
            }
            TypedStmt::Assign { value, .. } => {
                if let TypedExpr::Call { callee, .. } = value {
                    if let Some(target) = func_defs.get(callee) {
                        if is_inlinable(target, recursive_funcs) {
                            new_stmts.push(stmt);
                            continue;
                        }
                    }
                }
                lift_calls_expr(value, &mut new_stmts, func_defs, recursive_funcs, counter);
                new_stmts.push(stmt);
            }
            TypedStmt::Return(Some(expr), _) => {
                if let TypedExpr::Call { callee, .. } = expr {
                    if let Some(target) = func_defs.get(callee) {
                        if is_inlinable(target, recursive_funcs) {
                            new_stmts.push(stmt);
                            continue;
                        }
                    }
                }
                lift_calls_expr(expr, &mut new_stmts, func_defs, recursive_funcs, counter);
                new_stmts.push(stmt);
            }
            TypedStmt::Expr(expr) => {
                lift_calls_expr(expr, &mut new_stmts, func_defs, recursive_funcs, counter);
                new_stmts.push(stmt);
            }
            TypedStmt::IndexAssign { index, value, .. } => {
                lift_calls_expr(index, &mut new_stmts, func_defs, recursive_funcs, counter);
                lift_calls_expr(value, &mut new_stmts, func_defs, recursive_funcs, counter);
                new_stmts.push(stmt);
            }
            _ => {
                new_stmts.push(stmt);
            }
        }
    }

    block.stmts = new_stmts;
}

fn lift_calls_expr(
    expr: &mut TypedExpr,
    pre_stmts: &mut Vec<TypedStmt>,
    func_defs: &HashMap<String, TypedFunction>,
    recursive_funcs: &HashSet<String>,
    counter: &mut usize,
) {
    match expr {
        TypedExpr::Call { callee, args, ty, span } => {
            // First lift inside arguments
            for arg in args.iter_mut() {
                lift_calls_expr(arg, pre_stmts, func_defs, recursive_funcs, counter);
            }
            // If this call is inlinable, extract it
            if let Some(target) = func_defs.get(callee) {
                if is_inlinable(target, recursive_funcs) {
                    *counter += 1;
                    let tmp_name = format!("__inl_lifted_{}_{}", callee, counter);
                    let tmp_expr = TypedExpr::Call {
                        callee: callee.clone(),
                        args: args.clone(),
                        ty: ty.clone(),
                        span: *span,
                    };
                    pre_stmts.push(TypedStmt::Let {
                        name: tmp_name.clone(),
                        is_mutable: false,
                        ty: ty.clone(),
                        value: tmp_expr,
                        span: *span,
                    });
                    *expr = TypedExpr::Ident {
                        name: tmp_name,
                        ty: ty.clone(),
                        span: *span,
                    };
                }
            }
        }
        TypedExpr::Unary { expr, .. } => {
            lift_calls_expr(expr, pre_stmts, func_defs, recursive_funcs, counter);
        }
        TypedExpr::Binary { left, right, .. } => {
            lift_calls_expr(left, pre_stmts, func_defs, recursive_funcs, counter);
            lift_calls_expr(right, pre_stmts, func_defs, recursive_funcs, counter);
        }
        TypedExpr::ArrayLiteral { elements, .. } => {
            for el in elements {
                lift_calls_expr(el, pre_stmts, func_defs, recursive_funcs, counter);
            }
        }
        TypedExpr::Index { target, index, .. } => {
            lift_calls_expr(target, pre_stmts, func_defs, recursive_funcs, counter);
            lift_calls_expr(index, pre_stmts, func_defs, recursive_funcs, counter);
        }
        _ => {}
    }
}

fn inline_block(
    block: &mut TypedBlock,
    func_defs: &HashMap<String, TypedFunction>,
    recursive_funcs: &HashSet<String>,
    call_counter: &mut usize,
) -> bool {
    let mut changed = false;
    let mut new_stmts = Vec::with_capacity(block.stmts.len());

    for mut stmt in block.stmts.drain(..) {
        match &mut stmt {
            TypedStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if inline_block(then_branch, func_defs, recursive_funcs, call_counter) {
                    changed = true;
                }
                if let Some(eb) = else_branch {
                    if inline_block(eb, func_defs, recursive_funcs, call_counter) {
                        changed = true;
                    }
                }
                new_stmts.push(stmt);
            }
            TypedStmt::While { body, .. } => {
                if inline_block(body, func_defs, recursive_funcs, call_counter) {
                    changed = true;
                }
                new_stmts.push(stmt);
            }
            TypedStmt::Let {
                name,
                is_mutable,
                ty,
                value: TypedExpr::Call { callee, args, .. },
                span,
            } => {
                if let Some(target) = func_defs.get(callee) {
                    if is_inlinable(target, recursive_funcs) {
                        *call_counter += 1;
                        let cid = *call_counter;
                        expand_inlined_call(
                            target,
                            args,
                            TargetVar::Let {
                                name: name.clone(),
                                is_mutable: *is_mutable,
                                ty: ty.clone(),
                                span: *span,
                            },
                            cid,
                            &mut new_stmts,
                        );
                        changed = true;
                        continue;
                    }
                }
                new_stmts.push(stmt);
            }
            TypedStmt::Assign {
                name,
                value: TypedExpr::Call { callee, args, .. },
                span,
            } => {
                if let Some(target) = func_defs.get(callee) {
                    if is_inlinable(target, recursive_funcs) {
                        *call_counter += 1;
                        let cid = *call_counter;
                        expand_inlined_call(
                            target,
                            args,
                            TargetVar::Assign {
                                name: name.clone(),
                                span: *span,
                            },
                            cid,
                            &mut new_stmts,
                        );
                        changed = true;
                        continue;
                    }
                }
                new_stmts.push(stmt);
            }
            TypedStmt::Return(Some(TypedExpr::Call { callee, args, .. }), span) => {
                if let Some(target) = func_defs.get(callee) {
                    if is_inlinable(target, recursive_funcs) {
                        *call_counter += 1;
                        let cid = *call_counter;
                        expand_inlined_call(
                            target,
                            args,
                            TargetVar::Return { span: *span },
                            cid,
                            &mut new_stmts,
                        );
                        changed = true;
                        continue;
                    }
                }
                new_stmts.push(stmt);
            }
            _ => {
                new_stmts.push(stmt);
            }
        }
    }

    block.stmts = new_stmts;
    changed
}

enum TargetVar {
    Let {
        name: String,
        is_mutable: bool,
        ty: Type,
        span: Span,
    },
    Assign {
        name: String,
        span: Span,
    },
    Return {
        span: Span,
    },
}

fn expand_inlined_call(
    callee: &TypedFunction,
    args: &[TypedExpr],
    target: TargetVar,
    call_id: usize,
    out_stmts: &mut Vec<TypedStmt>,
) {
    // 1. Build variable rename mapping for all parameters and locals in callee
    let mut rename_map: HashMap<String, String> = HashMap::new();
    for p in &callee.params {
        rename_map.insert(p.name.clone(), format!("__inl_{}_{}_{}", callee.name, call_id, p.name));
    }
    collect_local_names(&callee.body, &mut rename_map, &callee.name, call_id);

    // 2. Emit parameter bindings: let mut renamed_p = arg;
    for (i, p) in callee.params.iter().enumerate() {
        let renamed_name = rename_map.get(&p.name).unwrap().clone();
        let arg_expr = if i < args.len() {
            args[i].clone()
        } else {
            TypedExpr::Ident {
                name: p.name.clone(),
                ty: p.ty.clone(),
                span: p.span,
            }
        };
        out_stmts.push(TypedStmt::Let {
            name: renamed_name,
            is_mutable: true,
            ty: p.ty.clone(),
            value: arg_expr,
            span: p.span,
        });
    }

    // 3. Clone and rename callee statements, transforming the final Return into target assignment
    let total_stmts = callee.body.stmts.len();
    for (idx, stmt) in callee.body.stmts.iter().enumerate() {
        if idx == total_stmts - 1 {
            // Final return statement
            if let TypedStmt::Return(Some(ret_val), _span) = stmt {
                let renamed_val = rename_expr(ret_val, &rename_map);
                match target {
                    TargetVar::Let {
                        name,
                        is_mutable,
                        ty,
                        span,
                    } => {
                        out_stmts.push(TypedStmt::Let {
                            name,
                            is_mutable,
                            ty,
                            value: renamed_val,
                            span,
                        });
                    }
                    TargetVar::Assign { name, span } => {
                        out_stmts.push(TypedStmt::Assign {
                            name,
                            value: renamed_val,
                            span,
                        });
                    }
                    TargetVar::Return { span } => {
                        out_stmts.push(TypedStmt::Return(Some(renamed_val), span));
                    }
                }
                return;
            }
        }

        let mut cloned = stmt.clone();
        rename_stmt(&mut cloned, &rename_map);
        out_stmts.push(cloned);
    }
}

fn collect_local_names(
    block: &TypedBlock,
    map: &mut HashMap<String, String>,
    callee_name: &str,
    call_id: usize,
) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { name, .. } => {
                if !map.contains_key(name) {
                    map.insert(name.clone(), format!("__inl_{}_{}_{}", callee_name, call_id, name));
                }
            }
            TypedStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_local_names(then_branch, map, callee_name, call_id);
                if let Some(eb) = else_branch {
                    collect_local_names(eb, map, callee_name, call_id);
                }
            }
            TypedStmt::While { body, .. } => {
                collect_local_names(body, map, callee_name, call_id);
            }
            _ => {}
        }
    }
}

fn rename_stmt(stmt: &mut TypedStmt, map: &HashMap<String, String>) {
    match stmt {
        TypedStmt::Let { name, value, .. } => {
            if let Some(new_name) = map.get(name) {
                *name = new_name.clone();
            }
            *value = rename_expr(value, map);
        }
        TypedStmt::Assign { name, value, .. } => {
            if let Some(new_name) = map.get(name) {
                *name = new_name.clone();
            }
            *value = rename_expr(value, map);
        }
        TypedStmt::IndexAssign {
            target,
            index,
            value,
            ..
        } => {
            if let Some(new_name) = map.get(target) {
                *target = new_name.clone();
            }
            *index = rename_expr(index, map);
            *value = rename_expr(value, map);
        }
        TypedStmt::Return(Some(val), _) => {
            *val = rename_expr(val, map);
        }
        TypedStmt::Expr(expr) => {
            *expr = rename_expr(expr, map);
        }
        TypedStmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            *condition = rename_expr(condition, map);
            rename_block(then_branch, map);
            if let Some(eb) = else_branch {
                rename_block(eb, map);
            }
        }
        TypedStmt::While { condition, body, .. } => {
            *condition = rename_expr(condition, map);
            rename_block(body, map);
        }
        _ => {}
    }
}

fn rename_block(block: &mut TypedBlock, map: &HashMap<String, String>) {
    for stmt in &mut block.stmts {
        rename_stmt(stmt, map);
    }
}

fn rename_expr(expr: &TypedExpr, map: &HashMap<String, String>) -> TypedExpr {
    match expr {
        TypedExpr::Ident { name, ty, span } => {
            let new_name = map.get(name).cloned().unwrap_or_else(|| name.clone());
            TypedExpr::Ident {
                name: new_name,
                ty: ty.clone(),
                span: *span,
            }
        }
        TypedExpr::Unary { op, expr, ty, span } => TypedExpr::Unary {
            op: *op,
            expr: Box::new(rename_expr(expr, map)),
            ty: ty.clone(),
            span: *span,
        },
        TypedExpr::Binary {
            op,
            left,
            right,
            ty,
            span,
        } => TypedExpr::Binary {
            op: *op,
            left: Box::new(rename_expr(left, map)),
            right: Box::new(rename_expr(right, map)),
            ty: ty.clone(),
            span: *span,
        },
        TypedExpr::Call {
            callee,
            args,
            ty,
            span,
        } => TypedExpr::Call {
            callee: callee.clone(),
            args: args.iter().map(|a| rename_expr(a, map)).collect(),
            ty: ty.clone(),
            span: *span,
        },
        TypedExpr::ArrayLiteral { elements, ty, span } => TypedExpr::ArrayLiteral {
            elements: elements.iter().map(|e| rename_expr(e, map)).collect(),
            ty: ty.clone(),
            span: *span,
        },
        TypedExpr::Index {
            target,
            index,
            is_safe,
            ty,
            span,
        } => TypedExpr::Index {
            target: Box::new(rename_expr(target, map)),
            index: Box::new(rename_expr(index, map)),
            is_safe: *is_safe,
            ty: ty.clone(),
            span: *span,
        },
        _ => expr.clone(),
    }
}
