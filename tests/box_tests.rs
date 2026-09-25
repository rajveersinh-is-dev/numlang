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
    let test_dir = std::env::temp_dir().join(format!("numlang_box_{}", id));
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
fn test_box_roundtrip() {
    let code = r#"
fn main() -> i64 {
    let x: i64 = 42;
    let bx: Box<i64> = box(x);
    return deref(bx);
}
"#;
    let (norm, _, stderr) = run_numlang_code(code, false);
    assert_eq!(norm, Some(42), "stderr: {}", stderr);
}

#[test]
fn test_box_in_enum() {
    // Linked list: Cons(1, box(Cons(2, box(Nil))))
    // sum_list supercompiles to `return 3;`
    let code = r#"
enum List { Nil, Cons(i64, Box<List>) }

fn sum_list(l: List) -> i64 {
    return match l {
        Nil => 0,
        Cons(h, t) => h + sum_list(deref(t)),
    };
}

fn main() -> i64 {
    let l: List = Cons(1, box(Cons(2, box(Nil))));
    return sum_list(l);
}
"#;
    let (norm, _, stderr_norm) = run_numlang_code(code, false);
    assert_eq!(norm, Some(3), "stderr norm: {}", stderr_norm);

    let (sc, _, stderr_sc) = run_numlang_code(code, true);
    assert_eq!(sc, Some(3), "stderr sc: {}", stderr_sc);
}
