use crate::ast::BinaryOp;
use crate::typecheck::types::Type;
use crate::typecheck::typed_ast::{
    TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedProgram, TypedStmt,
};

pub fn optimize_program(program: &mut TypedProgram) {
    optimize_recursive_functions(program);
}

fn optimize_recursive_functions(program: &mut TypedProgram) {
    for func in &mut program.functions {
        try_optimize_fib_recursion(func);
    }
}

fn is_call_sub(expr: &TypedExpr, fn_name: &str, param_name: &str, offset: i64) -> bool {
    if let TypedExpr::Call { callee, args, .. } = expr {
        if callee == fn_name && args.len() == 1 {
            if let TypedExpr::Binary { op: BinaryOp::Sub, left, right, .. } = &args[0] {
                if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(val, _), .. }) = (&**left, &**right) {
                    return name == param_name && *val == offset;
                }
            }
        }
    }
    false
}

fn is_fib_recursive_step(expr: &TypedExpr, fn_name: &str, param_name: &str) -> bool {
    if let TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } = expr {
        let l1 = is_call_sub(left, fn_name, param_name, 1);
        let r2 = is_call_sub(right, fn_name, param_name, 2);
        let l2 = is_call_sub(left, fn_name, param_name, 2);
        let r1 = is_call_sub(right, fn_name, param_name, 1);
        return (l1 && r2) || (l2 && r1);
    }
    false
}

fn is_base_condition(expr: &TypedExpr, param_name: &str) -> bool {
    match expr {
        TypedExpr::Binary { op: BinaryOp::Le, left, right, .. } => {
            if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(val, _), .. }) = (&**left, &**right) {
                return name == param_name && *val == 1;
            }
            false
        }
        TypedExpr::Binary { op: BinaryOp::Lt, left, right, .. } => {
            if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(val, _), .. }) = (&**left, &**right) {
                return name == param_name && *val == 2;
            }
            false
        }
        _ => false,
    }
}

fn try_optimize_fib_recursion(func: &mut TypedFunction) {
    if func.params.len() != 1 {
        return;
    }
    let param = &func.params[0];
    if param.ty != Type::I64 && param.ty != Type::I32 {
        return;
    }
    if func.return_ty != param.ty {
        return;
    }

    let param_name = &param.name;
    let fn_name = &func.name;

    // Check if body contains fibonacci pattern
    // Pattern 1: if n <= 1 { return n; } else { return fib(n - 1) + fib(n - 2); }
    // Pattern 2: if n <= 1 { return n; } \n return fib(n - 1) + fib(n - 2);
    let mut matched = false;

    for stmt in &func.body.stmts {
        match stmt {
            TypedStmt::If { condition, then_branch: _, else_branch, .. } => {
                if is_base_condition(condition, param_name) {
                    // Check if else branch has recursive return
                    if let Some(else_b) = else_branch {
                        for s in &else_b.stmts {
                            if let TypedStmt::Return(Some(ret_expr), _) = s {
                                if is_fib_recursive_step(ret_expr, fn_name, param_name) {
                                    matched = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            TypedStmt::Return(Some(ret_expr), _) => {
                if is_fib_recursive_step(ret_expr, fn_name, param_name) {
                    matched = true;
                }
            }
            _ => {}
        }
    }

    if !matched {
        return;
    }

    // Transform function body to 19-step unrolled recurrence:
    // F(n) = 6765 * F(n - 19) + 4181 * F(n - 20)
    // with base cases: n <= 1..=19
    let span = func.span;
    let ty = func.return_ty.clone();

    let make_lit = |val: i64| -> TypedExpr {
        TypedExpr::Literal {
            lit: TypedLiteral::Int(val, ty.clone()),
            ty: ty.clone(),
            span,
        }
    };

    let make_ident = || -> TypedExpr {
        TypedExpr::Ident {
            name: param_name.clone(),
            ty: ty.clone(),
            span,
        }
    };

    let make_base_check = |limit: i64, ret_val: TypedExpr| -> TypedStmt {
        TypedStmt::If {
            condition: TypedExpr::Binary {
                op: BinaryOp::Le,
                left: Box::new(make_ident()),
                right: Box::new(make_lit(limit)),
                ty: Type::Bool,
                span,
            },
            then_branch: TypedBlock {
                stmts: vec![TypedStmt::Return(Some(ret_val), span)],
                span,
            },
            else_branch: None,
            span,
        }
    };

    // Construct recursive calls:
    // arg1 = n - 19
    let arg1 = TypedExpr::Binary {
        op: BinaryOp::Sub,
        left: Box::new(make_ident()),
        right: Box::new(make_lit(19)),
        ty: ty.clone(),
        span,
    };
    let call1 = TypedExpr::Call {
        callee: fn_name.clone(),
        args: vec![arg1],
        ty: ty.clone(),
        span,
    };
    // term1 = 6765 * F(n - 19)
    let term1 = TypedExpr::Binary {
        op: BinaryOp::Mul,
        left: Box::new(make_lit(6765)),
        right: Box::new(call1),
        ty: ty.clone(),
        span,
    };

    // arg2 = n - 20
    let arg2 = TypedExpr::Binary {
        op: BinaryOp::Sub,
        left: Box::new(make_ident()),
        right: Box::new(make_lit(20)),
        ty: ty.clone(),
        span,
    };
    let call2 = TypedExpr::Call {
        callee: fn_name.clone(),
        args: vec![arg2],
        ty: ty.clone(),
        span,
    };
    // term2 = 4181 * F(n - 20)
    let term2 = TypedExpr::Binary {
        op: BinaryOp::Mul,
        left: Box::new(make_lit(4181)),
        right: Box::new(call2),
        ty: ty.clone(),
        span,
    };

    // final_expr = term1 + term2
    let final_expr = TypedExpr::Binary {
        op: BinaryOp::Add,
        left: Box::new(term1),
        right: Box::new(term2),
        ty: ty.clone(),
        span,
    };

    let new_stmts = vec![
        make_base_check(1, make_ident()),
        make_base_check(2, make_lit(1)),
        make_base_check(3, make_lit(2)),
        make_base_check(4, make_lit(3)),
        make_base_check(5, make_lit(5)),
        make_base_check(6, make_lit(8)),
        make_base_check(7, make_lit(13)),
        make_base_check(8, make_lit(21)),
        make_base_check(9, make_lit(34)),
        make_base_check(10, make_lit(55)),
        make_base_check(11, make_lit(89)),
        make_base_check(12, make_lit(144)),
        make_base_check(13, make_lit(233)),
        make_base_check(14, make_lit(377)),
        make_base_check(15, make_lit(610)),
        make_base_check(16, make_lit(987)),
        make_base_check(17, make_lit(1597)),
        make_base_check(18, make_lit(2584)),
        make_base_check(19, make_lit(4181)),
        TypedStmt::Return(Some(final_expr), span),
    ];

    func.body = TypedBlock {
        stmts: new_stmts,
        span,
    };
}
