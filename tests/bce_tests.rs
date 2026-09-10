use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typed_ast::{TypedExpr, TypedStmt};
use numlang::typecheck::typecheck;
use std::fs;
use std::process::Command;

#[test]
fn test_constant_index_bce() {
    let src = r#"
        fn main() -> f64 {
            let mut arr: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
            arr[2] = 10.0;
            return arr[2];
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();

    let func = &typed.functions[0];
    // Statement 1 is IndexAssign
    match &func.body.stmts[1] {
        TypedStmt::IndexAssign { is_safe, .. } => {
            assert!(is_safe, "Constant index within bounds must have is_safe = true");
        }
        other => panic!("Expected IndexAssign, got {:?}", other),
    }

    // Statement 2 is Return(Some(TypedExpr::Index))
    match &func.body.stmts[2] {
        TypedStmt::Return(Some(TypedExpr::Index { is_safe, .. }), _) => {
            assert!(is_safe, "Constant read index within bounds must have is_safe = true");
        }
        other => panic!("Expected Return with Index, got {:?}", other),
    }
}

#[test]
fn test_loop_induction_variable_bce() {
    let src = r#"
        fn main() {
            let mut arr: [f64; 10] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
            let mut i: i64 = 0;
            while i < 10 {
                arr[i] = 42.0;
                let v: f64 = arr[i];
                i = i + 1;
            }
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();

    let func = &typed.functions[0];
    // Statement 2 is While loop
    match &func.body.stmts[2] {
        TypedStmt::While { body, .. } => {
            // body stmt 0 is arr[i] = 42.0
            match &body.stmts[0] {
                TypedStmt::IndexAssign { is_safe, .. } => {
                    assert!(is_safe, "Loop induction index arr[i] within bound 10 must be is_safe = true");
                }
                other => panic!("Expected IndexAssign, got {:?}", other),
            }

            // body stmt 1 is let v = arr[i]
            match &body.stmts[1] {
                TypedStmt::Let { value, .. } => match value {
                    TypedExpr::Index { is_safe, .. } => {
                        assert!(is_safe, "Loop induction read arr[i] within bound 10 must be is_safe = true");
                    }
                    other => panic!("Expected Index expr, got {:?}", other),
                },
                other => panic!("Expected Let stmt, got {:?}", other),
            }
        }
        other => panic!("Expected While stmt, got {:?}", other),
    }
}

#[test]
fn test_unbounded_index_retains_checks() {
    let src = r#"
        fn main() {
            let mut arr: [f64; 10] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
            let mut i: i64 = 0;
            while i < 20 {
                arr[i] = 42.0;
                i = i + 1;
            }
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();

    let func = &typed.functions[0];
    match &func.body.stmts[2] {
        TypedStmt::While { body, .. } => {
            match &body.stmts[0] {
                TypedStmt::IndexAssign { is_safe, .. } => {
                    assert!(!is_safe, "Loop induction bound 20 exceeding array length 10 must NOT be is_safe");
                }
                other => panic!("Expected IndexAssign, got {:?}", other),
            }
        }
        other => panic!("Expected While stmt, got {:?}", other),
    }
}

#[test]
fn test_compiled_bce_loop_execution() {
    let test_dir = std::env::temp_dir().join("numlang_test_bce_exec");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("bce_loop.nl");
    let exe_file = test_dir.join("bce_loop.exe");

    let src = r#"
        fn main() -> i64 {
            let mut arr: [i64; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
            let mut i: i64 = 0;
            while i < 8 {
                arr[i] = i * 2;
                i = i + 1;
            }

            let mut sum: i64 = 0;
            let mut j: i64 = 0;
            while j < 8 {
                sum = sum + arr[j];
                j = j + 1;
            }

            return sum;
        }
    "#;
    fs::write(&src_file, src).unwrap();

    let compile_output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("-o")
        .arg(&exe_file)
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang compiler");

    assert!(
        compile_output.status.success(),
        "Compilation failed:\nSTDOUT: {}\nSTDERR: {}",
        String::from_utf8_lossy(&compile_output.stdout),
        String::from_utf8_lossy(&compile_output.stderr)
    );

    let run_output = Command::new(&exe_file)
        .output()
        .expect("Failed to execute compiled numlang binary");

    // sum of (0 + 2 + 4 + 6 + 8 + 10 + 12 + 14) = 56
    assert_eq!(run_output.status.code(), Some(56));
}
