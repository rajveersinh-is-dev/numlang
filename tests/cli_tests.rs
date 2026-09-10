use std::fs;
use std::process::Command;

#[test]
fn test_cli_emit_tokens() {
    let test_dir = std::env::temp_dir().join("numlang_test_cli");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("sample.nl");
    fs::write(&src_file, "fn main() { let x = 42; }").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("--emit-tokens")
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Fn"));
    assert!(stdout.contains("Ident(\"main\")"));
    assert!(stdout.contains("IntLiteral(42)"));
}

#[test]
fn test_cli_emit_ast() {
    let test_dir = std::env::temp_dir().join("numlang_test_cli");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("sample_ast.nl");
    fs::write(&src_file, "fn add(a: f64, b: f64) -> f64 { return a + b; }").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("--emit-ast")
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("name: \"add\""));
    assert!(stdout.contains("Add"));
}

#[test]
fn test_cli_syntax_error_diagnostic() {
    let test_dir = std::env::temp_dir().join("numlang_test_cli");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("bad.nl");
    fs::write(&src_file, "fn broken() { let x = ; }").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("syntax_error") || stderr.contains("Syntax error"));
}

#[test]
fn test_cli_check_success() {
    let test_dir = std::env::temp_dir().join("numlang_test_cli");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("valid_check.nl");
    fs::write(
        &src_file,
        "fn calculate(x: f64) -> f64 { let y: f64 = 2.0; return x * y; }",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("--check")
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("type check passed"));
}

#[test]
fn test_cli_type_error_diagnostic() {
    let test_dir = std::env::temp_dir().join("numlang_test_cli");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("type_error.nl");
    fs::write(
        &src_file,
        "fn broken() { let a: f64 = 1.0; let b: i64 = 2; let c = a + b; }",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("type_error") || stderr.contains("Type error"));
    assert!(stderr.contains("mismatched operand types") || stderr.contains("left is f64, right is i64"));
}

#[test]
fn test_cli_emit_typed_ast() {
    let test_dir = std::env::temp_dir().join("numlang_test_cli");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("typed_ast.nl");
    fs::write(
        &src_file,
        "fn square(n: i64) -> i64 { return n * n; }",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("--emit-typed-ast")
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("TypedProgram"));
    assert!(stdout.contains("return_ty: I64"));
}

