use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn run_numlang_code(code: &str, supercompile: bool) -> (Option<i32>, String, String) {
    let test_id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tid = std::thread::current().id();
    let temp_dir = std::env::temp_dir().join(format!(
        "numlang_generic_{}_{:?}_{}",
        std::process::id(),
        tid,
        test_id
    ));
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let source_path = temp_dir.join("test.nl");
    fs::write(&source_path, code).expect("Failed to write test source");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run");
    if supercompile {
        cmd.arg("--supercompile");
    }
    cmd.arg(&source_path);

    let output = cmd.output().expect("Failed to execute numlang command");

    let _ = fs::remove_dir_all(&temp_dir);

    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn test_generic_identity() {
    let code = r#"
fn id<T>(x: T) -> T {
    return x;
}

fn main() -> i64 {
    return id(42);
}
"#;
    let (norm, _, stderr_norm) = run_numlang_code(code, false);
    assert_eq!(norm, Some(42), "stderr norm: {}", stderr_norm);

    let (sc, _, stderr_sc) = run_numlang_code(code, true);
    assert_eq!(sc, Some(42), "stderr sc: {}", stderr_sc);
}

#[test]
fn test_generic_swap_pair() {
    let code = r#"
fn select_first<T, U>(a: T, b: U) -> T {
    return a;
}

fn main() -> i64 {
    let x: i64 = select_first(42, 100);
    return x;
}
"#;
    let (norm, _, stderr_norm) = run_numlang_code(code, false);
    assert_eq!(norm, Some(42), "stderr norm: {}", stderr_norm);

    let (sc, _, stderr_sc) = run_numlang_code(code, true);
    assert_eq!(sc, Some(42), "stderr sc: {}", stderr_sc);
}

#[test]
fn test_generic_higher_order_apply() {
    let code = r#"
fn apply<T, U>(f: fn(T) -> U, x: T) -> U {
    return f(x);
}

fn main() -> i64 {
    return apply(|x: i64| x * 2, 21);
}
"#;
    let (norm, _, stderr_norm) = run_numlang_code(code, false);
    assert_eq!(norm, Some(42), "stderr norm: {}", stderr_norm);

    let (sc, _, stderr_sc) = run_numlang_code(code, true);
    assert_eq!(sc, Some(42), "stderr sc: {}", stderr_sc);
}

#[test]
fn test_generic_multiple_instantiations() {
    let code = r#"
fn choose_second<T, U>(a: T, b: U) -> U {
    return b;
}

fn main() -> i64 {
    let r1: i64 = choose_second(true, 40);
    let r2: i64 = choose_second(10, 2);
    return r1 + r2;
}
"#;
    let (norm, _, stderr_norm) = run_numlang_code(code, false);
    assert_eq!(norm, Some(42), "stderr norm: {}", stderr_norm);

    let (sc, _, stderr_sc) = run_numlang_code(code, true);
    assert_eq!(sc, Some(42), "stderr sc: {}", stderr_sc);
}
