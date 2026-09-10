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
