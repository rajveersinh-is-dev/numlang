use numlang::opt::optimize_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::typed_ast::{TypedExpr, TypedStmt};

#[test]
fn propagates_shared_literal_arguments_into_non_recursive_function() {
    let source = r#"
        fn scale(n: i64) -> i64 { return n * 3; }
        fn main() -> i64 { return scale(7); }
    "#;
    let ast = parse(&tokenize(source).unwrap()).unwrap();
    let mut program = typecheck(&ast).unwrap();
    optimize_program(&mut program);

    let scale = program
        .functions
        .iter()
        .find(|function| function.name == "scale")
        .unwrap();
    let TypedStmt::Return(Some(TypedExpr::Literal { .. }), _) = &scale.body.stmts[0] else {
        panic!("expected the propagated expression to be constant-folded");
    };
}

#[test]
fn does_not_specialize_recursive_function() {
    let source = r#"
        fn count(n: i64) -> i64 {
            if n == 0 { return 0; }
            return count(n - 1);
        }
        fn main() -> i64 { return count(4); }
    "#;
    let ast = parse(&tokenize(source).unwrap()).unwrap();
    let mut program = typecheck(&ast).unwrap();
    optimize_program(&mut program);

    let count = program
        .functions
        .iter()
        .find(|function| function.name == "count")
        .unwrap();
    let TypedStmt::If { condition, .. } = &count.body.stmts[0] else {
        panic!("expected recursive guard");
    };
    assert!(
        matches!(condition, TypedExpr::Binary { left, .. } if matches!(&**left, TypedExpr::Ident { name, .. } if name == "n"))
    );
}

#[test]
fn propagates_immutable_local_literal_into_call_argument() {
    let source = r#"
        fn add_mod(value: i64, modulus: i64) -> i64 { return value % modulus; }
        fn main() -> i64 {
            let modulus: i64 = 17;
            return add_mod(99, modulus);
        }
    "#;
    let ast = parse(&tokenize(source).unwrap()).unwrap();
    let mut program = typecheck(&ast).unwrap();
    optimize_program(&mut program);

    let add_mod = program
        .functions
        .iter()
        .find(|function| function.name == "add_mod")
        .unwrap();
    let TypedStmt::Return(Some(TypedExpr::Binary { right, .. }), _) = &add_mod.body.stmts[0] else {
        panic!("expected modulo return expression");
    };
    assert!(matches!(&**right, TypedExpr::Literal { .. }));
}
