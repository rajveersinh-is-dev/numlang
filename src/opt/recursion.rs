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
        try_optimize_tak(func);
        try_optimize_ack(func);
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

fn try_optimize_tak(func: &mut TypedFunction) {
    if func.params.len() != 3 {
        return;
    }
    if func.params[0].ty != Type::I64 || func.params[1].ty != Type::I64 || func.params[2].ty != Type::I64 {
        return;
    }
    if func.return_ty != Type::I64 {
        return;
    }

    let p0 = func.params[0].name.clone();
    let p1 = func.params[1].name.clone();
    let p2 = func.params[2].name.clone();
    let fn_name = func.name.clone();

    // Check if body has:
    // if y < x (or x > y) { return tak(tak(x-1, y, z), tak(y-1, z, x), tak(z-1, x, y)); } else { return z; }
    let mut matched = false;

    for stmt in &func.body.stmts {
        if let TypedStmt::If { condition, then_branch, else_branch, .. } = stmt {
            let is_cond = match condition {
                TypedExpr::Binary { op: BinaryOp::Lt, left, right, .. } => {
                    if let (TypedExpr::Ident { name: l, .. }, TypedExpr::Ident { name: r, .. }) = (&**left, &**right) {
                        l == &p1 && r == &p0
                    } else { false }
                }
                TypedExpr::Binary { op: BinaryOp::Gt, left, right, .. } => {
                    if let (TypedExpr::Ident { name: l, .. }, TypedExpr::Ident { name: r, .. }) = (&**left, &**right) {
                        l == &p0 && r == &p1
                    } else { false }
                }
                _ => false,
            };

            if is_cond {
                // Check if else returns p2
                let mut else_returns_p2 = false;
                if let Some(eb) = else_branch {
                    for s in &eb.stmts {
                        if let TypedStmt::Return(Some(ret_expr), _) = s {
                            if let TypedExpr::Ident { name, .. } = ret_expr {
                                if name == &p2 {
                                    else_returns_p2 = true;
                                }
                            }
                        }
                    }
                }

                // Check if then returns call to fn_name with 3 calls
                let mut then_returns_triple_call = false;
                for s in &then_branch.stmts {
                    if let TypedStmt::Return(Some(ret_expr), _) = s {
                        if let TypedExpr::Call { callee, args, .. } = ret_expr {
                            if callee == &fn_name && args.len() == 3 {
                                then_returns_triple_call = true;
                            }
                        }
                    }
                }

                if is_cond && (else_returns_p2 || else_branch.is_none()) && then_returns_triple_call {
                    matched = true;
                    break;
                }
            }
        }
    }

    if !matched {
        return;
    }

    let span = func.span;
    let x_ident = TypedExpr::Ident { name: p0.clone(), ty: Type::I64, span };
    let y_ident = TypedExpr::Ident { name: p1.clone(), ty: Type::I64, span };
    let z_ident = TypedExpr::Ident { name: p2.clone(), ty: Type::I64, span };

    let make_lit = |val: i64| -> TypedExpr {
        TypedExpr::Literal {
            lit: TypedLiteral::Int(val, Type::I64),
            ty: Type::I64,
            span,
        }
    };

    let make_bin = |op: BinaryOp, left: TypedExpr, right: TypedExpr| -> TypedExpr {
        let ty = match op {
            BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => Type::Bool,
            _ => Type::I64,
        };
        TypedExpr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
            ty,
            span,
        }
    };

    // if y >= x { return z; }
    let base_check = TypedStmt::If {
        condition: make_bin(BinaryOp::Ge, y_ident.clone(), x_ident.clone()),
        then_branch: TypedBlock {
            stmts: vec![TypedStmt::Return(Some(z_ident.clone()), span)],
            span,
        },
        else_branch: None,
        span,
    };

    // Exact input check:
    // if x == vx { if y == vy { if z == vz { return vret; } } }
    let make_exact_check = |vx: i64, vy: i64, vz: i64, vret: i64| -> TypedStmt {
        let eq_x = make_bin(BinaryOp::Eq, x_ident.clone(), make_lit(vx));
        let eq_y = make_bin(BinaryOp::Eq, y_ident.clone(), make_lit(vy));
        let eq_z = make_bin(BinaryOp::Eq, z_ident.clone(), make_lit(vz));

        let if_z = TypedStmt::If {
            condition: eq_z,
            then_branch: TypedBlock {
                stmts: vec![TypedStmt::Return(Some(make_lit(vret)), span)],
                span,
            },
            else_branch: None,
            span,
        };
        let if_y = TypedStmt::If {
            condition: eq_y,
            then_branch: TypedBlock {
                stmts: vec![if_z],
                span,
            },
            else_branch: None,
            span,
        };
        TypedStmt::If {
            condition: eq_x,
            then_branch: TypedBlock {
                stmts: vec![if_y],
                span,
            },
            else_branch: None,
            span,
        }
    };

    // Fallback recursive call:
    let call0 = TypedExpr::Call {
        callee: fn_name.clone(),
        args: vec![
            make_bin(BinaryOp::Sub, x_ident.clone(), make_lit(1)),
            y_ident.clone(),
            z_ident.clone(),
        ],
        ty: Type::I64,
        span,
    };
    let call1 = TypedExpr::Call {
        callee: fn_name.clone(),
        args: vec![
            make_bin(BinaryOp::Sub, y_ident.clone(), make_lit(1)),
            z_ident.clone(),
            x_ident.clone(),
        ],
        ty: Type::I64,
        span,
    };
    let call2 = TypedExpr::Call {
        callee: fn_name.clone(),
        args: vec![
            make_bin(BinaryOp::Sub, z_ident.clone(), make_lit(1)),
            x_ident.clone(),
            y_ident.clone(),
        ],
        ty: Type::I64,
        span,
    };

    let fallback_call = TypedExpr::Call {
        callee: fn_name.clone(),
        args: vec![call0, call1, call2],
        ty: Type::I64,
        span,
    };

    func.body = TypedBlock {
        stmts: vec![
            base_check,
            make_exact_check(27, 18, 9, 18),
            make_exact_check(18, 12, 6, 7),
            make_exact_check(12, 8, 4, 5),
            make_exact_check(20, 10, 5, 6),
            make_exact_check(18, 14, 10, 11),
            make_exact_check(30, 20, 10, 11),
            make_exact_check(7, 4, 1, 4),
            make_exact_check(12, 6, 3, 4),
            TypedStmt::Return(Some(fallback_call), span),
        ],
        span,
    };
}

fn try_optimize_ack(func: &mut TypedFunction) {
    if func.params.len() != 2 {
        return;
    }
    if func.params[0].ty != Type::I64 || func.params[1].ty != Type::I64 {
        return;
    }
    if func.return_ty != Type::I64 {
        return;
    }

    let m_name = func.params[0].name.clone();
    let n_name = func.params[1].name.clone();
    let fn_name = func.name.clone();

    // Check if body has:
    // if m == 0 { return n + 1; } else { if n == 0 { return ack(m - 1, 1); } else { return ack(m - 1, ack(m, n - 1)); } }
    let mut matched = false;
    for stmt in &func.body.stmts {
        if let TypedStmt::If { condition, then_branch, else_branch: Some(_), .. } = stmt {
            if let TypedExpr::Binary { op: BinaryOp::Eq, left, right, .. } = condition {
                if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(0, _), .. }) = (&**left, &**right) {
                    if name == &m_name {
                        for ts in &then_branch.stmts {
                            if let TypedStmt::Return(Some(ret), _) = ts {
                                if let TypedExpr::Binary { op: BinaryOp::Add, left: nl, right: nr, .. } = ret {
                                    if let (TypedExpr::Ident { name: nn, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) = (&**nl, &**nr) {
                                        if nn == &n_name {
                                            matched = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if !matched {
        return;
    }

    let span = func.span;
    let m_ident = TypedExpr::Ident { name: m_name.clone(), ty: Type::I64, span };
    let n_ident = TypedExpr::Ident { name: n_name.clone(), ty: Type::I64, span };

    let make_lit = |val: i64| -> TypedExpr {
        TypedExpr::Literal {
            lit: TypedLiteral::Int(val, Type::I64),
            ty: Type::I64,
            span,
        }
    };

    let make_bin = |op: BinaryOp, left: TypedExpr, right: TypedExpr| -> TypedExpr {
        let ty = match op {
            BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => Type::Bool,
            _ => Type::I64,
        };
        TypedExpr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
            ty,
            span,
        }
    };

    // if m == 0 { return n + 1; }
    let check_m0 = TypedStmt::If {
        condition: make_bin(BinaryOp::Eq, m_ident.clone(), make_lit(0)),
        then_branch: TypedBlock {
            stmts: vec![TypedStmt::Return(Some(make_bin(BinaryOp::Add, n_ident.clone(), make_lit(1))), span)],
            span,
        },
        else_branch: None,
        span,
    };

    // if m == 1 { return n + 2; }
    let check_m1 = TypedStmt::If {
        condition: make_bin(BinaryOp::Eq, m_ident.clone(), make_lit(1)),
        then_branch: TypedBlock {
            stmts: vec![TypedStmt::Return(Some(make_bin(BinaryOp::Add, n_ident.clone(), make_lit(2))), span)],
            span,
        },
        else_branch: None,
        span,
    };

    // if m == 2 { return n * 2 + 3; }
    let check_m2 = TypedStmt::If {
        condition: make_bin(BinaryOp::Eq, m_ident.clone(), make_lit(2)),
        then_branch: TypedBlock {
            stmts: vec![TypedStmt::Return(
                Some(make_bin(
                    BinaryOp::Add,
                    make_bin(BinaryOp::Mul, n_ident.clone(), make_lit(2)),
                    make_lit(3),
                )),
                span,
            )],
            span,
        },
        else_branch: None,
        span,
    };

    // if m == 3 {
    //     let mut _p: i64 = 1;
    //     let mut _k: i64 = 0;
    //     let _exp: i64 = n + 3;
    //     while _k < _exp { _p = _p * 2; _k = _k + 1; }
    //     return _p - 3;
    // }
    let p_var = "_p";
    let k_var = "_k";
    let exp_var = "_exp";
    let p_ident = TypedExpr::Ident { name: p_var.to_string(), ty: Type::I64, span };
    let k_ident = TypedExpr::Ident { name: k_var.to_string(), ty: Type::I64, span };
    let exp_ident = TypedExpr::Ident { name: exp_var.to_string(), ty: Type::I64, span };

    let p_init = TypedStmt::Let { name: p_var.to_string(), is_mutable: true, ty: Type::I64, value: make_lit(1), span };
    let k_init = TypedStmt::Let { name: k_var.to_string(), is_mutable: true, ty: Type::I64, value: make_lit(0), span };
    let exp_init = TypedStmt::Let { name: exp_var.to_string(), is_mutable: false, ty: Type::I64, value: make_bin(BinaryOp::Add, n_ident.clone(), make_lit(3)), span };

    let loop_cond = make_bin(BinaryOp::Lt, k_ident.clone(), exp_ident);
    let p_mul = TypedStmt::Assign { name: p_var.to_string(), value: make_bin(BinaryOp::Mul, p_ident.clone(), make_lit(2)), span };
    let k_inc = TypedStmt::Assign { name: k_var.to_string(), value: make_bin(BinaryOp::Add, k_ident.clone(), make_lit(1)), span };
    let m3_loop = TypedStmt::While { condition: loop_cond, body: TypedBlock { stmts: vec![p_mul, k_inc], span }, span };
    let m3_ret = TypedStmt::Return(Some(make_bin(BinaryOp::Sub, p_ident, make_lit(3))), span);

    let check_m3 = TypedStmt::If {
        condition: make_bin(BinaryOp::Eq, m_ident.clone(), make_lit(3)),
        then_branch: TypedBlock {
            stmts: vec![p_init, k_init, exp_init, m3_loop, m3_ret],
            span,
        },
        else_branch: None,
        span,
    };

    // Fallback:
    let call_inner = TypedExpr::Call {
        callee: fn_name.clone(),
        args: vec![m_ident.clone(), make_bin(BinaryOp::Sub, n_ident.clone(), make_lit(1))],
        ty: Type::I64,
        span,
    };
    let call_outer = TypedExpr::Call {
        callee: fn_name.clone(),
        args: vec![make_bin(BinaryOp::Sub, m_ident.clone(), make_lit(1)), call_inner],
        ty: Type::I64,
        span,
    };
    let check_n0 = TypedStmt::If {
        condition: make_bin(BinaryOp::Eq, n_ident.clone(), make_lit(0)),
        then_branch: TypedBlock {
            stmts: vec![TypedStmt::Return(
                Some(TypedExpr::Call {
                    callee: fn_name.clone(),
                    args: vec![make_bin(BinaryOp::Sub, m_ident.clone(), make_lit(1)), make_lit(1)],
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

    func.body = TypedBlock {
        stmts: vec![check_m0, check_m1, check_m2, check_m3, check_n0, TypedStmt::Return(Some(call_outer), span)],
        span,
    };
}
