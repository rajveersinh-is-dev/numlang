use numlang::ast::{BinaryOp, Expr, Literal, Stmt};
use numlang::parser::{parse, parse_expr_str};
use numlang::token::tokenize;

#[test]
fn test_pratt_operator_precedence() {
    let tokens = tokenize("1 + 2 * 3").unwrap();
    let expr = parse_expr_str(&tokens).unwrap();

    // Must be 1 + (2 * 3)
    match expr {
        Expr::Binary { op, left, right, .. } => {
            assert_eq!(op, BinaryOp::Add);
            assert_eq!(*left, Expr::Literal(Literal::Int(1), left.span()));
            match *right {
                Expr::Binary { op: op2, left: l2, right: r2, .. } => {
                    assert_eq!(op2, BinaryOp::Mul);
                    assert_eq!(*l2, Expr::Literal(Literal::Int(2), l2.span()));
                    assert_eq!(*r2, Expr::Literal(Literal::Int(3), r2.span()));
                }
                _ => panic!("Expected binary multiplication on RHS"),
            }
        }
        _ => panic!("Expected binary addition at root"),
    }
}

#[test]
fn test_pratt_grouped_precedence() {
    let tokens = tokenize("(1 + 2) * 3").unwrap();
    let expr = parse_expr_str(&tokens).unwrap();

    // Must be (1 + 2) * 3
    match expr {
        Expr::Binary { op, left, right, .. } => {
            assert_eq!(op, BinaryOp::Mul);
            match *left {
                Expr::Group(inner, _) => match *inner {
                    Expr::Binary { op: op_inner, .. } => {
                        assert_eq!(op_inner, BinaryOp::Add);
                    }
                    _ => panic!("Expected Add inside group"),
                },
                _ => panic!("Expected Group on LHS"),
            }
            assert_eq!(*right, Expr::Literal(Literal::Int(3), right.span()));
        }
        _ => panic!("Expected Mul at root"),
    }
}

#[test]
fn test_pratt_exponentiation_right_associativity() {
    let tokens = tokenize("2 ^ 3 ^ 2").unwrap();
    let expr = parse_expr_str(&tokens).unwrap();

    // 2 ^ (3 ^ 2) -> 2 ^ 9
    match expr {
        Expr::Binary { op, left, right, .. } => {
            assert_eq!(op, BinaryOp::Pow);
            assert_eq!(*left, Expr::Literal(Literal::Int(2), left.span()));
            match *right {
                Expr::Binary { op: op2, left: l2, right: r2, .. } => {
                    assert_eq!(op2, BinaryOp::Pow);
                    assert_eq!(*l2, Expr::Literal(Literal::Int(3), l2.span()));
                    assert_eq!(*r2, Expr::Literal(Literal::Int(2), r2.span()));
                }
                _ => panic!("Expected nested Pow on RHS"),
            }
        }
        _ => panic!("Expected Pow at root"),
    }
}

#[test]
fn test_parse_function_and_statements() {
    let src = r#"
        fn compute(a: f64, b: f64) -> f64 {
            let x: f64 = a + b;
            if x > 10.0 {
                return x * 2.0;
            } else {
                return x;
            }
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let program = parse(&tokens).unwrap();

    assert_eq!(program.functions.len(), 1);
    let func = &program.functions[0];
    assert_eq!(func.name, "compute");
    assert_eq!(func.params.len(), 2);
    assert_eq!(func.params[0].name, "a");
    assert_eq!(func.params[0].ty, "f64");
    assert_eq!(func.params[1].name, "b");
    assert_eq!(func.params[1].ty, "f64");
    assert_eq!(func.return_ty, Some("f64".to_string()));
    assert_eq!(func.body.stmts.len(), 2);

    match &func.body.stmts[0] {
        Stmt::Let { name, ty, .. } => {
            assert_eq!(name, "x");
            assert_eq!(ty.as_deref(), Some("f64"));
        }
        _ => panic!("Expected let statement"),
    }

    match &func.body.stmts[1] {
        Stmt::If { else_branch, .. } => {
            assert!(else_branch.is_some());
        }
        _ => panic!("Expected if statement"),
    }
}

#[test]
fn test_parse_mut_and_assignment() {
    let src = r#"
        fn mutate_counter() {
            let mut count: i64 = 0;
            count = count + 1;
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let program = parse(&tokens).unwrap();
    assert_eq!(program.functions.len(), 1);

    let func = &program.functions[0];
    assert_eq!(func.body.stmts.len(), 2);

    match &func.body.stmts[0] {
        Stmt::Let {
            name,
            is_mutable,
            ty,
            ..
        } => {
            assert_eq!(name, "count");
            assert!(*is_mutable);
            assert_eq!(ty.as_deref(), Some("i64"));
        }
        _ => panic!("Expected let mut statement"),
    }

    match &func.body.stmts[1] {
        Stmt::Assign { name, .. } => {
            assert_eq!(name, "count");
        }
        _ => panic!("Expected assign statement"),
    }
}

