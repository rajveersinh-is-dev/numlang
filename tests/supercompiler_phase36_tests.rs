use std::fs;
use std::process::Command;

use numlang::mir::supercompiler::independence::{sets_are_independent, ReadWriteSet};
use numlang::mir::{BasicBlockId, Terminator};

fn run_numlang_code(
    code: &str,
    supercompile: bool,
    parallel_residualize: bool,
) -> (Option<i32>, String, String) {
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
    let test_dir = std::env::temp_dir().join(format!("numlang_phase36_{}", id));
    fs::create_dir_all(&test_dir).expect("create test dir");
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).expect("write src file");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run");
    if supercompile {
        cmd.arg("--supercompile");
    }
    if parallel_residualize {
        cmd.arg("--parallel-residualize");
    }
    cmd.arg(&src_file);

    let output = cmd.output().expect("Failed to run numlang program");

    let _ = fs::remove_dir_all(&test_dir);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.code(), stdout, stderr)
}

#[test]
fn test_independence_disjoint_sets() {
    let mut set_a = ReadWriteSet::default();
    set_a.reads.insert("x".to_string());
    set_a.writes.insert("a".to_string());

    let mut set_b = ReadWriteSet::default();
    set_b.reads.insert("y".to_string());
    set_b.writes.insert("b".to_string());

    assert!(
        sets_are_independent(&set_a, &set_b),
        "Disjoint sets should be recognized as data-independent"
    );
}

#[test]
fn test_independence_conflicting_write() {
    let mut set_a = ReadWriteSet::default();
    set_a.reads.insert("x".to_string());
    set_a.writes.insert("shared".to_string());

    let mut set_b = ReadWriteSet::default();
    set_b.reads.insert("shared".to_string());
    set_b.writes.insert("y".to_string());

    assert!(
        !sets_are_independent(&set_a, &set_b),
        "Sets with a write-read conflict must not be data-independent"
    );

    let mut set_c = ReadWriteSet::default();
    set_c.writes.insert("shared".to_string());

    assert!(
        !sets_are_independent(&set_a, &set_c),
        "Sets with a write-write conflict must not be data-independent"
    );
}

#[test]
fn test_fork_terminator_successors() {
    let fork = Terminator::Fork {
        left: BasicBlockId(10),
        right: BasicBlockId(20),
        join: BasicBlockId(30),
    };
    let succs = fork.successors();
    assert_eq!(succs.len(), 3);
    assert_eq!(succs[0], BasicBlockId(10));
    assert_eq!(succs[1], BasicBlockId(20));
    assert_eq!(succs[2], BasicBlockId(30));
}

#[test]
fn test_parallel_residualize_correctness() {
    let src = r#"
fn main() -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 0;
    let mut i: i64 = 0;
    while i < 10 {
        a = a + i;
        b = b + 1;
        i = i + 1;
    }
    return a + b;
}
"#;
    let (code, stdout, stderr) = run_numlang_code(src, true, true);
    assert_eq!(
        code,
        Some(55),
        "Expected exit code 55 with --parallel-residualize, got {:?}. stdout: '{}', stderr: '{}'",
        code,
        stdout,
        stderr
    );
}

#[test]
fn test_parallel_residualize_sequential_fallback() {
    let src = r#"
fn main() -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 0;
    let mut i: i64 = 0;
    while i < 10 {
        a = a + i;
        b = b + 1;
        i = i + 1;
    }
    return a + b;
}
"#;
    let (code, stdout, stderr) = run_numlang_code(src, true, false);
    assert_eq!(
        code,
        Some(55),
        "Expected exit code 55 in sequential mode, got {:?}. stdout: '{}', stderr: '{}'",
        code,
        stdout,
        stderr
    );
}
