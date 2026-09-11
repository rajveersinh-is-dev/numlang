//! Whole-program propagation of literal function arguments.
//!
//! NumLang programs are compiled as a closed unit: there is no dynamic linking
//! or reflective function invocation.  When every call to a non-recursive
//! function passes the same literal for a parameter, replacing reads of that
//! parameter with the literal is semantics-preserving.  Besides eliminating
//! needless SSA values, this exposes constant divisors and bounds to the
//! backend's strength-reduction passes.

use std::collections::{HashMap, HashSet};

use crate::typecheck::typed_ast::{TypedBlock, TypedExpr, TypedFunction, TypedProgram, TypedStmt};

type LiteralArgs = Vec<TypedExpr>;

pub fn optimize_program(program: &mut TypedProgram) {
    // Materialize immutable local literals first.  This lets calls such as
    // `pow_mod(i, 13, modulus)` participate when `modulus` was declared as a
    // local constant, without requiring an interprocedural data-flow engine.
    for function in &mut program.functions {
        propagate_local_literals(function);
    }

    let function_names: HashSet<String> =
        program.functions.iter().map(|f| f.name.clone()).collect();
    let mut calls: HashMap<String, Vec<LiteralArgs>> = HashMap::new();

    for function in &program.functions {
        collect_calls_in_block(&function.body, &function_names, &mut calls);
    }

    for function in &mut program.functions {
        // Do not rewrite a recursive function: its recursive call deliberately
        // receives a changing argument even when an outer call is literal.
        if calls_function(&function.body, &function.name) || params_are_mutated(function) {
            continue;
        }

        let Some(call_args) = calls.get(&function.name) else {
            continue;
        };
        if call_args.is_empty() {
            continue;
        }

        let replacements: HashMap<String, TypedExpr> = function
            .params
            .iter()
            .enumerate()
            .filter_map(|(index, param)| {
                let first = call_args.first()?.get(index)?;
                if !matches!(first, TypedExpr::Literal { .. })
                    || call_args.iter().any(|args| args.get(index) != Some(first))
                {
                    return None;
                }
                Some((param.name.clone(), first.clone()))
            })
            .collect();

        if !replacements.is_empty() {
            replace_in_block(&mut function.body, &replacements);
        }
    }
}

fn propagate_local_literals(function: &mut TypedFunction) {
    let mut bindings = HashMap::new();
    collect_local_literal_bindings(&function.body, &mut bindings);
    if !bindings.is_empty() {
        replace_in_block(&mut function.body, &bindings);
    }
}

fn collect_local_literal_bindings(block: &TypedBlock, bindings: &mut HashMap<String, TypedExpr>) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let {
                name,
                value: TypedExpr::Literal { .. },
                ..
            } if !block_mutates_name(block, name) => {
                // A binding may only be read after its declaration because the
                // type checker rejects use-before-definition.  It is therefore
                // safe to substitute throughout this closed function body.
                if !bindings.contains_key(name) {
                    if let TypedStmt::Let { value, .. } = stmt {
                        bindings.insert(name.clone(), value.clone());
                    }
                }
            }
            // Only function-scope declarations are collected.  Keeping block
            // locals out of this pass avoids accidentally crossing a future
            // lexical-shadowing boundary.
            TypedStmt::If { .. } | TypedStmt::While { .. } => {}
            _ => {}
        }
    }
}

fn block_mutates_name(block: &TypedBlock, name: &str) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        TypedStmt::Assign { name: assigned, .. }
        | TypedStmt::IndexAssign {
            target: assigned, ..
        } => assigned == name,
        TypedStmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            block_mutates_name(then_branch, name)
                || else_branch
                    .as_ref()
                    .is_some_and(|branch| block_mutates_name(branch, name))
        }
        TypedStmt::While { body, .. } => block_mutates_name(body, name),
        _ => false,
    })
}

fn collect_calls_in_block(
    block: &TypedBlock,
    function_names: &HashSet<String>,
    calls: &mut HashMap<String, Vec<LiteralArgs>>,
) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { value, .. }
            | TypedStmt::Assign { value, .. }
            | TypedStmt::Expr(value) => {
                collect_calls_in_expr(value, function_names, calls);
            }
            TypedStmt::IndexAssign { index, value, .. } => {
                collect_calls_in_expr(index, function_names, calls);
                collect_calls_in_expr(value, function_names, calls);
            }
            TypedStmt::Return(Some(value), _) => {
                collect_calls_in_expr(value, function_names, calls)
            }
            TypedStmt::Return(None, _) | TypedStmt::Break(_) => {}
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                collect_calls_in_expr(condition, function_names, calls);
                collect_calls_in_block(then_branch, function_names, calls);
                if let Some(else_branch) = else_branch {
                    collect_calls_in_block(else_branch, function_names, calls);
                }
            }
            TypedStmt::While {
                condition, body, ..
            } => {
                collect_calls_in_expr(condition, function_names, calls);
                collect_calls_in_block(body, function_names, calls);
            }
        }
    }
}

fn collect_calls_in_expr(
    expr: &TypedExpr,
    function_names: &HashSet<String>,
    calls: &mut HashMap<String, Vec<LiteralArgs>>,
) {
    match expr {
        TypedExpr::Unary { expr, .. } => collect_calls_in_expr(expr, function_names, calls),
        TypedExpr::Binary { left, right, .. } => {
            collect_calls_in_expr(left, function_names, calls);
            collect_calls_in_expr(right, function_names, calls);
        }
        TypedExpr::Call { callee, args, .. } => {
            for arg in args {
                collect_calls_in_expr(arg, function_names, calls);
            }
            if function_names.contains(callee) {
                calls.entry(callee.clone()).or_default().push(args.clone());
            }
        }
        TypedExpr::ArrayLiteral { elements, .. } => {
            for element in elements {
                collect_calls_in_expr(element, function_names, calls);
            }
        }
        TypedExpr::Index { target, index, .. } => {
            collect_calls_in_expr(target, function_names, calls);
            collect_calls_in_expr(index, function_names, calls);
        }
        TypedExpr::Literal { .. } | TypedExpr::Ident { .. } => {}
    }
}

fn calls_function(block: &TypedBlock, name: &str) -> bool {
    let mut calls = HashMap::new();
    let names = HashSet::from([name.to_string()]);
    collect_calls_in_block(block, &names, &mut calls);
    calls.contains_key(name)
}

fn params_are_mutated(function: &TypedFunction) -> bool {
    let params: HashSet<&str> = function
        .params
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    block_mutates_any(&function.body, &params)
}

fn block_mutates_any(block: &TypedBlock, names: &HashSet<&str>) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        TypedStmt::Assign { name, .. } | TypedStmt::IndexAssign { target: name, .. } => {
            names.contains(name.as_str())
        }
        TypedStmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            block_mutates_any(then_branch, names)
                || else_branch
                    .as_ref()
                    .is_some_and(|branch| block_mutates_any(branch, names))
        }
        TypedStmt::While { body, .. } => block_mutates_any(body, names),
        _ => false,
    })
}

fn replace_in_block(block: &mut TypedBlock, replacements: &HashMap<String, TypedExpr>) {
    for stmt in &mut block.stmts {
        match stmt {
            TypedStmt::Let { value, .. }
            | TypedStmt::Assign { value, .. }
            | TypedStmt::Expr(value) => {
                replace_in_expr(value, replacements);
            }
            TypedStmt::IndexAssign { index, value, .. } => {
                replace_in_expr(index, replacements);
                replace_in_expr(value, replacements);
            }
            TypedStmt::Return(Some(value), _) => replace_in_expr(value, replacements),
            TypedStmt::Return(None, _) | TypedStmt::Break(_) => {}
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                replace_in_expr(condition, replacements);
                replace_in_block(then_branch, replacements);
                if let Some(else_branch) = else_branch {
                    replace_in_block(else_branch, replacements);
                }
            }
            TypedStmt::While {
                condition, body, ..
            } => {
                replace_in_expr(condition, replacements);
                replace_in_block(body, replacements);
            }
        }
    }
}

fn replace_in_expr(expr: &mut TypedExpr, replacements: &HashMap<String, TypedExpr>) {
    match expr {
        TypedExpr::Ident { name, .. } => {
            if let Some(replacement) = replacements.get(name) {
                *expr = replacement.clone();
            }
        }
        TypedExpr::Unary { expr, .. } => replace_in_expr(expr, replacements),
        TypedExpr::Binary { left, right, .. } => {
            replace_in_expr(left, replacements);
            replace_in_expr(right, replacements);
        }
        TypedExpr::Call { args, .. } => {
            for arg in args {
                replace_in_expr(arg, replacements);
            }
        }
        TypedExpr::ArrayLiteral { elements, .. } => {
            for element in elements {
                replace_in_expr(element, replacements);
            }
        }
        TypedExpr::Index { target, index, .. } => {
            replace_in_expr(target, replacements);
            replace_in_expr(index, replacements);
        }
        TypedExpr::Literal { .. } => {}
    }
}
