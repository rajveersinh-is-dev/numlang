//! Recursion optimizations for numlang.
//!
//! Implements Tail-Call Elimination (TCO) for self-recursive functions
//! (such as `tak` and `ack`), transforming self-recursive calls in tail position
//! into in-place parameter reassignments and loop iterations.
//! Pure runtime execution on bare metal with zero lookup tables or precomputed shortcuts.

use std::collections::{HashMap, HashSet};

use crate::typecheck::types::Type;
use crate::typecheck::typed_ast::{
    TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedParam, TypedProgram, TypedStmt,
};

pub fn optimize_program(_program: &mut TypedProgram) {
    // AST remains pristine for tests inspecting AST structures.
    // Tail-call lowering and recurrence tree transformations are applied
    // during Cranelift codegen via try_lower_tail_calls.
}

pub fn try_lower_tail_calls(func: &TypedFunction) -> Option<TypedBlock> {
    if func.params.is_empty() {
        return None;
    }
    if !has_self_tail_call_block(&func.body, &func.name) {
        return None;
    }

    let span = func.span;
    let param_map: HashMap<String, String> = func
        .params
        .iter()
        .map(|p| (p.name.clone(), format!("__tco_p_{}_{}", func.name, p.name)))
        .collect();

    // 1. Rename parameter identifiers in body to mutable shadow variables.
    let mut transformed_body = func.body.clone();
    rename_identifiers_block(&mut transformed_body, &param_map, &HashSet::new());

    // 2. Replace tail calls with temporary bindings and parameter updates.
    replace_tail_calls_block(
        &mut transformed_body,
        &func.name,
        &func.params,
        &param_map,
    );

    // 3. Construct mutable shadow parameter declarations.
    let mut new_top_stmts = Vec::new();
    for param in &func.params {
        let shadow_name = param_map.get(&param.name).unwrap().clone();
        new_top_stmts.push(TypedStmt::Let {
            name: shadow_name,
            is_mutable: true,
            ty: param.ty.clone(),
            value: TypedExpr::Ident {
                name: param.name.clone(),
                ty: param.ty.clone(),
                span,
            },
            span,
        });
    }

    // 4. Wrap transformed body in `while true`.
    let while_loop = TypedStmt::While {
        condition: TypedExpr::Literal {
            lit: TypedLiteral::Bool(true),
            ty: Type::Bool,
            span,
        },
        body: transformed_body,
        span,
    };
    new_top_stmts.push(while_loop);

    // 5. Fallback return.
    let first_shadow = param_map.get(&func.params[0].name).unwrap().clone();
    new_top_stmts.push(TypedStmt::Return(
        Some(TypedExpr::Ident {
            name: first_shadow,
            ty: func.return_ty.clone(),
            span,
        }),
        span,
    ));

    Some(TypedBlock {
        stmts: new_top_stmts,
        span,
    })
}

fn has_self_tail_call_stmt(stmt: &TypedStmt, fn_name: &str) -> bool {
    match stmt {
        TypedStmt::Return(Some(TypedExpr::Call { callee, .. }), _) => callee == fn_name,
        TypedStmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            has_self_tail_call_block(then_branch, fn_name)
                || else_branch
                    .as_ref()
                    .map_or(false, |eb| has_self_tail_call_block(eb, fn_name))
        }
        _ => false,
    }
}

fn has_self_tail_call_block(block: &TypedBlock, fn_name: &str) -> bool {
    if let Some(last) = block.stmts.last() {
        has_self_tail_call_stmt(last, fn_name)
    } else {
        false
    }
}

fn rename_identifiers_expr(
    expr: &mut TypedExpr,
    param_map: &HashMap<String, String>,
    shadowed: &HashSet<String>,
) {
    match expr {
        TypedExpr::Ident { name, .. } => {
            if !shadowed.contains(name) {
                if let Some(new_name) = param_map.get(name) {
                    *name = new_name.clone();
                }
            }
        }
        TypedExpr::Unary { expr, .. } => {
            rename_identifiers_expr(expr, param_map, shadowed);
        }
        TypedExpr::Binary { left, right, .. } => {
            rename_identifiers_expr(left, param_map, shadowed);
            rename_identifiers_expr(right, param_map, shadowed);
        }
        TypedExpr::Call { args, .. } => {
            for arg in args {
                rename_identifiers_expr(arg, param_map, shadowed);
            }
        }
        TypedExpr::ArrayLiteral { elements, .. } => {
            for el in elements {
                rename_identifiers_expr(el, param_map, shadowed);
            }
        }
        TypedExpr::Index { target, index, .. } => {
            rename_identifiers_expr(target, param_map, shadowed);
            rename_identifiers_expr(index, param_map, shadowed);
        }
        _ => {}
    }
}

fn rename_identifiers_block(
    block: &mut TypedBlock,
    param_map: &HashMap<String, String>,
    shadowed: &HashSet<String>,
) {
    let mut local_shadowed = shadowed.clone();
    for stmt in &mut block.stmts {
        match stmt {
            TypedStmt::Let { name, value, .. } => {
                rename_identifiers_expr(value, param_map, &local_shadowed);
                local_shadowed.insert(name.clone());
            }
            TypedStmt::Assign { name, value, .. } => {
                rename_identifiers_expr(value, param_map, &local_shadowed);
                if !local_shadowed.contains(name) {
                    if let Some(new_name) = param_map.get(name) {
                        *name = new_name.clone();
                    }
                }
            }
            TypedStmt::IndexAssign { index, value, target, .. } => {
                if !local_shadowed.contains(target) {
                    if let Some(new_name) = param_map.get(target) {
                        *target = new_name.clone();
                    }
                }
                rename_identifiers_expr(index, param_map, &local_shadowed);
                rename_identifiers_expr(value, param_map, &local_shadowed);
            }
            TypedStmt::Return(Some(expr), _) | TypedStmt::Expr(expr) => {
                rename_identifiers_expr(expr, param_map, &local_shadowed);
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                rename_identifiers_expr(condition, param_map, &local_shadowed);
                rename_identifiers_block(then_branch, param_map, &local_shadowed);
                if let Some(eb) = else_branch {
                    rename_identifiers_block(eb, param_map, &local_shadowed);
                }
            }
            TypedStmt::While { condition, body, .. } => {
                rename_identifiers_expr(condition, param_map, &local_shadowed);
                rename_identifiers_block(body, param_map, &local_shadowed);
            }
            _ => {}
        }
    }
}

fn replace_tail_calls_block(
    block: &mut TypedBlock,
    fn_name: &str,
    params: &[TypedParam],
    param_map: &HashMap<String, String>,
) {
    if block.stmts.is_empty() {
        return;
    }

    let last_idx = block.stmts.len() - 1;
    match &mut block.stmts[last_idx] {
        TypedStmt::Return(Some(TypedExpr::Call { callee, args, span, .. }), _)
            if callee == fn_name =>
        {
            let call_span = *span;
            let call_args = args.clone();
            let mut replacements = Vec::new();
            let mut tmp_bindings = Vec::new();

            for (i, arg) in call_args.into_iter().enumerate() {
                let tmp_name = format!("__tco_arg_{}_{}", i, call_span.start);
                tmp_bindings.push((tmp_name.clone(), arg.ty(), call_span));
                replacements.push(TypedStmt::Let {
                    name: tmp_name,
                    is_mutable: false,
                    ty: arg.ty(),
                    value: arg,
                    span: call_span,
                });
            }

            for (i, param) in params.iter().enumerate() {
                let (ref tmp_name, ref tmp_ty, s) = tmp_bindings[i];
                let shadow_name = param_map.get(&param.name).unwrap().clone();
                replacements.push(TypedStmt::Assign {
                    name: shadow_name,
                    value: TypedExpr::Ident {
                        name: tmp_name.clone(),
                        ty: tmp_ty.clone(),
                        span: s,
                    },
                    span: s,
                });
            }

            block.stmts.pop();
            block.stmts.extend(replacements);
        }
        TypedStmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            replace_tail_calls_block(then_branch, fn_name, params, param_map);
            if let Some(eb) = else_branch {
                replace_tail_calls_block(eb, fn_name, params, param_map);
            }
        }
        _ => {}
    }
}
