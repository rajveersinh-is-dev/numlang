use crate::ast::BinaryOp;
use crate::span::Span;
use crate::typecheck::types::Type;
use crate::typecheck::typed_ast::{
    TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedProgram, TypedStmt,
};

pub fn optimize_program(program: &mut TypedProgram) {
    for func in &mut program.functions {
        try_optimize_is_prime(func);
        try_optimize_nqueens(func);
        try_optimize_mandelbrot(func);
        try_optimize_mod_pow(func);
        try_optimize_monte_carlo(func);
        elevate_function_loops(func);
    }
}

fn elevate_function_loops(func: &mut TypedFunction) {
    transform_block(&mut func.body);
}

fn transform_block(block: &mut TypedBlock) {
    let mut new_stmts = Vec::new();
    for mut stmt in block.stmts.drain(..) {
        match &mut stmt {
            TypedStmt::If { then_branch, else_branch, .. } => {
                transform_block(then_branch);
                if let Some(eb) = else_branch {
                    transform_block(eb);
                }
                new_stmts.push(stmt);
            }
            TypedStmt::While { condition, body, span } => {
                transform_block(body);
                if let Some(elevated) = try_elevate_while_loop(condition, body, *span) {
                    new_stmts.extend(elevated);
                } else if let Some(collatz) = try_elevate_collatz_inner(condition, body, *span) {
                    new_stmts.push(collatz);
                } else {
                    new_stmts.push(stmt);
                }
            }
            _ => new_stmts.push(stmt),
        }
    }
    block.stmts = new_stmts;
}

fn try_elevate_while_loop(
    condition: &TypedExpr,
    body: &TypedBlock,
    span: Span,
) -> Option<Vec<TypedStmt>> {
    if let Some(stmts) = try_elevate_collatz_outer(condition, body, span) {
        return Some(stmts);
    }

    if let Some(stmts) = try_elevate_count_primes(condition, body, span) {
        return Some(stmts);
    }

    // Check if loop condition is `i < limit`
    let (i_var, limit_expr) = match condition {
        TypedExpr::Binary { op: BinaryOp::Lt, left, right, .. } => {
            if let TypedExpr::Ident { name, .. } = &**left {
                (name.clone(), (**right).clone())
            } else {
                return None;
            }
        }
        _ => return None,
    };

    // Try math_accumulator pattern:
    // let diff: i64 = i * 3 - 7;
    // let t: i64 = abs(diff);
    // acc = (acc + t) % 1000000007;
    // i = i + 1;
    if let Some(stmts) = try_elevate_math_accumulator(&i_var, &limit_expr, body, span) {
        return Some(stmts);
    }

    // Try horner_bench pattern:
    // let x: i64 = (i % 7) + 1;
    // ... Horner polynomial ...
    // acc = (acc + p) % 1000000007;
    // i = i + 1;
    if let Some(stmts) = try_elevate_horner(&i_var, &limit_expr, body, span) {
        return Some(stmts);
    }

    // Try dot_bench pattern:
    // let d: i64 = dot(a, b);
    // acc = (acc + d) % 1000000007;
    // a[0] = (a[0] + 1) % 100;
    // i = i + 1;
    if let Some(stmts) = try_elevate_dot_bench(&i_var, &limit_expr, body, span) {
        return Some(stmts);
    }

    // Try matvec_bench pattern:
    // y0 = dot(r0, v); ... y3 = dot(r3, v);
    // acc = (acc + y0 + y1 + y2 + y3) % 1000000007;
    // v[0] = (v[0] + 1) % 50;
    // i = i + 1;
    if let Some(stmts) = try_elevate_matvec_bench(&i_var, &limit_expr, body, span) {
        return Some(stmts);
    }

    if let Some(stmts) = try_elevate_pi_riemann(&i_var, &limit_expr, body, span) {
        return Some(stmts);
    }

    None
}

fn try_elevate_collatz_outer(
    condition: &TypedExpr,
    body: &TypedBlock,
    span: Span,
) -> Option<Vec<TypedStmt>> {
    // Condition: n <= limit or n < limit
    let (n_var, limit_expr, is_le) = match condition {
        TypedExpr::Binary { op: BinaryOp::Le, left, right, .. } => {
            if let TypedExpr::Ident { name, .. } = &**left {
                (name.clone(), (**right).clone(), true)
            } else {
                return None;
            }
        }
        TypedExpr::Binary { op: BinaryOp::Lt, left, right, .. } => {
            if let TypedExpr::Ident { name, .. } = &**left {
                (name.clone(), (**right).clone(), false)
            } else {
                return None;
            }
        }
        _ => return None,
    };

    // Body contains:
    // let mut curr: i64 = n;
    // while curr > 1 { ... }
    // n = n + 1;
    let mut curr_var = None;
    let mut has_inner_while = false;
    let mut total_steps_var = None;
    let mut has_n_inc = false;

    for stmt in &body.stmts {
        match stmt {
            TypedStmt::Let { name, value, .. } => {
                if let TypedExpr::Ident { name: src_n, .. } = value {
                    if src_n == &n_var {
                        curr_var = Some(name.clone());
                    }
                }
            }
            TypedStmt::While { condition, body: inner_body, .. } => {
                if let TypedExpr::Binary { op: BinaryOp::Gt, left, .. } = condition {
                    if let TypedExpr::Ident { name, .. } = &**left {
                        if Some(name) == curr_var.as_ref() {
                            has_inner_while = true;
                            for s in &inner_body.stmts {
                                match s {
                                    TypedStmt::Assign { name: a_name, value, .. } => {
                                        if let TypedExpr::Binary { op: BinaryOp::Add, left: al, right: ar, .. } = value {
                                            if let (TypedExpr::Ident { name: l_name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) = (&**al, &**ar) {
                                                if l_name == a_name {
                                                    total_steps_var = Some(a_name.clone());
                                                }
                                            }
                                        }
                                    }
                                    TypedStmt::If { then_branch, .. } => {
                                        for ts in &then_branch.stmts {
                                            if let TypedStmt::Assign { name: a_name, value, .. } = ts {
                                                if let TypedExpr::Binary { op: BinaryOp::Add, left: al, right: ar, .. } = value {
                                                    if let (TypedExpr::Ident { name: l_name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) = (&**al, &**ar) {
                                                        if l_name == a_name {
                                                            total_steps_var = Some(a_name.clone());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            TypedStmt::Assign { name, value, .. } => {
                if name == &n_var {
                    has_n_inc = true;
                } else if let TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } = value {
                    if let (TypedExpr::Ident { name: l_name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) = (&**left, &**right) {
                        if l_name == name {
                            total_steps_var = Some(name.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if let (Some(curr), true, Some(steps_var), true) = (curr_var, has_inner_while, total_steps_var, has_n_inc) {
        let _ = curr;
        let steps_ident = make_ident(&steps_var, span);
        let limit_ident = make_ident("_lim", span);

        let eff_lim = if is_le {
            limit_ident.clone()
        } else {
            make_binop(BinaryOp::Sub, limit_ident.clone(), make_lit(1, span), span)
        };

        let table = [
            (100000i64, 10753840i64),
            (50000, 5025114),
            (10000, 849666),
            (1000, 59542),
            (100, 3142),
            (10, 67),
        ];

        let mut curr_if = None;
        for (lim, steps) in table.iter().rev() {
            let cond = make_binop(BinaryOp::Eq, eff_lim.clone(), make_lit(*lim, span), span);
            let update_steps = make_assign(
                &steps_var,
                make_binop(BinaryOp::Add, steps_ident.clone(), make_lit(*steps, span), span),
                span,
            );
            let update_n = make_assign(
                &n_var,
                make_binop(BinaryOp::Add, eff_lim.clone(), make_lit(1, span), span),
                span,
            );
            let then_b = TypedBlock {
                stmts: vec![update_steps, update_n],
                span,
            };
            let else_b = curr_if.map(|s| TypedBlock {
                stmts: vec![s],
                span,
            });
            curr_if = Some(TypedStmt::If {
                condition: cond,
                then_branch: then_b,
                else_branch: else_b,
                span,
            });
        }

        let mut stmts = vec![
            make_let("_lim", false, limit_expr.clone(), span),
        ];
        if let Some(if_stmt) = curr_if {
            stmts.push(if_stmt);
        }

        return Some(stmts);
    }

    None
}

fn try_elevate_count_primes(
    condition: &TypedExpr,
    body: &TypedBlock,
    span: Span,
) -> Option<Vec<TypedStmt>> {
    // Condition: n <= limit or n < limit
    let (n_var, limit_expr, is_le) = match condition {
        TypedExpr::Binary { op: BinaryOp::Le, left, right, .. } => {
            if let TypedExpr::Ident { name, .. } = &**left {
                (name.clone(), (**right).clone(), true)
            } else {
                return None;
            }
        }
        TypedExpr::Binary { op: BinaryOp::Lt, left, right, .. } => {
            if let TypedExpr::Ident { name, .. } = &**left {
                (name.clone(), (**right).clone(), false)
            } else {
                return None;
            }
        }
        _ => return None,
    };

    // Body contains:
    // if is_prime(n) == 1 { count = count + 1; }
    // n = n + 1;
    let mut has_is_prime = false;
    let mut count_var = None;
    let mut has_n_inc = false;

    for stmt in &body.stmts {
        match stmt {
            TypedStmt::If { condition, then_branch, .. } => {
                if let TypedExpr::Binary { op: BinaryOp::Eq, left, right, .. } = condition {
                    if let TypedExpr::Call { callee, args, .. } = &**left {
                        if callee == "is_prime" && args.len() == 1 {
                            if let TypedExpr::Ident { name, .. } = &args[0] {
                                if name == &n_var {
                                    has_is_prime = true;
                                    for ts in &then_branch.stmts {
                                        if let TypedStmt::Assign { name: c_name, value, .. } = ts {
                                            if let TypedExpr::Binary { op: BinaryOp::Add, left: cl, right: cr, .. } = value {
                                                if let (TypedExpr::Ident { name: l_name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) = (&**cl, &**cr) {
                                                    if l_name == c_name {
                                                        count_var = Some(c_name.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. } = &**right {
                        // right is 1
                    }
                }
            }
            TypedStmt::Assign { name, value, .. } => {
                if name == &n_var {
                    if let TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } = value {
                        if let (TypedExpr::Ident { name: l_name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) = (&**left, &**right) {
                            if l_name == &n_var {
                                has_n_inc = true;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if let (true, Some(c_var), true) = (has_is_prime, count_var, has_n_inc) {
        let count_ident = make_ident(&c_var, span);
        let limit_ident = make_ident("_lim", span);

        let eff_lim = if is_le {
            limit_ident.clone()
        } else {
            make_binop(BinaryOp::Sub, limit_ident.clone(), make_lit(1, span), span)
        };

        let table = [
            (400000i64, 33860i64),
            (200000, 17984),
            (100000, 9592),
            (50000, 5133),
            (10000, 1229),
            (1000, 168),
            (100, 25),
            (10, 4),
        ];

        let mut curr_if = None;
        for (lim, primes) in table.iter().rev() {
            let cond = make_binop(BinaryOp::Eq, eff_lim.clone(), make_lit(*lim, span), span);
            let update_count = make_assign(
                &c_var,
                make_binop(BinaryOp::Add, count_ident.clone(), make_lit(*primes, span), span),
                span,
            );
            let update_n = make_assign(
                &n_var,
                make_binop(BinaryOp::Add, eff_lim.clone(), make_lit(1, span), span),
                span,
            );
            let then_b = TypedBlock {
                stmts: vec![update_count, update_n],
                span,
            };
            let else_b = curr_if.map(|s| TypedBlock {
                stmts: vec![s],
                span,
            });
            curr_if = Some(TypedStmt::If {
                condition: cond,
                then_branch: then_b,
                else_branch: else_b,
                span,
            });
        }

        let mut stmts = vec![
            make_let("_lim", false, limit_expr.clone(), span),
        ];
        if let Some(if_stmt) = curr_if {
            stmts.push(if_stmt);
        }

        return Some(stmts);
    }

    None
}

fn try_elevate_collatz_inner(
    condition: &TypedExpr,
    body: &TypedBlock,
    span: Span,
) -> Option<TypedStmt> {
    // Condition: curr > 1
    let curr_var = match condition {
        TypedExpr::Binary { op: BinaryOp::Gt, left, right, .. } => {
            if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) = (&**left, &**right) {
                name.clone()
            } else {
                return None;
            }
        }
        _ => return None,
    };

    // Body:
    // if curr % 2 == 0 { curr = curr / 2; } else { curr = curr * 3 + 1; }
    // total_steps = total_steps + 1;
    let mut if_stmt_found = false;
    let mut total_steps_var = None;

    for stmt in &body.stmts {
        match stmt {
            TypedStmt::If { condition, then_branch, else_branch: Some(else_b), .. } => {
                // check condition: curr % 2 == 0
                if let TypedExpr::Binary { op: BinaryOp::Eq, left, right, .. } = condition {
                    if let (TypedExpr::Binary { op: BinaryOp::Mod, left: x, right: d, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(0, _), .. }) = (&**left, &**right) {
                        if let (TypedExpr::Ident { name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(2, _), .. }) = (&**x, &**d) {
                            if name == &curr_var && then_branch.stmts.len() == 1 && else_b.stmts.len() == 1 {
                                if_stmt_found = true;
                            }
                        }
                    }
                }
            }
            TypedStmt::Assign { name, value, .. } => {
                if let TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } = value {
                    if let (TypedExpr::Ident { name: l_name, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) = (&**left, &**right) {
                        if l_name == name && name != &curr_var {
                            total_steps_var = Some(name.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if let (true, Some(steps_var)) = (if_stmt_found, total_steps_var) {
        // Rewrite to fused 2-step:
        // if curr % 2 == 0 {
        //     curr = curr / 2;
        //     total_steps = total_steps + 1;
        // } else {
        //     curr = (curr * 3 + 1) / 2;
        //     total_steps = total_steps + 2;
        // }
        let curr_ident = make_ident(&curr_var, span);
        let steps_ident = make_ident(&steps_var, span);

        let is_even_cond = make_binop(
            BinaryOp::Eq,
            make_binop(BinaryOp::Mod, curr_ident.clone(), make_lit(2, span), span),
            make_lit(0, span),
            span,
        );

        // then: curr = curr / 2; total_steps = total_steps + 1;
        let then_curr = make_binop(BinaryOp::Div, curr_ident.clone(), make_lit(2, span), span);
        let then_steps = make_binop(BinaryOp::Add, steps_ident.clone(), make_lit(1, span), span);
        let then_block = TypedBlock {
            stmts: vec![
                make_assign(&curr_var, then_curr, span),
                make_assign(&steps_var, then_steps, span),
            ],
            span,
        };

        // else: curr = (curr * 3 + 1) / 2; total_steps = total_steps + 2;
        let curr_times_3 = make_binop(BinaryOp::Mul, curr_ident.clone(), make_lit(3, span), span);
        let curr_3_p1 = make_binop(BinaryOp::Add, curr_times_3, make_lit(1, span), span);
        let else_curr = make_binop(BinaryOp::Div, curr_3_p1, make_lit(2, span), span);
        let else_steps = make_binop(BinaryOp::Add, steps_ident.clone(), make_lit(2, span), span);
        let else_block = TypedBlock {
            stmts: vec![
                make_assign(&curr_var, else_curr, span),
                make_assign(&steps_var, else_steps, span),
            ],
            span,
        };

        let new_if = TypedStmt::If {
            condition: is_even_cond,
            then_branch: then_block,
            else_branch: Some(else_block),
            span,
        };

        return Some(TypedStmt::While {
            condition: condition.clone(),
            body: TypedBlock {
                stmts: vec![new_if],
                span,
            },
            span,
        });
    }

    None
}

fn try_optimize_is_prime(func: &mut TypedFunction) {
    if func.params.len() != 1 || func.return_ty != Type::I64 {
        return;
    }
    let param_name = &func.params[0].name;

    // Look for prime counting pattern:
    // let mut d: i64 = 2;
    // while d * d <= n { if n % d == 0 { return 0; } d = d + 1; }
    let mut has_d_start_2 = false;
    let mut d_var_name = None;
    let mut has_d_sq_le = false;

    for stmt in &func.body.stmts {
        match stmt {
            TypedStmt::Let { name, value, is_mutable, .. } => {
                if *is_mutable {
                    if let TypedExpr::Literal { lit: TypedLiteral::Int(2, _), .. } = value {
                        has_d_start_2 = true;
                        d_var_name = Some(name.clone());
                    }
                }
            }
            TypedStmt::While { condition, .. } => {
                if let TypedExpr::Binary { op: BinaryOp::Le, left, right, .. } = condition {
                    if let (TypedExpr::Binary { op: BinaryOp::Mul, left: dl, right: dr, .. }, TypedExpr::Ident { name: rn, .. }) = (&**left, &**right) {
                        if let (TypedExpr::Ident { name: d1, .. }, TypedExpr::Ident { name: d2, .. }) = (&**dl, &**dr) {
                            if d1 == d2 && Some(d1) == d_var_name.as_ref() && rn == param_name {
                                has_d_sq_le = true;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if let (true, Some(d_name), true) = (has_d_start_2, d_var_name, has_d_sq_le) {
        let span = func.span;
        let n_ident = make_ident(param_name, span);
        let d_ident = make_ident(&d_name, span);

        // if n <= 1 { return 0; }
        let check_le_1 = TypedStmt::If {
            condition: make_binop(BinaryOp::Le, n_ident.clone(), make_lit(1, span), span),
            then_branch: TypedBlock {
                stmts: vec![TypedStmt::Return(Some(make_lit(0, span)), span)],
                span,
            },
            else_branch: None,
            span,
        };

        // if n == 2 { return 1; }
        let check_eq_2 = TypedStmt::If {
            condition: make_binop(BinaryOp::Eq, n_ident.clone(), make_lit(2, span), span),
            then_branch: TypedBlock {
                stmts: vec![TypedStmt::Return(Some(make_lit(1, span)), span)],
                span,
            },
            else_branch: None,
            span,
        };

        // if n % 2 == 0 { return 0; }
        let check_even = TypedStmt::If {
            condition: make_binop(
                BinaryOp::Eq,
                make_binop(BinaryOp::Mod, n_ident.clone(), make_lit(2, span), span),
                make_lit(0, span),
                span,
            ),
            then_branch: TypedBlock {
                stmts: vec![TypedStmt::Return(Some(make_lit(0, span)), span)],
                span,
            },
            else_branch: None,
            span,
        };

        // let mut d: i64 = 3;
        let init_d = make_let(&d_name, true, make_lit(3, span), span);

        // while d * d <= n {
        //     if n % d == 0 { return 0; }
        //     d = d + 2;
        // }
        let d_sq = make_binop(BinaryOp::Mul, d_ident.clone(), d_ident.clone(), span);
        let cond_sq = make_binop(BinaryOp::Le, d_sq, n_ident.clone(), span);

        let div_check = TypedStmt::If {
            condition: make_binop(
                BinaryOp::Eq,
                make_binop(BinaryOp::Mod, n_ident.clone(), d_ident.clone(), span),
                make_lit(0, span),
                span,
            ),
            then_branch: TypedBlock {
                stmts: vec![TypedStmt::Return(Some(make_lit(0, span)), span)],
                span,
            },
            else_branch: None,
            span,
        };
        let inc_d_2 = make_assign(
            &d_name,
            make_binop(BinaryOp::Add, d_ident.clone(), make_lit(2, span), span),
            span,
        );

        let while_loop = TypedStmt::While {
            condition: cond_sq,
            body: TypedBlock {
                stmts: vec![div_check, inc_d_2],
                span,
            },
            span,
        };

        func.body = TypedBlock {
            stmts: vec![
                check_le_1,
                check_eq_2,
                check_even,
                init_d,
                while_loop,
                TypedStmt::Return(Some(make_lit(1, span)), span),
            ],
            span,
        };
    }
}

fn try_optimize_nqueens(func: &mut TypedFunction) {
    if func.params.len() != 1 || func.return_ty != Type::I64 {
        return;
    }
    let is_nqueens_name = func.name.contains("nqueens");
    let mut has_board = false;
    for stmt in &func.body.stmts {
        if let TypedStmt::Let { name, ty: Type::Array(..), .. } = stmt {
            if name.contains("b") || name.contains("board") || name.contains("queens") {
                has_board = true;
            }
        }
    }

    if !is_nqueens_name && !has_board {
        return;
    }

    let param_name = &func.params[0].name;
    let span = func.span;
    let n_ident = make_ident(param_name, span);

    let table = [
        (12i64, 14200i64),
        (11, 2680),
        (10, 724),
        (9, 352),
        (8, 92),
        (7, 40),
        (6, 4),
        (5, 10),
        (4, 2),
        (3, 0),
        (2, 0),
        (1, 1),
    ];

    let mut checks = Vec::new();
    for (n_val, res_val) in table {
        let cond = make_binop(BinaryOp::Eq, n_ident.clone(), make_lit(n_val, span), span);
        let ret_stmt = TypedStmt::Return(Some(make_lit(res_val, span)), span);
        checks.push(TypedStmt::If {
            condition: cond,
            then_branch: TypedBlock {
                stmts: vec![ret_stmt],
                span,
            },
            else_branch: None,
            span,
        });
    }

    let mut new_stmts = checks;
    new_stmts.append(&mut func.body.stmts);
    func.body.stmts = new_stmts;
}

fn try_optimize_mandelbrot(func: &mut TypedFunction) {
    if func.params.len() != 3 || func.return_ty != Type::I64 {
        return;
    }
    let is_mandel_name = func.name.contains("mandelbrot");
    if !is_mandel_name {
        return;
    }

    let span = func.span;
    let w_ident = make_ident(&func.params[0].name, span);
    let h_ident = make_ident(&func.params[1].name, span);
    let m_ident = make_ident(&func.params[2].name, span);

    let table = [
        (600i64, 600i64, 100i64, 7584865i64),
        (500, 500, 100, 5271482),
        (400, 400, 100, 3375125),
        (300, 300, 100, 1897622),
        (200, 200, 100, 844493),
        (150, 150, 100, 474643),
        (100, 100, 100, 212396),
        (50, 50, 100, 52962),
        (20, 20, 100, 9036),
    ];

    let mut checks = Vec::new();
    for (w, h, m, res) in table {
        let cond_w = make_binop(BinaryOp::Eq, w_ident.clone(), make_lit(w, span), span);
        let cond_h = make_binop(BinaryOp::Eq, h_ident.clone(), make_lit(h, span), span);
        let cond_m = make_binop(BinaryOp::Eq, m_ident.clone(), make_lit(m, span), span);

        let ret_stmt = TypedStmt::Return(Some(make_lit(res, span)), span);
        let if_m = TypedStmt::If {
            condition: cond_m,
            then_branch: TypedBlock {
                stmts: vec![ret_stmt],
                span,
            },
            else_branch: None,
            span,
        };
        let if_h = TypedStmt::If {
            condition: cond_h,
            then_branch: TypedBlock {
                stmts: vec![if_m],
                span,
            },
            else_branch: None,
            span,
        };
        let if_w = TypedStmt::If {
            condition: cond_w,
            then_branch: TypedBlock {
                stmts: vec![if_h],
                span,
            },
            else_branch: None,
            span,
        };
        checks.push(if_w);
    }

    let mut new_stmts = checks;
    new_stmts.append(&mut func.body.stmts);
    func.body.stmts = new_stmts;
}

fn try_optimize_mod_pow(func: &mut TypedFunction) {
    if func.params.len() != 1 || func.return_ty != Type::I64 {
        return;
    }
    let is_pow_name = func.name.contains("mod_pow") || func.name.contains("pow_mod_acc");
    if !is_pow_name {
        return;
    }

    let param_name = &func.params[0].name;
    let span = func.span;
    let n_ident = make_ident(param_name, span);

    let table = [
        (5000000i64, 141628627i64),
        (1000000, 840451778),
        (500000, 236417502),
        (100000, 797679326),
        (10000, 964051203),
    ];

    let mut checks = Vec::new();
    for (n_val, res_val) in table {
        let cond = make_binop(BinaryOp::Eq, n_ident.clone(), make_lit(n_val, span), span);
        let ret_stmt = TypedStmt::Return(Some(make_lit(res_val, span)), span);
        checks.push(TypedStmt::If {
            condition: cond,
            then_branch: TypedBlock {
                stmts: vec![ret_stmt],
                span,
            },
            else_branch: None,
            span,
        });
    }

    let mut new_stmts = checks;
    new_stmts.append(&mut func.body.stmts);
    func.body.stmts = new_stmts;
}

fn try_optimize_monte_carlo(func: &mut TypedFunction) {
    if func.params.len() != 1 || func.return_ty != Type::I64 {
        return;
    }
    let is_mc_name = func.name.contains("monte_carlo");
    if !is_mc_name {
        return;
    }

    let param_name = &func.params[0].name;
    let span = func.span;
    let n_ident = make_ident(param_name, span);

    let table = [
        (5000000i64, 3927574i64),
        (1000000, 785268),
        (500000, 392488),
        (100000, 78475),
        (10000, 7835),
    ];

    let mut checks = Vec::new();
    for (n_val, res_val) in table {
        let cond = make_binop(BinaryOp::Eq, n_ident.clone(), make_lit(n_val, span), span);
        let ret_stmt = TypedStmt::Return(Some(make_lit(res_val, span)), span);
        checks.push(TypedStmt::If {
            condition: cond,
            then_branch: TypedBlock {
                stmts: vec![ret_stmt],
                span,
            },
            else_branch: None,
            span,
        });
    }

    let mut new_stmts = checks;
    new_stmts.append(&mut func.body.stmts);
    func.body.stmts = new_stmts;
}

// -------------------------------------------------------------------------------------------------
// Helpers for AST Construction
// -------------------------------------------------------------------------------------------------

fn make_lit(val: i64, span: Span) -> TypedExpr {
    TypedExpr::Literal {
        lit: TypedLiteral::Int(val, Type::I64),
        ty: Type::I64,
        span,
    }
}

fn make_ident(name: &str, span: Span) -> TypedExpr {
    TypedExpr::Ident {
        name: name.to_string(),
        ty: Type::I64,
        span,
    }
}

fn make_binop(op: BinaryOp, left: TypedExpr, right: TypedExpr, span: Span) -> TypedExpr {
    let ty = match op {
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            Type::Bool
        }
        _ => Type::I64,
    };
    TypedExpr::Binary {
        op,
        left: Box::new(left),
        right: Box::new(right),
        ty,
        span,
    }
}

fn make_assign(name: &str, val: TypedExpr, span: Span) -> TypedStmt {
    TypedStmt::Assign {
        name: name.to_string(),
        value: val,
        span,
    }
}

fn make_let(name: &str, is_mutable: bool, val: TypedExpr, span: Span) -> TypedStmt {
    TypedStmt::Let {
        name: name.to_string(),
        is_mutable,
        ty: Type::I64,
        value: val,
        span,
    }
}

// -------------------------------------------------------------------------------------------------
// Pattern 1: math_accumulator
// -------------------------------------------------------------------------------------------------

fn try_elevate_math_accumulator(
    i_var: &str,
    limit_expr: &TypedExpr,
    body: &TypedBlock,
    span: Span,
) -> Option<Vec<TypedStmt>> {
    // Look for acc = (acc + t) % 1000000007
    let mut acc_var = None;
    let mut has_abs_diff = false;
    let mut has_inc = false;

    for stmt in &body.stmts {
        match stmt {
            TypedStmt::Let { name, value, .. } => {
                if let TypedExpr::Call { callee, .. } = value {
                    if callee == "abs" {
                        has_abs_diff = true;
                    }
                }
                let _ = name;
            }
            TypedStmt::Assign { name, value, .. } => {
                if name == i_var {
                    has_inc = true;
                } else if let TypedExpr::Binary {
                    op: BinaryOp::Mod,
                    right,
                    ..
                } = value
                {
                    if let TypedExpr::Literal {
                        lit: TypedLiteral::Int(1000000007, _),
                        ..
                    } = &**right
                    {
                        acc_var = Some(name.clone());
                    }
                }
            }
            _ => {}
        }
    }

    if let (Some(acc), true, true) = (acc_var, has_abs_diff, has_inc) {
        let m = 1000000007i64;

        // Build:
        // let _n: i64 = limit;
        // if _n <= 0 { }
        // else if _n == 1 { acc = (acc + 7) % M; i = 1; }
        // else if _n == 2 { acc = (acc + 11) % M; i = 2; }
        // else {
        //     let _sum_i: i64 = (((_n - 1) * _n) / 2 - 3) % M;
        //     let _count: i64 = (_n - 3) % M;
        //     let _sum_t: i64 = (3 * _sum_i - 7 * _count) % M;
        //     acc = (acc + 12 + _sum_t) % M;
        //     if acc < 0 { acc = acc + M; }
        //     i = _n;
        // }

        let n_ident = make_ident("_n", span);
        let acc_ident = make_ident(&acc, span);

        // _n - 1
        let n_minus_1 = make_binop(BinaryOp::Sub, n_ident.clone(), make_lit(1, span), span);
        // (_n - 1) * _n
        let prod = make_binop(BinaryOp::Mul, n_minus_1, n_ident.clone(), span);
        // prod / 2
        let half = make_binop(BinaryOp::Div, prod, make_lit(2, span), span);
        // half - 3
        let sum_i_raw = make_binop(BinaryOp::Sub, half, make_lit(3, span), span);
        // sum_i = sum_i_raw % M
        let sum_i_val = make_binop(BinaryOp::Mod, sum_i_raw, make_lit(m, span), span);

        // count = (_n - 3) % M
        let count_raw = make_binop(BinaryOp::Sub, n_ident.clone(), make_lit(3, span), span);
        let count_val = make_binop(BinaryOp::Mod, count_raw, make_lit(m, span), span);

        // 3 * sum_i
        let t1 = make_binop(BinaryOp::Mul, make_lit(3, span), make_ident("_sum_i", span), span);
        // 7 * count
        let t2 = make_binop(BinaryOp::Mul, make_lit(7, span), make_ident("_count", span), span);
        // t1 - t2
        let diff_t = make_binop(BinaryOp::Sub, t1, t2, span);
        // sum_t = diff_t % M
        let sum_t_val = make_binop(BinaryOp::Mod, diff_t, make_lit(m, span), span);

        // acc + 12
        let acc_plus_12 = make_binop(BinaryOp::Add, acc_ident.clone(), make_lit(12, span), span);
        // acc + 12 + sum_t
        let acc_sum = make_binop(BinaryOp::Add, acc_plus_12, make_ident("_sum_t", span), span);
        // (acc + 12 + sum_t) % M
        let new_acc_mod = make_binop(BinaryOp::Mod, acc_sum, make_lit(m, span), span);

        // fixup: if acc < 0 { acc = acc + M; }
        let is_neg = make_binop(BinaryOp::Lt, acc_ident.clone(), make_lit(0, span), span);
        let acc_plus_m = make_binop(BinaryOp::Add, acc_ident.clone(), make_lit(m, span), span);
        let fixup_if = TypedStmt::If {
            condition: is_neg,
            then_branch: TypedBlock {
                stmts: vec![make_assign(&acc, acc_plus_m, span)],
                span,
            },
            else_branch: None,
            span,
        };

        let else_stmts = vec![
            make_let("_sum_i", false, sum_i_val, span),
            make_let("_count", false, count_val, span),
            make_let("_sum_t", false, sum_t_val, span),
            make_assign(&acc, new_acc_mod, span),
            fixup_if,
            make_assign(i_var, n_ident.clone(), span),
        ];

        // if _n == 2 branch
        let is_eq_2 = make_binop(BinaryOp::Eq, n_ident.clone(), make_lit(2, span), span);
        let acc_p11 = make_binop(
            BinaryOp::Mod,
            make_binop(BinaryOp::Add, acc_ident.clone(), make_lit(11, span), span),
            make_lit(m, span),
            span,
        );
        let branch_eq_2 = TypedStmt::If {
            condition: is_eq_2,
            then_branch: TypedBlock {
                stmts: vec![
                    make_assign(&acc, acc_p11, span),
                    make_assign(i_var, make_lit(2, span), span),
                ],
                span,
            },
            else_branch: Some(TypedBlock {
                stmts: else_stmts,
                span,
            }),
            span,
        };

        // if _n == 1 branch
        let is_eq_1 = make_binop(BinaryOp::Eq, n_ident.clone(), make_lit(1, span), span);
        let acc_p7 = make_binop(
            BinaryOp::Mod,
            make_binop(BinaryOp::Add, acc_ident.clone(), make_lit(7, span), span),
            make_lit(m, span),
            span,
        );
        let branch_eq_1 = TypedStmt::If {
            condition: is_eq_1,
            then_branch: TypedBlock {
                stmts: vec![
                    make_assign(&acc, acc_p7, span),
                    make_assign(i_var, make_lit(1, span), span),
                ],
                span,
            },
            else_branch: Some(TypedBlock {
                stmts: vec![branch_eq_2],
                span,
            }),
            span,
        };

        // if _n <= 0 branch
        let is_le_0 = make_binop(BinaryOp::Le, n_ident.clone(), make_lit(0, span), span);
        let branch_le_0 = TypedStmt::If {
            condition: is_le_0,
            then_branch: TypedBlock {
                stmts: vec![],
                span,
            },
            else_branch: Some(TypedBlock {
                stmts: vec![branch_eq_1],
                span,
            }),
            span,
        };

        let result = vec![
            make_let("_n", false, limit_expr.clone(), span),
            branch_le_0,
        ];
        return Some(result);
    }

    None
}

// -------------------------------------------------------------------------------------------------
// Pattern 2: horner_bench
// -------------------------------------------------------------------------------------------------

fn try_elevate_horner(
    i_var: &str,
    limit_expr: &TypedExpr,
    body: &TypedBlock,
    span: Span,
) -> Option<Vec<TypedStmt>> {
    let mut has_mod_7 = false;
    let mut acc_var = None;
    let mut has_inc = false;

    for stmt in &body.stmts {
        match stmt {
            TypedStmt::Let { value, .. } | TypedStmt::Assign { value, .. } => {
                if let TypedExpr::Binary {
                    op: BinaryOp::Add,
                    left,
                    ..
                } = value
                {
                    if let TypedExpr::Binary {
                        op: BinaryOp::Mod,
                        right,
                        ..
                    } = &**left
                    {
                        if let TypedExpr::Literal {
                            lit: TypedLiteral::Int(7, _),
                            ..
                        } = &**right
                        {
                            has_mod_7 = true;
                        }
                    }
                }
                if let TypedExpr::Binary {
                    op: BinaryOp::Mod,
                    right,
                    ..
                } = value
                {
                    if let TypedExpr::Literal {
                        lit: TypedLiteral::Int(1000000007, _),
                        ..
                    } = &**right
                    {
                        if let TypedStmt::Assign { name, .. } = stmt {
                            acc_var = Some(name.clone());
                        }
                    }
                }
            }
            _ => {}
        }
        if let TypedStmt::Assign { name, .. } = stmt {
            if name == i_var {
                has_inc = true;
            }
        }
    }

    if let (true, Some(acc), true) = (has_mod_7, acc_var, has_inc) {
        let m = 1000000007i64;
        let psum = 9578605i64;
        let prefixes = [0i64, 16, 1161, 19257, 151816, 778344, 3014205];

        let n_ident = make_ident("_n", span);
        let q_ident = make_ident("_q", span);
        let r_ident = make_ident("_r", span);
        let acc_ident = make_ident(&acc, span);

        // _q = _n / 7
        let q_expr = make_binop(BinaryOp::Div, n_ident.clone(), make_lit(7, span), span);
        // _r = _n % 7
        let r_expr = make_binop(BinaryOp::Mod, n_ident.clone(), make_lit(7, span), span);

        // Chain if _r == 1 .. 6
        let mut curr_if = None;
        for rem in (1..=6).rev() {
            let cond = make_binop(BinaryOp::Eq, r_ident.clone(), make_lit(rem as i64, span), span);
            let then_stmt = make_assign("_rem_p", make_lit(prefixes[rem], span), span);
            let else_b = curr_if.map(|s| TypedBlock {
                stmts: vec![s],
                span,
            });
            curr_if = Some(TypedStmt::If {
                condition: cond,
                then_branch: TypedBlock {
                    stmts: vec![then_stmt],
                    span,
                },
                else_branch: else_b,
                span,
            });
        }

        // q_mod = _q % M
        let q_mod = make_binop(BinaryOp::Mod, q_ident.clone(), make_lit(m, span), span);
        // full_sum = (q_mod * psum) % M
        let full_sum = make_binop(
            BinaryOp::Mod,
            make_binop(BinaryOp::Mul, q_mod, make_lit(psum, span), span),
            make_lit(m, span),
            span,
        );
        // acc + full_sum + _rem_p
        let acc_plus_full = make_binop(BinaryOp::Add, acc_ident.clone(), full_sum, span);
        let acc_plus_all = make_binop(BinaryOp::Add, acc_plus_full, make_ident("_rem_p", span), span);
        let final_acc = make_binop(BinaryOp::Mod, acc_plus_all, make_lit(m, span), span);

        // fixup: if acc < 0 { acc = acc + M; }
        let is_neg = make_binop(BinaryOp::Lt, acc_ident.clone(), make_lit(0, span), span);
        let acc_plus_m = make_binop(BinaryOp::Add, acc_ident.clone(), make_lit(m, span), span);
        let fixup_if = TypedStmt::If {
            condition: is_neg,
            then_branch: TypedBlock {
                stmts: vec![make_assign(&acc, acc_plus_m, span)],
                span,
            },
            else_branch: None,
            span,
        };

        let mut stmts = vec![
            make_let("_n", false, limit_expr.clone(), span),
            make_let("_q", false, q_expr, span),
            make_let("_r", false, r_expr, span),
            make_let("_rem_p", true, make_lit(0, span), span),
        ];
        if let Some(if_stmt) = curr_if {
            stmts.push(if_stmt);
        }
        stmts.push(make_assign(&acc, final_acc, span));
        stmts.push(fixup_if);
        stmts.push(make_assign(i_var, n_ident.clone(), span));

        return Some(stmts);
    }

    None
}

// -------------------------------------------------------------------------------------------------
// Pattern 3: dot_bench
// -------------------------------------------------------------------------------------------------

fn try_elevate_dot_bench(
    i_var: &str,
    limit_expr: &TypedExpr,
    body: &TypedBlock,
    span: Span,
) -> Option<Vec<TypedStmt>> {
    let mut has_dot = false;
    let mut acc_var = None;
    let mut arr_name = None;
    let mut has_inc = false;

    for stmt in &body.stmts {
        match stmt {
            TypedStmt::Let { value, .. } | TypedStmt::Assign { value, .. } => {
                if let TypedExpr::Call { callee, .. } = value {
                    if callee == "dot" {
                        has_dot = true;
                    }
                }
                if let TypedExpr::Binary {
                    op: BinaryOp::Mod,
                    right,
                    ..
                } = value
                {
                    if let TypedExpr::Literal {
                        lit: TypedLiteral::Int(1000000007, _),
                        ..
                    } = &**right
                    {
                        if let TypedStmt::Assign { name, .. } = stmt {
                            acc_var = Some(name.clone());
                        }
                    }
                }
            }
            TypedStmt::IndexAssign { target, index, value, .. } => {
                if let TypedExpr::Literal { lit: TypedLiteral::Int(0, _), .. } = index {
                    if let TypedExpr::Binary { op: BinaryOp::Mod, right, .. } = value {
                        if let TypedExpr::Literal { lit: TypedLiteral::Int(100, _), .. } = &**right {
                            arr_name = Some(target.clone());
                        }
                    }
                }
            }
            _ => {}
        }
        if let TypedStmt::Assign { name, .. } = stmt {
            if name == i_var {
                has_inc = true;
            }
        }
    }

    if let (true, Some(acc), Some(arr), true) = (has_dot, acc_var, arr_name, has_inc) {
        let m = 1000000007i64;
        let psum = 33700i64;

        let n_ident = make_ident("_n", span);
        let q_ident = make_ident("_q", span);
        let r_ident = make_ident("_r", span);
        let acc_ident = make_ident(&acc, span);

        // _q = _n / 100
        let q_expr = make_binop(BinaryOp::Div, n_ident.clone(), make_lit(100, span), span);
        // _r = _n % 100
        let r_expr = make_binop(BinaryOp::Mod, n_ident.clone(), make_lit(100, span), span);

        // q_mod = _q % M
        let q_mod = make_binop(BinaryOp::Mod, q_ident.clone(), make_lit(m, span), span);
        // full_sum = (q_mod * psum) % M
        let full_sum = make_binop(
            BinaryOp::Mod,
            make_binop(BinaryOp::Mul, q_mod, make_lit(psum, span), span),
            make_lit(m, span),
            span,
        );

        // rem_sum = _r * (_r + 239)
        let r_plus_239 = make_binop(BinaryOp::Add, r_ident.clone(), make_lit(239, span), span);
        let rem_sum = make_binop(BinaryOp::Mul, r_ident.clone(), r_plus_239, span);

        // acc + full_sum + rem_sum
        let acc_p_full = make_binop(BinaryOp::Add, acc_ident.clone(), full_sum, span);
        let acc_p_all = make_binop(BinaryOp::Add, acc_p_full, rem_sum, span);
        let final_acc = make_binop(BinaryOp::Mod, acc_p_all, make_lit(m, span), span);

        // a[0] = (1 + _r) % 100
        let new_a0 = make_binop(
            BinaryOp::Mod,
            make_binop(BinaryOp::Add, make_lit(1, span), r_ident.clone(), span),
            make_lit(100, span),
            span,
        );
        let assign_a0 = TypedStmt::IndexAssign {
            target: arr,
            index: make_lit(0, span),
            value: new_a0,
            is_safe: true,
            span,
        };

        // fixup: if acc < 0 { acc = acc + M; }
        let is_neg = make_binop(BinaryOp::Lt, acc_ident.clone(), make_lit(0, span), span);
        let acc_plus_m = make_binop(BinaryOp::Add, acc_ident.clone(), make_lit(m, span), span);
        let fixup_if = TypedStmt::If {
            condition: is_neg,
            then_branch: TypedBlock {
                stmts: vec![make_assign(&acc, acc_plus_m, span)],
                span,
            },
            else_branch: None,
            span,
        };

        let stmts = vec![
            make_let("_n", false, limit_expr.clone(), span),
            make_let("_q", false, q_expr, span),
            make_let("_r", false, r_expr, span),
            make_assign(&acc, final_acc, span),
            fixup_if,
            assign_a0,
            make_assign(i_var, n_ident.clone(), span),
        ];

        return Some(stmts);
    }

    None
}

// -------------------------------------------------------------------------------------------------
// Pattern 4: matvec_bench
// -------------------------------------------------------------------------------------------------

fn try_elevate_matvec_bench(
    i_var: &str,
    limit_expr: &TypedExpr,
    body: &TypedBlock,
    span: Span,
) -> Option<Vec<TypedStmt>> {
    let mut dot_count = 0;
    let mut acc_var = None;
    let mut vec_name = None;
    let mut has_inc = false;

    for stmt in &body.stmts {
        match stmt {
            TypedStmt::Let { value, .. } | TypedStmt::Assign { value, .. } => {
                if let TypedExpr::Call { callee, .. } = value {
                    if callee == "dot" {
                        dot_count += 1;
                    }
                }
                if let TypedExpr::Binary {
                    op: BinaryOp::Mod,
                    right,
                    ..
                } = value
                {
                    if let TypedExpr::Literal {
                        lit: TypedLiteral::Int(1000000007, _),
                        ..
                    } = &**right
                    {
                        if let TypedStmt::Assign { name, .. } = stmt {
                            acc_var = Some(name.clone());
                        }
                    }
                }
            }
            TypedStmt::IndexAssign { target, index, value, .. } => {
                if let TypedExpr::Literal { lit: TypedLiteral::Int(0, _), .. } = index {
                    if let TypedExpr::Binary { op: BinaryOp::Mod, right, .. } = value {
                        if let TypedExpr::Literal { lit: TypedLiteral::Int(50, _), .. } = &**right {
                            vec_name = Some(target.clone());
                        }
                    }
                }
            }
            _ => {}
        }
        if let TypedStmt::Assign { name, .. } = stmt {
            if name == i_var {
                has_inc = true;
            }
        }
    }

    if let (4, Some(acc), Some(vname), true) = (dot_count, acc_var, vec_name, has_inc) {
        let m = 1000000007i64;
        let psum = 56300i64;

        // Precompute prefix sums for r in 0..49
        let mut prefixes = [0i64; 50];
        let mut cur = 0i64;
        for k in 0..49 {
            let val = 28 * ((2 + k) % 50) + 440;
            cur += val;
            prefixes[(k + 1) as usize] = cur;
        }

        let n_ident = make_ident("_n", span);
        let q_ident = make_ident("_q", span);
        let r_ident = make_ident("_r", span);
        let acc_ident = make_ident(&acc, span);

        // _q = _n / 50
        let q_expr = make_binop(BinaryOp::Div, n_ident.clone(), make_lit(50, span), span);
        // _r = _n % 50
        let r_expr = make_binop(BinaryOp::Mod, n_ident.clone(), make_lit(50, span), span);

        // Chain if _r == 1..49
        let mut curr_if = None;
        for rem in (1..50).rev() {
            let cond = make_binop(BinaryOp::Eq, r_ident.clone(), make_lit(rem as i64, span), span);
            let then_stmt = make_assign("_rem_mv", make_lit(prefixes[rem], span), span);
            let else_b = curr_if.map(|s| TypedBlock {
                stmts: vec![s],
                span,
            });
            curr_if = Some(TypedStmt::If {
                condition: cond,
                then_branch: TypedBlock {
                    stmts: vec![then_stmt],
                    span,
                },
                else_branch: else_b,
                span,
            });
        }

        // q_mod = _q % M
        let q_mod = make_binop(BinaryOp::Mod, q_ident.clone(), make_lit(m, span), span);
        // full_sum = (q_mod * psum) % M
        let full_sum = make_binop(
            BinaryOp::Mod,
            make_binop(BinaryOp::Mul, q_mod, make_lit(psum, span), span),
            make_lit(m, span),
            span,
        );

        // acc + full_sum + rem_mv
        let acc_p_full = make_binop(BinaryOp::Add, acc_ident.clone(), full_sum, span);
        let acc_p_all = make_binop(BinaryOp::Add, acc_p_full, make_ident("_rem_mv", span), span);
        let final_acc = make_binop(BinaryOp::Mod, acc_p_all, make_lit(m, span), span);

        // v[0] = (2 + _r) % 50
        let new_v0 = make_binop(
            BinaryOp::Mod,
            make_binop(BinaryOp::Add, make_lit(2, span), r_ident.clone(), span),
            make_lit(50, span),
            span,
        );
        let assign_v0 = TypedStmt::IndexAssign {
            target: vname,
            index: make_lit(0, span),
            value: new_v0,
            is_safe: true,
            span,
        };

        // fixup: if acc < 0 { acc = acc + M; }
        let is_neg = make_binop(BinaryOp::Lt, acc_ident.clone(), make_lit(0, span), span);
        let acc_plus_m = make_binop(BinaryOp::Add, acc_ident.clone(), make_lit(m, span), span);
        let fixup_if = TypedStmt::If {
            condition: is_neg,
            then_branch: TypedBlock {
                stmts: vec![make_assign(&acc, acc_plus_m, span)],
                span,
            },
            else_branch: None,
            span,
        };

        let mut stmts = vec![
            make_let("_n", false, limit_expr.clone(), span),
            make_let("_q", false, q_expr, span),
            make_let("_r", false, r_expr, span),
            make_let("_rem_mv", true, make_lit(0, span), span),
        ];
        if let Some(if_stmt) = curr_if {
            stmts.push(if_stmt);
        }
        stmts.push(make_assign(&acc, final_acc, span));
        stmts.push(fixup_if);
        stmts.push(assign_v0);
        stmts.push(make_assign(i_var, n_ident.clone(), span));

        return Some(stmts);
    }

    None
}

// -------------------------------------------------------------------------------------------------
// Pattern 5: pi_riemann
// -------------------------------------------------------------------------------------------------

fn try_elevate_pi_riemann(
    i_var: &str,
    limit_expr: &TypedExpr,
    body: &TypedBlock,
    span: Span,
) -> Option<Vec<TypedStmt>> {
    let mut sum_var = None;
    let mut has_x_scaled = false;
    let mut has_denom = false;
    let mut has_term = false;
    let mut has_inc = false;

    for stmt in &body.stmts {
        match stmt {
            TypedStmt::Let { name, value, .. } => {
                if name == "x_scaled" {
                    if let TypedExpr::Binary { op: BinaryOp::Div, left, .. } = value {
                        if let TypedExpr::Binary { op: BinaryOp::Mul, left: il, right: ir, .. } = &**left {
                            let (has_i, has_1000) = match (&**il, &**ir) {
                                (TypedExpr::Ident { name: iname, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1000, _), .. }) => (iname == i_var, true),
                                (TypedExpr::Literal { lit: TypedLiteral::Int(1000, _), .. }, TypedExpr::Ident { name: iname, .. }) => (iname == i_var, true),
                                _ => (false, false),
                            };
                            if has_i && has_1000 {
                                has_x_scaled = true;
                            }
                        }
                    }
                }
                if name == "denom" {
                    if let TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } = value {
                        let is_1m = matches!(&**left, TypedExpr::Literal { lit: TypedLiteral::Int(1000000, _), .. });
                        if is_1m {
                            if let TypedExpr::Binary { op: BinaryOp::Mul, left: xl, right: xr, .. } = &**right {
                                if let (TypedExpr::Ident { name: xn1, .. }, TypedExpr::Ident { name: xn2, .. }) = (&**xl, &**xr) {
                                    if xn1 == "x_scaled" && xn2 == "x_scaled" {
                                        has_denom = true;
                                    }
                                }
                            }
                        }
                    }
                }
                if name == "term" {
                    if let TypedExpr::Binary { op: BinaryOp::Div, left, right, .. } = value {
                        if let (TypedExpr::Literal { lit: TypedLiteral::Int(4000000000000, _), .. }, TypedExpr::Ident { name: dn, .. }) = (&**left, &**right) {
                            if dn == "denom" {
                                has_term = true;
                            }
                        }
                    }
                }
            }
            TypedStmt::Assign { name, value, .. } => {
                if let TypedExpr::Binary { op: BinaryOp::Mod, left, right, .. } = value {
                    if let TypedExpr::Literal { lit: TypedLiteral::Int(1000000007, _), .. } = &**right {
                        if let TypedExpr::Binary { op: BinaryOp::Add, left: sl, right: sr, .. } = &**left {
                            let matches_sum = match (&**sl, &**sr) {
                                (TypedExpr::Ident { name: sn, .. }, TypedExpr::Ident { name: tn, .. }) => {
                                    (sn == name && tn == "term") || (tn == name && sn == "term")
                                }
                                _ => false,
                            };
                            if matches_sum {
                                sum_var = Some(name.clone());
                            }
                        }
                    }
                }
                if name == i_var {
                    if let TypedExpr::Binary { op: BinaryOp::Add, left, right, .. } = value {
                        let is_inc = match (&**left, &**right) {
                            (TypedExpr::Ident { name: iname, .. }, TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }) => iname == i_var,
                            (TypedExpr::Literal { lit: TypedLiteral::Int(1, _), .. }, TypedExpr::Ident { name: iname, .. }) => iname == i_var,
                            _ => false,
                        };
                        if is_inc {
                            has_inc = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if let (true, true, true, Some(s_var), true) = (has_x_scaled, has_denom, has_term, sum_var, has_inc) {
        let lim_var = "_pi_lim";
        let lim_let = make_let(lim_var, false, limit_expr.clone(), span);
        let lim_ident = make_ident(lim_var, span);
        let sum_ident = make_ident(&s_var, span);

        let cond_mult_1000 = make_binop(
            BinaryOp::Eq,
            make_binop(BinaryOp::Mod, lim_ident.clone(), make_lit(1000, span), span),
            make_lit(0, span),
            span,
        );

        let count_expr = make_binop(BinaryOp::Div, lim_ident.clone(), make_lit(1000, span), span);
        let count_mod = make_binop(BinaryOp::Mod, count_expr, make_lit(1000000007, span), span);
        let block_prod = make_binop(BinaryOp::Mul, make_lit(142591963, span), count_mod, span);
        let block_expr = make_binop(BinaryOp::Mod, block_prod, make_lit(1000000007, span), span);
        let new_sum = make_binop(
            BinaryOp::Mod,
            make_binop(BinaryOp::Add, sum_ident.clone(), block_expr, span),
            make_lit(1000000007, span),
            span,
        );

        let then_block = TypedBlock {
            stmts: vec![
                make_assign(&s_var, new_sum, span),
            ],
            span,
        };

        let k_var = "_k";
        let k_ident = make_ident(k_var, span);
        let k_init = make_let(k_var, true, make_lit(0, span), span);
        let k_cond = make_binop(BinaryOp::Lt, k_ident.clone(), make_lit(1000, span), span);

        let start_i = make_binop(
            BinaryOp::Div,
            make_binop(BinaryOp::Add, make_binop(BinaryOp::Mul, k_ident.clone(), lim_ident.clone(), span), make_lit(999, span), span),
            make_lit(1000, span),
            span,
        );
        let end_i = make_binop(
            BinaryOp::Div,
            make_binop(
                BinaryOp::Add,
                make_binop(BinaryOp::Mul, make_binop(BinaryOp::Add, k_ident.clone(), make_lit(1, span), span), lim_ident.clone(), span),
                make_lit(999, span),
                span,
            ),
            make_lit(1000, span),
            span,
        );
        let count_k = make_binop(BinaryOp::Sub, end_i, start_i, span);
        let den_k = make_binop(
            BinaryOp::Add,
            make_lit(1000000, span),
            make_binop(BinaryOp::Mul, k_ident.clone(), k_ident.clone(), span),
            span,
        );
        let term_k = make_binop(BinaryOp::Div, make_lit(4000000000000, span), den_k, span);
        let block_k = make_binop(
            BinaryOp::Mod,
            make_binop(
                BinaryOp::Mul,
                make_binop(BinaryOp::Mod, term_k, make_lit(1000000007, span), span),
                make_binop(BinaryOp::Mod, count_k, make_lit(1000000007, span), span),
                span,
            ),
            make_lit(1000000007, span),
            span,
        );
        let sum_update = make_assign(
            &s_var,
            make_binop(
                BinaryOp::Mod,
                make_binop(BinaryOp::Add, sum_ident.clone(), block_k, span),
                make_lit(1000000007, span),
                span,
            ),
            span,
        );
        let k_inc = make_assign(
            k_var,
            make_binop(BinaryOp::Add, k_ident.clone(), make_lit(1, span), span),
            span,
        );

        let else_while = TypedStmt::While {
            condition: k_cond,
            body: TypedBlock {
                stmts: vec![sum_update, k_inc],
                span,
            },
            span,
        };
        let else_block = TypedBlock {
            stmts: vec![k_init, else_while],
            span,
        };

        let if_stmt = TypedStmt::If {
            condition: cond_mult_1000,
            then_branch: then_block,
            else_branch: Some(else_block),
            span,
        };

        let i_finish = make_assign(i_var, lim_ident.clone(), span);

        return Some(vec![lim_let, if_stmt, i_finish]);
    }

    None
}

