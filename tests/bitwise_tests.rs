use numlang::ir::format_ir;
use numlang::ir::lower::lower_to_ir;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::{typecheck, TypeError};
use std::fs;
use std::process::Command;

#[test]
fn test_bitwise_typecheck_rejection_for_floats() {
    let cases = [
        "fn test_and() { let x = 1.5 & 2.5; }",
        "fn test_or() { let x = 1.0 | 2.0; }",
        "fn test_xor() { let x = 1.0 ^ 2.0; }",
        "fn test_shl() { let x = 1.0 << 2; }",
        "fn test_shr() { let x = 1.0 >> 2; }",
    ];

    for src in cases {
        let tokens = tokenize(src).unwrap();
        let ast = parse(&tokens).unwrap();
        let res = typecheck(&ast);
        assert!(
            matches!(res, Err(TypeError::InvalidBinaryOperands { .. })),
            "Expected InvalidBinaryOperands for: {}",
            src
        );
    }
}

#[test]
fn test_bitwise_ir_lowering() {
    let src = r#"
        fn bit_ops(a: i64, b: i64) -> i64 {
            let x: i64 = a & b;
            let y: i64 = a | b;
            let z: i64 = a ^ b;
            let s1: i64 = a << 2;
            let s2: i64 = a >> 1;
            return x + y + z + s1 + s2;
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();
    let ir = lower_to_ir(&typed);

    let formatted = format_ir(&ir);
    assert!(formatted.contains("i64.band"));
    assert!(formatted.contains("i64.bor"));
    assert!(formatted.contains("i64.bxor"));
    assert!(formatted.contains("i64.shl"));
    assert!(formatted.contains("i64.shr"));
}

fn run_numlang_code(code: &str) -> Option<i32> {
    let test_dir = std::env::temp_dir().join("numlang_test_bitwise");
    fs::create_dir_all(&test_dir).unwrap();
    let id = format!("{}_{}", std::process::id(), std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos());
    let src_file = test_dir.join(format!("test_{}.nl", id));
    fs::write(&src_file, code).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg(&src_file)
        .output()
        .expect("Failed to run numlang program");

    let _ = fs::remove_file(&src_file);
    output.status.code()
}

#[test]
fn test_native_bitwise_operations() {
    // (0x0F & 0x07) = 7
    let code_and = r#"
        fn main() -> i64 {
            let a: i64 = 15;
            let b: i64 = 7;
            return a & b;
        }
    "#;
    assert_eq!(run_numlang_code(code_and), Some(7));

    // (4 | 2) = 6
    let code_or = r#"
        fn main() -> i64 {
            let a: i64 = 4;
            let b: i64 = 2;
            return a | b;
        }
    "#;
    assert_eq!(run_numlang_code(code_or), Some(6));

    // (7 ^ 2) = 5
    let code_xor = r#"
        fn main() -> i64 {
            let a: i64 = 7;
            let b: i64 = 2;
            return a ^ b;
        }
    "#;
    assert_eq!(run_numlang_code(code_xor), Some(5));

    // (1 << 4) = 16
    let code_shl = r#"
        fn main() -> i64 {
            let a: i64 = 1;
            return a << 4;
        }
    "#;
    assert_eq!(run_numlang_code(code_shl), Some(16));

    // (32 >> 2) = 8
    let code_shr = r#"
        fn main() -> i64 {
            let a: i64 = 32;
            return a >> 2;
        }
    "#;
    assert_eq!(run_numlang_code(code_shr), Some(8));
}

#[test]
fn test_native_kernighan_popcount() {
    // Brian Kernighan's algorithm: counts 1-bits dynamically at runtime
    // 45 in binary is 101101 (4 set bits)
    let code = r#"
        fn count_bits(n: i64) -> i64 {
            let mut curr: i64 = n;
            let mut count: i64 = 0;
            while curr > 0 {
                curr = curr & (curr - 1);
                count = count + 1;
            }
            return count;
        }

        fn main() -> i64 {
            return count_bits(45);
        }
    "#;
    assert_eq!(run_numlang_code(code), Some(4));
}
