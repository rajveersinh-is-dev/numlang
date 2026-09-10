use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::{typecheck, Type, TypeError};

#[test]
fn test_typecheck_valid_program() {
    let src = r#"
        fn add(a: f64, b: f64) -> f64 {
            let sum = a + b;
            return sum;
        }

        fn main() {
            let mut x: f64 = 10.5;
            x = add(x, 2.5);
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();

    assert_eq!(typed.functions.len(), 2);
    assert_eq!(typed.functions[0].return_ty, Type::F64);
    assert_eq!(typed.functions[1].return_ty, Type::Void);
}

#[test]
fn test_strict_rejection_of_mixed_arithmetic() {
    let src = r#"
        fn mixed() {
            let a: f64 = 10.5;
            let b: i64 = 5;
            let c = a + b;
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let res = typecheck(&ast);

    match res {
        Err(TypeError::InvalidBinaryOperands { left, right, .. }) => {
            assert_eq!(left, Type::F64);
            assert_eq!(right, Type::I64);
        }
        _ => panic!("Expected InvalidBinaryOperands error, got {:?}", res),
    }
}

#[test]
fn test_immutability_enforcement() {
    let src = r#"
        fn mutate_immutable() {
            let x = 42;
            x = 100;
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let res = typecheck(&ast);

    match res {
        Err(TypeError::CannotMutateImmutable { name, .. }) => {
            assert_eq!(name, "x");
        }
        _ => panic!("Expected CannotMutateImmutable error, got {:?}", res),
    }
}

#[test]
fn test_mutability_success() {
    let src = r#"
        fn mutate_mutable() {
            let mut x: i64 = 42;
            x = 100;
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    assert!(typecheck(&ast).is_ok());
}

#[test]
fn test_undeclared_variable() {
    let src = r#"
        fn bad_var() {
            let y = x + 1;
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let res = typecheck(&ast);

    match res {
        Err(TypeError::UndeclaredVariable { name, .. }) => {
            assert_eq!(name, "x");
        }
        _ => panic!("Expected UndeclaredVariable error, got {:?}", res),
    }
}

#[test]
fn test_block_scoping_and_shadowing() {
    let src = r#"
        fn scoping() {
            let x: i64 = 10;
            if true {
                let x: f64 = 3.14;
                let y = x + 1.0;
            }
            let z = x + 5;
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    assert!(typecheck(&ast).is_ok());
}

#[test]
fn test_function_arity_mismatch() {
    let src = r#"
        fn calc(a: i64, b: i64) -> i64 {
            return a + b;
        }

        fn run() {
            let res = calc(10);
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let res = typecheck(&ast);

    match res {
        Err(TypeError::ArityMismatch { name, expected, found, .. }) => {
            assert_eq!(name, "calc");
            assert_eq!(expected, 2);
            assert_eq!(found, 1);
        }
        _ => panic!("Expected ArityMismatch error, got {:?}", res),
    }
}

#[test]
fn test_function_return_mismatch() {
    let src = r#"
        fn get_int() -> i64 {
            return 3.14;
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let res = typecheck(&ast);

    match res {
        Err(TypeError::InvalidReturn { expected, found, .. }) => {
            assert_eq!(expected, Type::I64);
            assert_eq!(found, Type::F64);
        }
        _ => panic!("Expected InvalidReturn error, got {:?}", res),
    }
}

#[test]
fn test_condition_must_be_bool() {
    let src = r#"
        fn loop_bad() {
            while 42 {
                let x = 1;
            }
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let res = typecheck(&ast);

    match res {
        Err(TypeError::InvalidConditionType { found, .. }) => {
            assert_eq!(found, Type::I64);
        }
        _ => panic!("Expected InvalidConditionType error, got {:?}", res),
    }
}
