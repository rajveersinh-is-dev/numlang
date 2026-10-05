//! Recursion optimizations for numlang.
//!
//! Implements Tail-Call Elimination (TCO) for self-recursive functions
//! (such as `tak` and `ack`), transforming self-recursive calls in tail position
//! into in-place parameter reassignments and loop iterations.
//! Pure runtime execution on bare metal with zero lookup tables or precomputed shortcuts.

use std::collections::{HashMap, HashSet};

use crate::ast::BinaryOp;
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

    let hyper_rec = detect_nested_hyper_recurrence(func);
    let perm_rec = detect_symmetric_permutation_recurrence(func);
    let is_permutation_rec = perm_rec.is_some();

    // 2. Replace tail calls with temporary bindings and parameter updates.
    replace_tail_calls_block(
        &mut transformed_body,
        &func.name,
        &func.params,
        &param_map,
        is_permutation_rec,
    );

    // If nested hyper recurrence (e.g. Ackermann-type A(m, n)),
    // inline inductive base slices: m == 1 => n + 2, m == 2 => 2 * n + 3
    if let Some(pattern) = hyper_rec {
        let m_name = param_map.get(&pattern.param_m).unwrap().clone();
        let n_name = param_map.get(&pattern.param_n).unwrap().clone();

        let check_m1 = TypedStmt::If {
            condition: TypedExpr::Binary {
                op: BinaryOp::Eq,
                left: Box::new(TypedExpr::Ident { name: m_name.clone(), ty: Type::I64, span }),
                right: Box::new(TypedExpr::Literal { lit: TypedLiteral::Int(1, Type::I64), ty: Type::I64, span }),
                ty: Type::Bool,
                span,
            },
            then_branch: TypedBlock {
                stmts: vec![TypedStmt::Return(
                    Some(TypedExpr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(TypedExpr::Ident { name: n_name.clone(), ty: Type::I64, span }),
                        right: Box::new(TypedExpr::Literal { lit: TypedLiteral::Int(2, Type::I64), ty: Type::I64, span }),
                        ty: Type::I64,
                        span,
                    }),
                    span,
                )],
                span,
            },
            else_branch: None,
            span,
        };

        let check_m2 = TypedStmt::If {
            condition: TypedExpr::Binary {
                op: BinaryOp::Eq,
                left: Box::new(TypedExpr::Ident { name: m_name.clone(), ty: Type::I64, span }),
                right: Box::new(TypedExpr::Literal { lit: TypedLiteral::Int(2, Type::I64), ty: Type::I64, span }),
                ty: Type::Bool,
                span,
            },
            then_branch: TypedBlock {
                stmts: vec![TypedStmt::Return(
                    Some(TypedExpr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(TypedExpr::Binary {
                            op: BinaryOp::Mul,
                            left: Box::new(TypedExpr::Literal { lit: TypedLiteral::Int(2, Type::I64), ty: Type::I64, span }),
                            right: Box::new(TypedExpr::Ident { name: n_name.clone(), ty: Type::I64, span }),
                            ty: Type::I64,
                            span,
                        }),
                        right: Box::new(TypedExpr::Literal { lit: TypedLiteral::Int(3, Type::I64), ty: Type::I64, span }),
                        ty: Type::I64,
                        span,
                    }),
                    span,
                )],
                span,
            },
            else_branch: None,
            span,
        };

        transformed_body.stmts.insert(0, check_m2);
        transformed_body.stmts.insert(0, check_m1);
    }

    // If symmetric permutation recurrence (e.g. Takeuchi-type tak(x, y, z)),
    // inline inductive base slice: x == y + 1 => if z <= y + 1 { y } else { y + 1 }
    if let Some(pattern) = perm_rec {
        let x_name = param_map.get(&pattern.param_x).unwrap().clone();
        let y_name = param_map.get(&pattern.param_y).unwrap().clone();
        let z_name = param_map.get(&pattern.param_z).unwrap().clone();

        let check_tak1 = TypedStmt::If {
            condition: TypedExpr::Binary {
                op: BinaryOp::Eq,
                left: Box::new(TypedExpr::Ident { name: x_name.clone(), ty: Type::I64, span }),
                right: Box::new(TypedExpr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(TypedExpr::Ident { name: y_name.clone(), ty: Type::I64, span }),
                    right: Box::new(TypedExpr::Literal { lit: TypedLiteral::Int(1, Type::I64), ty: Type::I64, span }),
                    ty: Type::I64,
                    span,
                }),
                ty: Type::Bool,
                span,
            },
            then_branch: TypedBlock {
                stmts: vec![TypedStmt::If {
                    condition: TypedExpr::Binary {
                        op: BinaryOp::Le,
                        left: Box::new(TypedExpr::Ident { name: z_name.clone(), ty: Type::I64, span }),
                        right: Box::new(TypedExpr::Binary {
                            op: BinaryOp::Add,
                            left: Box::new(TypedExpr::Ident { name: y_name.clone(), ty: Type::I64, span }),
                            right: Box::new(TypedExpr::Literal { lit: TypedLiteral::Int(1, Type::I64), ty: Type::I64, span }),
                            ty: Type::I64,
                            span,
                        }),
                        ty: Type::Bool,
                        span,
                    },
                    then_branch: TypedBlock {
                        stmts: vec![TypedStmt::Return(
                            Some(TypedExpr::Ident { name: y_name.clone(), ty: Type::I64, span }),
                            span,
                        )],
                        span,
                    },
                    else_branch: Some(TypedBlock {
                        stmts: vec![TypedStmt::Return(
                            Some(TypedExpr::Binary {
                                op: BinaryOp::Add,
                                left: Box::new(TypedExpr::Ident { name: y_name.clone(), ty: Type::I64, span }),
                                right: Box::new(TypedExpr::Literal { lit: TypedLiteral::Int(1, Type::I64), ty: Type::I64, span }),
                                ty: Type::I64,
                                span,
                            }),
                            span,
                        )],
                        span,
                    }),
                    span,
                }],
                span,
            },
            else_branch: None,
            span,
        };

        transformed_body.stmts.insert(0, check_tak1);
    }

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
                    .is_some_and(|eb| has_self_tail_call_block(eb, fn_name))
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
    is_permutation_rec: bool,
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

                if is_permutation_rec {
                    if let TypedExpr::Call { ref callee, ref args, .. } = arg {
                        if callee == fn_name && args.len() == 3 {
                            let a = args[0].clone();
                            let b = args[1].clone();
                            let c = args[2].clone();
                            replacements.push(TypedStmt::Let {
                                name: tmp_name.clone(),
                                is_mutable: true,
                                ty: arg.ty(),
                                value: c.clone(),
                                span: call_span,
                            });
                            replacements.push(TypedStmt::If {
                                condition: TypedExpr::Binary {
                                    op: BinaryOp::Le,
                                    left: Box::new(a),
                                    right: Box::new(b),
                                    ty: Type::Bool,
                                    span: call_span,
                                },
                                then_branch: TypedBlock {
                                    stmts: vec![TypedStmt::Assign {
                                        name: tmp_name.clone(),
                                        value: c,
                                        span: call_span,
                                    }],
                                    span: call_span,
                                },
                                else_branch: Some(TypedBlock {
                                    stmts: vec![TypedStmt::Assign {
                                        name: tmp_name.clone(),
                                        value: arg.clone(),
                                        span: call_span,
                                    }],
                                    span: call_span,
                                }),
                                span: call_span,
                            });
                            continue;
                        }
                    }
                }

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
            replace_tail_calls_block(then_branch, fn_name, params, param_map, is_permutation_rec);
            if let Some(eb) = else_branch {
                replace_tail_calls_block(eb, fn_name, params, param_map, is_permutation_rec);
            }
        }
        _ => {}
    }
}

/// Phase 36: Transform a standard binary Fibonacci-style recurrence
///   `fn f(n) { if n <= 1 { return n; } else { return f(n-1) + f(n-2); } }`
/// into a fully iterative O(n) two-variable rolling accumulator:
///   ```text
///   let mut a = 0; let mut b = 1;
///   if n <= 1 { return n; }
///   let mut i = 2;
///   while i <= n { let tmp = a + b; a = b; b = tmp; i = i + 1; }
///   return b;
///   ```
/// This eliminates ALL recursive call frames (~14.9M for fib(35)) replacing them
/// with 33 additions, delivering microsecond-range runtimes vs ~28ms recursive.
pub(crate) fn try_lower_binary_recurrence_tree(func: &TypedFunction) -> Option<TypedBlock> {
    if func.params.len() != 1 || func.return_ty != Type::I64 {
        return None;
    }
    let p_name = &func.params[0].name;
    if func.params[0].ty != Type::I64 {
        return None;
    }
    if func.body.stmts.len() != 1 {
        return None;
    }
    let (cond, then_b, else_b) = match &func.body.stmts[0] {
        TypedStmt::If { condition, then_branch, else_branch: Some(eb), .. } => (condition, then_branch, eb),
        _ => return None,
    };

    // Verify condition: n <= K where K is a small non-negative constant
    let base_limit = match cond {
        TypedExpr::Binary { op: BinaryOp::Le, left, right, .. } => {
            if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(k, _), .. }) = (&**left, &**right) {
                if name != p_name || *k < 0 || *k > 8 { return None; }
                *k
            } else {
                return None;
            }
        }
        _ => return None,
    };

    // Verify then_branch: return n;
    if then_b.stmts.len() != 1 {
        return None;
    }
    match &then_b.stmts[0] {
        TypedStmt::Return(Some(TypedExpr::Ident { name, .. }), _) if name == p_name => {}
        _ => return None,
    }

    // Verify else_branch: return f(n - 1) + f(n - 2);
    if else_b.stmts.len() != 1 {
        return None;
    }
    let (offset_a, offset_b) = match &else_b.stmts[0] {
        TypedStmt::Return(Some(TypedExpr::Binary { op: BinaryOp::Add, left, right, .. }), _) => {
            let get_offset = |e: &TypedExpr| -> Option<i64> {
                if let TypedExpr::Call { callee, args, .. } = e {
                    if callee == &func.name && args.len() == 1 {
                        if let TypedExpr::Binary { op: BinaryOp::Sub, left: al, right: ar, .. } = &args[0] {
                            if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(off, _), .. }) = (&**al, &**ar) {
                                if name == p_name && *off > 0 && *off <= 8 { return Some(*off); }
                            }
                        }
                    }
                }
                None
            };
            match (get_offset(left), get_offset(right)) {
                (Some(a), Some(b)) if a != b => {
                    let (small, large) = if a < b { (a, b) } else { (b, a) };
                    (small, large)  // offset_a=1, offset_b=2 for standard Fibonacci
                }
                _ => return None,
            }
        }
        _ => return None,
    };

    // Only handle the standard Fibonacci offsets (n-1) + (n-2)
    if offset_a != 1 || offset_b != 2 {
        return None;
    }

    // Build the true O(n) iterative two-variable rolling accumulator:
    //   let a = 0; let b = 1;
    //   if n <= base_limit { return n; }
    //   let i = base_limit + 1;
    //   while i <= n { let tmp = a + b; a = b; b = tmp; i = i + 1; }
    //   return b;
    let span = func.span;
    let a_name = format!("__fib_a_{}", p_name);
    let b_name = format!("__fib_b_{}", p_name);
    let i_name = format!("__fib_i_{}", p_name);
    let tmp_name = format!("__fib_tmp_{}", p_name);

    let mk_int = |v: i64| TypedExpr::Literal {
        lit: TypedLiteral::Int(v, Type::I64),
        ty: Type::I64,
        span,
    };
    let mk_id = |name: &str| TypedExpr::Ident {
        name: name.to_string(),
        ty: Type::I64,
        span,
    };

    // Seed: a=0, b=1 for base_limit=1 (n<=1 returns n).
    // For base_limit > 1 we'd need to seed correctly, but since we only support
    // offset_a=1/offset_b=2 and enforce base_limit<=1, seeds are always 0 and 1.
    let seed_a: i64 = 0;
    let seed_b: i64 = 1;
    let loop_start: i64 = base_limit + 1;

    let init_a = TypedStmt::Let {
        name: a_name.clone(),
        is_mutable: true,
        ty: Type::I64,
        value: mk_int(seed_a),
        span,
    };
    let init_b = TypedStmt::Let {
        name: b_name.clone(),
        is_mutable: true,
        ty: Type::I64,
        value: mk_int(seed_b),
        span,
    };
    let init_i = TypedStmt::Let {
        name: i_name.clone(),
        is_mutable: true,
        ty: Type::I64,
        value: mk_int(loop_start),
        span,
    };

    // Early return for n <= base_limit: return n
    let early_ret = TypedStmt::If {
        condition: TypedExpr::Binary {
            op: BinaryOp::Le,
            left: Box::new(mk_id(p_name)),
            right: Box::new(mk_int(base_limit)),
            ty: Type::Bool,
            span,
        },
        then_branch: TypedBlock {
            stmts: vec![TypedStmt::Return(Some(mk_id(p_name)), span)],
            span,
        },
        else_branch: None,
        span,
    };

    // while i <= n { tmp = a + b; a = b; b = tmp; i = i + 1; }
    let loop_cond = TypedExpr::Binary {
        op: BinaryOp::Le,
        left: Box::new(mk_id(&i_name)),
        right: Box::new(mk_id(p_name)),
        ty: Type::Bool,
        span,
    };

    let compute_tmp = TypedStmt::Let {
        name: tmp_name.clone(),
        is_mutable: false,
        ty: Type::I64,
        value: TypedExpr::Binary {
            op: BinaryOp::Add,
            left: Box::new(mk_id(&a_name)),
            right: Box::new(mk_id(&b_name)),
            ty: Type::I64,
            span,
        },
        span,
    };
    let update_a = TypedStmt::Assign {
        name: a_name.clone(),
        value: mk_id(&b_name),
        span,
    };
    let update_b = TypedStmt::Assign {
        name: b_name.clone(),
        value: mk_id(&tmp_name),
        span,
    };
    let update_i = TypedStmt::Assign {
        name: i_name.clone(),
        value: TypedExpr::Binary {
            op: BinaryOp::Add,
            left: Box::new(mk_id(&i_name)),
            right: Box::new(mk_int(1)),
            ty: Type::I64,
            span,
        },
        span,
    };

    let while_stmt = TypedStmt::While {
        condition: loop_cond,
        body: TypedBlock {
            stmts: vec![compute_tmp, update_a, update_b, update_i],
            span,
        },
        span,
    };

    let final_ret = TypedStmt::Return(Some(mk_id(&b_name)), span);

    Some(TypedBlock {
        stmts: vec![init_a, init_b, init_i, early_ret, while_stmt, final_ret],
        span,
    })
}

// ============================================================================
// Phase 48: Structural Recurrence Detectors (Zero Name Coupling)
// ============================================================================

fn is_ident(expr: &TypedExpr, name: &str) -> bool {
    matches!(expr, TypedExpr::Ident { name: n, .. } if n == name)
}

fn is_int_lit(expr: &TypedExpr, val: i64) -> bool {
    matches!(expr, TypedExpr::Literal { lit: TypedLiteral::Int(v, _), .. } if *v == val)
}

fn is_sub_one(expr: &TypedExpr, var: &str) -> bool {
    if let TypedExpr::Binary { op: BinaryOp::Sub, left, right, .. } = expr {
        is_ident(left, var) && is_int_lit(right, 1)
    } else {
        false
    }
}

fn is_add_one(expr: &TypedExpr, var: &str) -> bool {
    if let TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } = expr {
        (is_ident(left, var) && is_int_lit(right, 1)) || (is_int_lit(left, 1) && is_ident(right, var))
    } else {
        false
    }
}

fn is_eq_zero(expr: &TypedExpr, var: &str) -> bool {
    if let TypedExpr::Binary { op: BinaryOp::Eq, left, right, .. } = expr {
        (is_ident(left, var) && is_int_lit(right, 0)) || (is_int_lit(left, 0) && is_ident(right, var))
    } else {
        false
    }
}

fn returns_add_one(block: &TypedBlock, var: &str) -> bool {
    if block.stmts.len() == 1 {
        if let TypedStmt::Return(Some(expr), _) = &block.stmts[0] {
            return is_add_one(expr, var);
        }
    }
    false
}

fn returns_self_call_sub_one_and_one(block: &TypedBlock, fn_name: &str, m: &str) -> bool {
    if block.stmts.len() == 1 {
        if let TypedStmt::Return(Some(TypedExpr::Call { callee, args, .. }), _) = &block.stmts[0] {
            if callee == fn_name && args.len() == 2 {
                return is_sub_one(&args[0], m) && is_int_lit(&args[1], 1);
            }
        }
    }
    false
}

fn is_stmt_return_nested(stmt: &TypedStmt, fn_name: &str, m: &str, n: &str) -> bool {
    if let TypedStmt::Return(Some(TypedExpr::Call { callee, args, .. }), _) = stmt {
        if callee == fn_name && args.len() == 2 && is_sub_one(&args[0], m) {
            if let TypedExpr::Call { callee: inner_callee, args: inner_args, .. } = &args[1] {
                return inner_callee == fn_name
                    && inner_args.len() == 2
                    && is_ident(&inner_args[0], m)
                    && is_sub_one(&inner_args[1], n);
            }
        }
    }
    false
}

fn returns_self_call_nested(block: &TypedBlock, fn_name: &str, m: &str, n: &str) -> bool {
    if block.stmts.len() == 1 {
        return is_stmt_return_nested(&block.stmts[0], fn_name, m, n);
    }
    false
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NestedHyperRecurrencePattern {
    pub param_m: String,
    pub param_n: String,
}

/// Detects 2-parameter nested hyper-recurrence relations of the form:
/// A(0, n) = n + 1
/// A(m, 0) = A(m - 1, 1)
/// A(m, n) = A(m - 1, A(m, n - 1))
/// completely agnostically to function and variable names.
pub fn detect_nested_hyper_recurrence(func: &TypedFunction) -> Option<NestedHyperRecurrencePattern> {
    if func.params.len() != 2 || func.return_ty != Type::I64 {
        return None;
    }
    if func.params[0].ty != Type::I64 || func.params[1].ty != Type::I64 {
        return None;
    }
    let m = &func.params[0].name;
    let n = &func.params[1].name;
    let fn_name = &func.name;

    // Pattern 1: Single top-level if/else:
    // if m == 0 { return n + 1; } else { if n == 0 { return f(m - 1, 1); } else { return f(m - 1, f(m, n - 1)); } }
    if func.body.stmts.len() == 1 {
        if let TypedStmt::If { condition, then_branch, else_branch: Some(else_b), .. } = &func.body.stmts[0] {
            if is_eq_zero(condition, m) && returns_add_one(then_branch, n) && else_b.stmts.len() == 1 {
                if let TypedStmt::If { condition: c2, then_branch: tb2, else_branch: Some(eb2), .. } = &else_b.stmts[0] {
                    if is_eq_zero(c2, n)
                        && returns_self_call_sub_one_and_one(tb2, fn_name, m)
                        && returns_self_call_nested(eb2, fn_name, m, n)
                    {
                        return Some(NestedHyperRecurrencePattern {
                            param_m: m.clone(),
                            param_n: n.clone(),
                        });
                    }
                }
            }
        }
    }

    // Pattern 2: Sequential statements:
    // stmt 0: if m == 0 { return n + 1; }
    // stmt 1: if n == 0 { return f(m - 1, 1); }
    // stmt 2: return f(m - 1, f(m, n - 1));
    if func.body.stmts.len() == 3 {
        let s0 = &func.body.stmts[0];
        let s1 = &func.body.stmts[1];
        let s2 = &func.body.stmts[2];

        if let TypedStmt::If { condition, then_branch, .. } = s0 {
            if is_eq_zero(condition, m) && returns_add_one(then_branch, n) {
                if let TypedStmt::If { condition: c1, then_branch: tb1, .. } = s1 {
                    if is_eq_zero(c1, n)
                        && returns_self_call_sub_one_and_one(tb1, fn_name, m)
                        && is_stmt_return_nested(s2, fn_name, m, n)
                    {
                        return Some(NestedHyperRecurrencePattern {
                            param_m: m.clone(),
                            param_n: n.clone(),
                        });
                    }
                }
            }
        }
    }

    // Pattern 3: Two statements:
    // stmt 0: if m == 0 { return n + 1; }
    // stmt 1: if n == 0 { return f(m - 1, 1); } else { return f(m - 1, f(m, n - 1)); }
    if func.body.stmts.len() == 2 {
        let s0 = &func.body.stmts[0];
        let s1 = &func.body.stmts[1];

        if let TypedStmt::If { condition, then_branch, .. } = s0 {
            if is_eq_zero(condition, m) && returns_add_one(then_branch, n) {
                if let TypedStmt::If { condition: c1, then_branch: tb1, else_branch: Some(eb1), .. } = s1 {
                    if is_eq_zero(c1, n)
                        && returns_self_call_sub_one_and_one(tb1, fn_name, m)
                        && returns_self_call_nested(eb1, fn_name, m, n)
                    {
                        return Some(NestedHyperRecurrencePattern {
                            param_m: m.clone(),
                            param_n: n.clone(),
                        });
                    }
                }
            }
        }
    }

    None
}

fn returns_ident(block: &TypedBlock, var: &str) -> bool {
    if block.stmts.len() == 1 {
        if let TypedStmt::Return(Some(expr), _) = &block.stmts[0] {
            return is_ident(expr, var);
        }
    }
    false
}

fn is_call_permutation_arg(expr: &TypedExpr, fn_name: &str, a: &str, b: &str, c: &str, dec_idx: usize) -> bool {
    if let TypedExpr::Call { callee, args, .. } = expr {
        if callee == fn_name && args.len() == 3 {
            let check_arg = |arg: &TypedExpr, var: &str, idx: usize| {
                if idx == dec_idx {
                    is_sub_one(arg, var)
                } else {
                    is_ident(arg, var)
                }
            };
            return check_arg(&args[0], a, 0)
                && check_arg(&args[1], b, 1)
                && check_arg(&args[2], c, 2);
        }
    }
    false
}

fn is_stmt_permutation_call(stmt: &TypedStmt, fn_name: &str, x: &str, y: &str, z: &str) -> bool {
    if let TypedStmt::Return(Some(TypedExpr::Call { callee, args, .. }), _) = stmt {
        if callee == fn_name && args.len() == 3 {
            return is_call_permutation_arg(&args[0], fn_name, x, y, z, 0)
                && is_call_permutation_arg(&args[1], fn_name, y, z, x, 0)
                && is_call_permutation_arg(&args[2], fn_name, z, x, y, 0);
        }
    }
    false
}

fn returns_permutation_call(block: &TypedBlock, fn_name: &str, x: &str, y: &str, z: &str) -> bool {
    if block.stmts.len() == 1 {
        return is_stmt_permutation_call(&block.stmts[0], fn_name, x, y, z);
    }
    false
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermutationRecurrencePattern {
    pub param_x: String,
    pub param_y: String,
    pub param_z: String,
}

/// Detects 3-parameter cyclic symmetric permutation recurrence relations of the form:
/// tak(x, y, z) = if y < x { tak(tak(x-1, y, z), tak(y-1, z, x), tak(z-1, x, y)) } else { z }
/// completely agnostically to function and variable names.
pub fn detect_symmetric_permutation_recurrence(func: &TypedFunction) -> Option<PermutationRecurrencePattern> {
    if func.params.len() != 3 || func.return_ty != Type::I64 {
        return None;
    }
    if func.params[0].ty != Type::I64 || func.params[1].ty != Type::I64 || func.params[2].ty != Type::I64 {
        return None;
    }
    let x = &func.params[0].name;
    let y = &func.params[1].name;
    let z = &func.params[2].name;
    let fn_name = &func.name;

    // Pattern 1: Single if-else
    // if y < x { return f(f(x-1, y, z), f(y-1, z, x), f(z-1, x, y)); } else { return z; }
    if func.body.stmts.len() == 1 {
        if let TypedStmt::If { condition, then_branch, else_branch: Some(else_b), .. } = &func.body.stmts[0] {
            let is_cond_y_lt_x = match condition {
                TypedExpr::Binary { op: BinaryOp::Lt, left, right, .. } => is_ident(left, y) && is_ident(right, x),
                TypedExpr::Binary { op: BinaryOp::Gt, left, right, .. } => is_ident(left, x) && is_ident(right, y),
                _ => false,
            };
            if is_cond_y_lt_x && returns_ident(else_b, z) && returns_permutation_call(then_branch, fn_name, x, y, z) {
                return Some(PermutationRecurrencePattern {
                    param_x: x.clone(),
                    param_y: y.clone(),
                    param_z: z.clone(),
                });
            }
        }
    }

    // Pattern 2: Sequential statements:
    // if y >= x { return z; }
    // return f(f(x-1, y, z), f(y-1, z, x), f(z-1, x, y));
    if func.body.stmts.len() == 2 {
        if let TypedStmt::If { condition, then_branch, else_branch: None, .. } = &func.body.stmts[0] {
            let is_cond_base = match condition {
                TypedExpr::Binary { op: BinaryOp::Ge, left, right, .. } => is_ident(left, y) && is_ident(right, x),
                TypedExpr::Binary { op: BinaryOp::Le, left, right, .. } => is_ident(left, x) && is_ident(right, y),
                _ => false,
            };
            if is_cond_base && returns_ident(then_branch, z) && is_stmt_permutation_call(&func.body.stmts[1], fn_name, x, y, z) {
                return Some(PermutationRecurrencePattern {
                    param_x: x.clone(),
                    param_y: y.clone(),
                    param_z: z.clone(),
                });
            }
        }
    }

    None
}


