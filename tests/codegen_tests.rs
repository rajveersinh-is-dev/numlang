use numlang::ir::format_ir;
use numlang::ir::lower::lower_to_ir;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use std::fs;
use std::process::Command;

#[test]
fn test_ir_lowering_arithmetic() {
    let src = r#"
        fn compute(a: f64, b: f64) -> f64 {
            let sum: f64 = a + b;
            let product: f64 = sum * 2.0;
            return product;
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();
    let ir = lower_to_ir(&typed);

    assert_eq!(ir.functions.len(), 1);
    let func = &ir.functions[0];
    assert_eq!(func.name, "compute");
    assert_eq!(func.params.len(), 2);
    assert!(!func.blocks.is_empty());

    let formatted = format_ir(&ir);
    assert!(formatted.contains("function compute"));
    assert!(formatted.contains("f64.add"));
    assert!(formatted.contains("f64.mul"));
    assert!(formatted.contains("ret"));
}

#[test]
fn test_ir_lowering_control_flow() {
    let src = r#"
        fn abs_val(x: i64) -> i64 {
            if x < 0 {
                return -x;
            } else {
                return x;
            }
        }

        fn count_up(limit: i64) {
            let mut i: i64 = 0;
            while i < limit {
                i = i + 1;
            }
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();
    let ir = lower_to_ir(&typed);

    assert_eq!(ir.functions.len(), 2);

    let formatted = format_ir(&ir);
    assert!(formatted.contains("brif"));
    assert!(formatted.contains("br bb"));
}

#[test]
fn test_cli_emit_ir() {
    let test_dir = std::env::temp_dir().join("numlang_test_cli");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("test_ir.nl");
    fs::write(
        &src_file,
        "fn dot(a: f64, b: f64) -> f64 { return a * b; }",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("--emit-ir")
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("function dot"));
    assert!(stdout.contains("f64.mul"));
    assert!(stdout.contains("ret"));
}
