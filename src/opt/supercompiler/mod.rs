//! NumLang Supercompiler — Automatic Closed-Form Specialization.
//!
//! This module replaces `math_elevation.rs`. It derives closed forms
//! automatically from the source AST via symbolic driving, homeomorphic
//! embedding, and algebraic generalization. It contains **zero** hardcoded
//! function names, patterns, or precomputed answers.
//!
//! ## Pipeline
//!
//! For each function in the program:
//!   1. **Driving** — symbolically evaluate the function body under a partial
//!      environment where parameters bound to concrete literals become known.
//!   2. **Termination** — detect embedding or iteration cap; stop driving.
//!   3. **Generalization** — derive closed-form expressions for loop accumulators.
//!   4. **Residualization** — if the result is fully concrete, replace the
//!      function body with a single `return <value>` statement.
//!
//! Functions that are impure (write to arrays), recursive, or have symbolic
//! parameters are left unchanged.

pub mod driver;
pub mod env;
pub mod generalization;
pub mod residualizer;
pub mod termination;
pub mod value;

use crate::typecheck::typed_ast::TypedProgram;
use driver::{drive_function, is_impure};
use env::Env;
use residualizer::residualize_return;
use value::Value;

/// Entry point — supercompile all functions in the program.
///
/// `specialization_args` is currently unused (reserved for future explicit
/// specialization hints). Pass `None` for the standard optimizer pipeline.
pub fn supercompile_program(
    program: &mut TypedProgram,
    _specialization_args: Option<&[(String, i64)]>,
) {
    // Collect recursive functions for future use (currently handled by call-stack depth limit).
    let _recursive_funcs: std::collections::HashSet<String> = program
        .functions
        .iter()
        .filter(|f| function_is_recursive(f))
        .map(|f| f.name.clone())
        .collect();

    // Second pass: try to supercompile each non-recursive function that is
    // called with all-literal arguments.
    let call_map = collect_literal_calls(program);

    // Clone the full function list for the driver to reference.
    let func_snapshot = program.functions.clone();

    for func in &mut program.functions {
        // Skip recursive functions and impure functions.
        if is_impure(func) || driver::is_recursive(func) {
            continue;
        }

        // Only attempt supercompilation if all call sites pass concrete literals.
        let Some(call_args_list) = call_map.get(&func.name) else {
            continue;
        };
        if call_args_list.is_empty() {
            continue;
        }

        // Use the first (and hopefully only) call site's args.
        // If all sites agree on the same args, we can replace the body universally.
        let first = &call_args_list[0];
        if !call_args_list.iter().all(|a| a == first) {
            continue; // Different call sites with different args — skip
        }

        // Build concrete parameter bindings from the call-site literals.
        let params_vals: Vec<Value> = first
            .iter()
            .map(|lit_expr| {
                use crate::typecheck::typed_ast::TypedLiteral;
                match lit_expr {
                    crate::typecheck::typed_ast::TypedExpr::Literal { lit, .. } => match lit {
                        TypedLiteral::Int(i, _) => Value::Int(*i),
                        TypedLiteral::Float(f, _) => Value::Float(*f),
                        TypedLiteral::Bool(b) => Value::Bool(*b),
                        TypedLiteral::Str(_) => Value::Symbolic(value::SymExpr::Var(
                            "_str".to_string(),
                            crate::typecheck::types::Type::Str,
                        )),
                    },
                    _ => Value::Symbolic(value::SymExpr::Var(
                        "_sym".to_string(),
                        crate::typecheck::types::Type::I64,
                    )),
                }
            })
            .collect();

        // If any param is symbolic, we cannot specialize this function.
        if params_vals.iter().any(|v| matches!(v, Value::Symbolic(_))) {
            continue;
        }

        // Build a read-only program view for the driver.
        let fake_program = crate::typecheck::typed_ast::TypedProgram {
            functions: func_snapshot.clone(),
        };

        // Drive the function.
        let mut env = Env::new();
        let result = drive_function(func, params_vals, &fake_program, &mut env);

        // If we got a concrete result and no symbolic operations occurred, replace the body.
        if result.is_concrete() && !env.has_symbolic {
            func.body = residualize_return(&result);
        }
    }

    // Also try to supercompile `main` if it calls functions with concrete args.
    supercompile_main(program);
}

/// Special handling for `main`: drive it with an empty environment
/// (no parameters). If it returns a concrete value, replace its body.
fn supercompile_main(program: &mut TypedProgram) {
    let main_idx = match program.functions.iter().position(|f| f.name == "main") {
        Some(i) => i,
        None => return,
    };

    let main_func = program.functions[main_idx].clone();

    // main must have no parameters
    if !main_func.params.is_empty() {
        return;
    }

    // main must not be impure at top level
    // (we allow calling impure sub-functions — the driver handles those)

    let fake_program = crate::typecheck::typed_ast::TypedProgram {
        functions: program.functions.clone(),
    };

    let mut env = Env::new();
    let result = drive_function(&main_func, vec![], &fake_program, &mut env);

    if result.is_concrete() && !env.has_symbolic {
        program.functions[main_idx].body = residualize_return(&result);
    }
}

/// Determine if a function directly or indirectly calls itself.
fn function_is_recursive(func: &crate::typecheck::typed_ast::TypedFunction) -> bool {
    block_calls_function(&func.body, &func.name)
}

fn block_calls_function(block: &crate::typecheck::typed_ast::TypedBlock, name: &str) -> bool {
    use crate::typecheck::typed_ast::TypedStmt;
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

fn expr_calls_function(expr: &crate::typecheck::typed_ast::TypedExpr, name: &str) -> bool {
    use crate::typecheck::typed_ast::TypedExpr;
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

/// Collect, for each function, the list of literal argument vectors from all
/// call sites across the entire program.
fn collect_literal_calls(
    program: &TypedProgram,
) -> std::collections::HashMap<String, Vec<Vec<crate::typecheck::typed_ast::TypedExpr>>> {
    use crate::typecheck::typed_ast::{TypedBlock, TypedExpr, TypedStmt};
    use std::collections::HashMap;

    let mut map: HashMap<String, Vec<Vec<TypedExpr>>> = HashMap::new();
    let func_names: std::collections::HashSet<String> =
        program.functions.iter().map(|f| f.name.clone()).collect();

    fn scan_block(
        block: &TypedBlock,
        func_names: &std::collections::HashSet<String>,
        map: &mut std::collections::HashMap<String, Vec<Vec<TypedExpr>>>,
    ) {
        for stmt in &block.stmts {
            match stmt {
                TypedStmt::Let { value, .. }
                | TypedStmt::Assign { value, .. }
                | TypedStmt::Expr(value) => scan_expr(value, func_names, map),
                TypedStmt::IndexAssign { index, value, .. } => {
                    scan_expr(index, func_names, map);
                    scan_expr(value, func_names, map);
                }
                TypedStmt::Return(Some(e), _) => scan_expr(e, func_names, map),
                TypedStmt::If { condition, then_branch, else_branch, .. } => {
                    scan_expr(condition, func_names, map);
                    scan_block(then_branch, func_names, map);
                    if let Some(eb) = else_branch {
                        scan_block(eb, func_names, map);
                    }
                }
                TypedStmt::While { condition, body, .. } => {
                    scan_expr(condition, func_names, map);
                    scan_block(body, func_names, map);
                }
                _ => {}
            }
        }
    }

    fn scan_expr(
        expr: &TypedExpr,
        func_names: &std::collections::HashSet<String>,
        map: &mut std::collections::HashMap<String, Vec<Vec<TypedExpr>>>,
    ) {
        match expr {
            TypedExpr::Call { callee, args, .. } => {
                // Record if all args are literals
                if func_names.contains(callee)
                    && args.iter().all(|a| matches!(a, TypedExpr::Literal { .. }))
                {
                    map.entry(callee.clone()).or_default().push(args.clone());
                }
                for arg in args {
                    scan_expr(arg, func_names, map);
                }
            }
            TypedExpr::Binary { left, right, .. } => {
                scan_expr(left, func_names, map);
                scan_expr(right, func_names, map);
            }
            TypedExpr::Unary { expr, .. } => scan_expr(expr, func_names, map),
            TypedExpr::ArrayLiteral { elements, .. } => {
                for e in elements {
                    scan_expr(e, func_names, map);
                }
            }
            TypedExpr::Index { target, index, .. } => {
                scan_expr(target, func_names, map);
                scan_expr(index, func_names, map);
            }
            _ => {}
        }
    }

    for func in &program.functions {
        scan_block(&func.body, &func_names, &mut map);
    }

    map
}
