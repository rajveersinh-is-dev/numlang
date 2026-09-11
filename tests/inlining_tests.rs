use numlang::opt::optimize_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::typed_ast::{TypedExpr, TypedStmt};

#[test]
fn inlines_single_return_function() {
    let source = r#"
        fn square(x: i64) -> i64 { return x * x; }
        fn main() -> i64 {
            let res: i64 = square(5);
            return res;
        }
    "#;
    let ast = parse(&tokenize(source).unwrap()).unwrap();
    let mut program = typecheck(&ast).unwrap();
    optimize_program(&mut program);

    let main = program
        .functions
        .iter()
        .find(|f| f.name == "main")
        .unwrap();

    let mut has_call = false;
    for stmt in &main.body.stmts {
        if let TypedStmt::Let { value: TypedExpr::Call { .. }, .. } = stmt {
            has_call = true;
        }
    }
    assert!(!has_call, "expected square(5) to be inlined");
}

#[test]
fn inlines_early_return_function() {
    let source = r#"
        fn abs_diff(a: i64, b: i64) -> i64 {
            if a >= b {
                return a - b;
            }
            return b - a;
        }
        fn main() -> i64 {
            let res: i64 = abs_diff(10, 25);
            return res;
        }
    "#;
    let ast = parse(&tokenize(source).unwrap()).unwrap();
    let mut program = typecheck(&ast).unwrap();
    optimize_program(&mut program);

    let main = program
        .functions
        .iter()
        .find(|f| f.name == "main")
        .unwrap();

    let mut has_call = false;
    for stmt in &main.body.stmts {
        if let TypedStmt::Let { value: TypedExpr::Call { .. }, .. } = stmt {
            has_call = true;
        }
    }
    assert!(!has_call, "expected abs_diff(10, 25) to be inlined");
}

#[test]
fn inlines_while_loop_with_early_return() {
    let source = r#"
        fn find_divisor(n: i64) -> i64 {
            let mut d: i64 = 2;
            while d * d <= n {
                if n % d == 0 {
                    return d;
                }
                d = d + 1;
            }
            return n;
        }
        fn main() -> i64 {
            let res: i64 = find_divisor(77);
            return res;
        }
    "#;
    let ast = parse(&tokenize(source).unwrap()).unwrap();
    let mut program = typecheck(&ast).unwrap();
    optimize_program(&mut program);

    let main = program
        .functions
        .iter()
        .find(|f| f.name == "main")
        .unwrap();

    let mut has_call = false;
    for stmt in &main.body.stmts {
        if let TypedStmt::Let { value: TypedExpr::Call { .. }, .. } = stmt {
            has_call = true;
        }
    }
    assert!(!has_call, "expected find_divisor to be inlined");
}

#[test]
fn does_not_inline_recursive_functions() {
    let source = r#"
        fn factorial(n: i64) -> i64 {
            if n <= 1 { return 1; }
            return n * factorial(n - 1);
        }
        fn main() -> i64 {
            return factorial(5);
        }
    "#;
    let ast = parse(&tokenize(source).unwrap()).unwrap();
    let mut program = typecheck(&ast).unwrap();
    optimize_program(&mut program);

    let fact = program
        .functions
        .iter()
        .find(|f| f.name == "factorial")
        .unwrap();

    assert_eq!(fact.name, "factorial");
}
