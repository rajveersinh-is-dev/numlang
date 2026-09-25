use std::fs;
use std::process::Command;

fn run_numlang_code(code: &str, supercompile: bool) -> (Option<i32>, String, String) {
    let id = format!(
        "{}_{:?}_{}",
        std::process::id(),
        std::thread::current().id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid time")
            .as_nanos()
    )
    .replace(['(', ')', ' '], "_");
    let test_dir = std::env::temp_dir().join(format!("numlang_ho_{}", id));
    fs::create_dir_all(&test_dir).expect("create test dir");
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).expect("write src file");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run");
    if supercompile {
        cmd.arg("--supercompile");
    }
    cmd.arg(&src_file);

    let output = cmd.output().expect("Failed to run numlang program");

    let _ = fs::remove_dir_all(&test_dir);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.code(), stdout, stderr)
}

#[test]
fn test_lambda_inline_fold() {
    // A closure passed to a function and immediately inlined by supercompiler
    let code = r#"
fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return f(x);
}
fn main() -> i64 {
    return apply(|x: i64| x * 2, 21);
}
"#;
    let (sc, _, stderr) = run_numlang_code(code, true);
    assert_eq!(sc, Some(42), "stderr: {}", stderr);
}

#[test]
fn test_closure_captures_constant() {
    let code = r#"
fn main() -> i64 {
    let factor: i64 = 7;
    let f: fn(i64) -> i64 = |x: i64| x * factor;
    return f(6);
}
"#;
    let (sc, _, stderr) = run_numlang_code(code, true);
    assert_eq!(sc, Some(42), "stderr: {}", stderr);
}

#[test]
fn test_higher_order_supercompile_constant() {
    // apply(|x| x + x, 5) with supercompilation → return 10;
    let code = r#"
fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return f(x);
}
fn main() -> i64 {
    return apply(|x: i64| x + x, 5);
}
"#;
    let (sc, _, stderr) = run_numlang_code(code, true);
    assert_eq!(sc, Some(10), "stderr: {}", stderr);
}
