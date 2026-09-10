use std::collections::{HashMap, HashSet};
use crate::ast::BinaryOp;
use crate::typecheck::types::Type;
use crate::typecheck::typed_ast::{
    TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedProgram, TypedStmt,
};

pub fn optimize_arrays(program: &mut TypedProgram) {
    for func in &mut program.functions {
        optimize_function_arrays(func);
    }
}

fn optimize_function_arrays(func: &mut TypedFunction) {
    // Pass 1: Expand any dot(a, b) calls in the function
    expand_dots_in_block(&mut func.body);

    // Pass 2: Find array declarations and mutation sets
    let mut array_literals: HashMap<String, Vec<TypedExpr>> = HashMap::new();
    let mut mutated_indices: HashSet<(String, i64)> = HashSet::new();
    let mut dynamic_mutated: HashSet<String> = HashSet::new();

    collect_array_info(
        &func.body,
        &mut array_literals,
        &mut mutated_indices,
        &mut dynamic_mutated,
    );

    // Pass 3: Propagate constant array elements
    propagate_in_block(
        &mut func.body,
        &array_literals,
        &mutated_indices,
        &dynamic_mutated,
    );

    // Pass 4: Constant folding
    fold_in_block(&mut func.body);
}

fn expand_dots_in_block(block: &mut TypedBlock) {
    for stmt in &mut block.stmts {
        match stmt {
            TypedStmt::Let { value, .. } => expand_dot_calls(value),
            TypedStmt::Assign { value, .. } => expand_dot_calls(value),
            TypedStmt::IndexAssign { index, value, .. } => {
                expand_dot_calls(index);
                expand_dot_calls(value);
            }
            TypedStmt::Return(Some(expr), _) => expand_dot_calls(expr),
            TypedStmt::Expr(expr) => expand_dot_calls(expr),
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                expand_dot_calls(condition);
                expand_dots_in_block(then_branch);
                if let Some(eb) = else_branch {
                    expand_dots_in_block(eb);
                }
            }
            TypedStmt::While { condition, body, .. } => {
                expand_dot_calls(condition);
                expand_dots_in_block(body);
            }
            _ => {}
        }
    }
}

fn expand_dot_calls(expr: &mut TypedExpr) {
    match expr {
        TypedExpr::Call {
            callee,
            args,
            ty,
            span,
        } if callee == "dot" && args.len() == 2 => {
            let left_ty = args[0].ty();
            let right_ty = args[1].ty();
            if let (Type::Array(elem_ty, len_a), Type::Array(_, len_b)) = (&left_ty, &right_ty) {
                if len_a == len_b && *len_a <= 16 && *len_a > 0 {
                    let n = *len_a;
                    let make_prod = |k: usize| -> TypedExpr {
                        let idx_expr = TypedExpr::Literal {
                            lit: TypedLiteral::Int(k as i64, Type::I64),
                            ty: Type::I64,
                            span: *span,
                        };
                        let get_a = TypedExpr::Index {
                            target: Box::new(args[0].clone()),
                            index: Box::new(idx_expr.clone()),
                            is_safe: true,
                            ty: *elem_ty.clone(),
                            span: *span,
                        };
                        let get_b = TypedExpr::Index {
                            target: Box::new(args[1].clone()),
                            index: Box::new(idx_expr),
                            is_safe: true,
                            ty: *elem_ty.clone(),
                            span: *span,
                        };
                        TypedExpr::Binary {
                            op: BinaryOp::Mul,
                            left: Box::new(get_a),
                            right: Box::new(get_b),
                            ty: *elem_ty.clone(),
                            span: *span,
                        }
                    };

                    let mut current = make_prod(0);
                    for k in 1..n {
                        let next_prod = make_prod(k);
                        current = TypedExpr::Binary {
                            op: BinaryOp::Add,
                            left: Box::new(current),
                            right: Box::new(next_prod),
                            ty: ty.clone(),
                            span: *span,
                        };
                    }
                    *expr = current;
                    return;
                }
            }
        }
        TypedExpr::Binary { left, right, .. } => {
            expand_dot_calls(left);
            expand_dot_calls(right);
        }
        TypedExpr::Unary { expr: inner, .. } => {
            expand_dot_calls(inner);
        }
        _ => {}
    }
}

fn collect_array_info(
    block: &TypedBlock,
    array_literals: &mut HashMap<String, Vec<TypedExpr>>,
    mutated_indices: &mut HashSet<(String, i64)>,
    dynamic_mutated: &mut HashSet<String>,
) {
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let {
                name,
                value: TypedExpr::ArrayLiteral { elements, .. },
                ..
            } => {
                array_literals.insert(name.clone(), elements.clone());
            }
            TypedStmt::Assign { name, .. } => {
                dynamic_mutated.insert(name.clone());
                array_literals.remove(name);
            }
            TypedStmt::IndexAssign {
                target,
                index,
                ..
            } => {
                if let TypedExpr::Literal {
                    lit: TypedLiteral::Int(idx, _),
                    ..
                } = index
                {
                    mutated_indices.insert((target.clone(), *idx));
                } else {
                    dynamic_mutated.insert(target.clone());
                    array_literals.remove(target);
                }
            }
            TypedStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_array_info(then_branch, array_literals, mutated_indices, dynamic_mutated);
                if let Some(eb) = else_branch {
                    collect_array_info(eb, array_literals, mutated_indices, dynamic_mutated);
                }
            }
            TypedStmt::While { body, .. } => {
                collect_array_info(body, array_literals, mutated_indices, dynamic_mutated);
            }
            _ => {}
        }
    }
}

fn propagate_in_block(
    block: &mut TypedBlock,
    array_literals: &HashMap<String, Vec<TypedExpr>>,
    mutated_indices: &HashSet<(String, i64)>,
    dynamic_mutated: &HashSet<String>,
) {
    for stmt in &mut block.stmts {
        match stmt {
            TypedStmt::Let { value, .. } => {
                propagate_array_elements(value, array_literals, mutated_indices, dynamic_mutated);
            }
            TypedStmt::Assign { value, .. } => {
                propagate_array_elements(value, array_literals, mutated_indices, dynamic_mutated);
            }
            TypedStmt::IndexAssign { index, value, .. } => {
                propagate_array_elements(index, array_literals, mutated_indices, dynamic_mutated);
                propagate_array_elements(value, array_literals, mutated_indices, dynamic_mutated);
            }
            TypedStmt::Return(Some(expr), _) => {
                propagate_array_elements(expr, array_literals, mutated_indices, dynamic_mutated);
            }
            TypedStmt::Expr(expr) => {
                propagate_array_elements(expr, array_literals, mutated_indices, dynamic_mutated);
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                propagate_array_elements(condition, array_literals, mutated_indices, dynamic_mutated);
                propagate_in_block(then_branch, array_literals, mutated_indices, dynamic_mutated);
                if let Some(eb) = else_branch {
                    propagate_in_block(eb, array_literals, mutated_indices, dynamic_mutated);
                }
            }
            TypedStmt::While { condition, body, .. } => {
                propagate_array_elements(condition, array_literals, mutated_indices, dynamic_mutated);
                propagate_in_block(body, array_literals, mutated_indices, dynamic_mutated);
            }
            _ => {}
        }
    }
}

fn propagate_array_elements(
    expr: &mut TypedExpr,
    array_literals: &HashMap<String, Vec<TypedExpr>>,
    mutated_indices: &HashSet<(String, i64)>,
    dynamic_mutated: &HashSet<String>,
) {
    match expr {
        TypedExpr::Index {
            target,
            index,
            ..
        } => {
            if let TypedExpr::Ident { name, .. } = &**target {
                if !dynamic_mutated.contains(name) {
                    if let TypedExpr::Literal {
                        lit: TypedLiteral::Int(idx, _),
                        ..
                    } = &**index
                    {
                        if !mutated_indices.contains(&(name.clone(), *idx)) {
                            if let Some(elements) = array_literals.get(name) {
                                if (*idx as usize) < elements.len() {
                                    *expr = elements[*idx as usize].clone();
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            propagate_array_elements(target, array_literals, mutated_indices, dynamic_mutated);
            propagate_array_elements(index, array_literals, mutated_indices, dynamic_mutated);
        }
        TypedExpr::Binary { left, right, .. } => {
            propagate_array_elements(left, array_literals, mutated_indices, dynamic_mutated);
            propagate_array_elements(right, array_literals, mutated_indices, dynamic_mutated);
        }
        TypedExpr::Unary { expr: inner, .. } => {
            propagate_array_elements(inner, array_literals, mutated_indices, dynamic_mutated);
        }
        TypedExpr::Call { args, .. } => {
            for a in args {
                propagate_array_elements(a, array_literals, mutated_indices, dynamic_mutated);
            }
        }
        _ => {}
    }
}

fn fold_in_block(block: &mut TypedBlock) {
    for stmt in &mut block.stmts {
        match stmt {
            TypedStmt::Let { value, .. } => fold_constants(value),
            TypedStmt::Assign { value, .. } => fold_constants(value),
            TypedStmt::IndexAssign { index, value, .. } => {
                fold_constants(index);
                fold_constants(value);
            }
            TypedStmt::Return(Some(expr), _) => fold_constants(expr),
            TypedStmt::Expr(expr) => fold_constants(expr),
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                fold_constants(condition);
                fold_in_block(then_branch);
                if let Some(eb) = else_branch {
                    fold_in_block(eb);
                }
            }
            TypedStmt::While { condition, body, .. } => {
                fold_constants(condition);
                fold_in_block(body);
            }
            _ => {}
        }
    }
}

fn fold_constants(expr: &mut TypedExpr) {
    match expr {
        TypedExpr::Binary {
            op,
            left,
            right,
            ty,
            span,
        } => {
            fold_constants(left);
            fold_constants(right);
            if let (
                TypedExpr::Literal {
                    lit: TypedLiteral::Int(l, _),
                    ..
                },
                TypedExpr::Literal {
                    lit: TypedLiteral::Int(r, _),
                    ..
                },
            ) = (&**left, &**right)
            {
                let res = match op {
                    BinaryOp::Add => Some(l.wrapping_add(*r)),
                    BinaryOp::Sub => Some(l.wrapping_sub(*r)),
                    BinaryOp::Mul => Some(l.wrapping_mul(*r)),
                    _ => None,
                };
                if let Some(val) = res {
                    *expr = TypedExpr::Literal {
                        lit: TypedLiteral::Int(val, ty.clone()),
                        ty: ty.clone(),
                        span: *span,
                    };
                }
            }
        }
        TypedExpr::Unary {
            op,
            expr: inner,
            ty,
            span,
        } => {
            fold_constants(inner);
            if let TypedExpr::Literal {
                lit: TypedLiteral::Int(val, _),
                ..
            } = &**inner
            {
                if let crate::ast::UnaryOp::Neg = op {
                    *expr = TypedExpr::Literal {
                        lit: TypedLiteral::Int(-val, ty.clone()),
                        ty: ty.clone(),
                        span: *span,
                    };
                }
            }
        }
        _ => {}
    }
}
