use numlang::testing::random_program_gen::generate_random_program;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

fn run_numlang_code(code: &str, supercompile: bool) -> Option<i32> {
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
    let test_dir = std::env::temp_dir().join(format!("numlang_diff_{}", id));
    fs::create_dir_all(&test_dir).expect("Failed to create test_dir");
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).expect("Failed to write test source");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run");
    cmd.arg("--use-mir");
    if supercompile {
        cmd.arg("--supercompile");
    }
    cmd.arg(&src_file);

    let output = cmd.output().expect("Failed to run numlang program");

    let _ = fs::remove_dir_all(&test_dir);
    output.status.code()
}

fn run_numlang_code_default(code: &str) -> Option<i32> {
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
    let test_dir = std::env::temp_dir().join(format!("numlang_diff_{}", id));
    fs::create_dir_all(&test_dir).expect("Failed to create test_dir");
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).expect("Failed to write test source");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run");
    cmd.arg(&src_file);

    let output = cmd.output().expect("Failed to run numlang program");

    let _ = fs::remove_dir_all(&test_dir);
    output.status.code()
}

#[test]
fn test_differential_random_programs_small() {
    // 1000 programs — fast enough for CI using thread pool
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
        .min(16);
    let current_seed = AtomicU64::new(0);
    let divergences = Mutex::new(Vec::new());

    std::thread::scope(|s| {
        for _ in 0..num_threads {
            s.spawn(|| loop {
                let seed = current_seed.fetch_add(1, Ordering::Relaxed);
                if seed >= 1000 {
                    break;
                }
                let prog = generate_random_program(seed);
                let norm = run_numlang_code(&prog, false);
                let sc = run_numlang_code(&prog, true);
                if norm != sc {
                    let mut divs = divergences.lock().expect("mutex lock");
                    divs.push((seed, norm, sc, prog));
                }
            });
        }
    });

    let divergences = divergences.into_inner().expect("mutex into_inner");
    assert!(
        divergences.is_empty(),
        "{} divergences found:\n{}",
        divergences.len(),
        divergences
            .iter()
            .take(3)
            .map(|(s, n, sc, p)| format!("seed={}: norm={:?} sc={:?}\n---\n{}\n---", s, n, sc, p))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn test_differential_10k() {
    let count: u64 = std::env::var("DIFF_CORRECTNESS_COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if cfg!(debug_assertions) { 200 } else { 10_000 });

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
        .min(16);
    let current_seed = AtomicU64::new(0);
    let divergences = Mutex::new(Vec::new());

    std::thread::scope(|s| {
        for _ in 0..num_threads {
            s.spawn(|| loop {
                let seed = current_seed.fetch_add(1, Ordering::Relaxed);
                if seed >= count {
                    break;
                }
                let prog = generate_random_program(seed);
                let norm = run_numlang_code(&prog, false);
                let sc = run_numlang_code(&prog, true);
                if norm != sc {
                    let mut divs = divergences.lock().expect("mutex lock");
                    divs.push((seed, norm, sc, prog));
                }
            });
        }
    });

    let divs = divergences.into_inner().expect("mutex poisoned");
    assert!(
        divs.is_empty(),
        "{} divergences in {} programs:\n{}",
        divs.len(),
        count,
        divs.iter()
            .take(5)
            .map(|(s, n, sc, p)| format!("seed={}: norm={:?} sc={:?}\n---\n{}\n---", s, n, sc, p))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn test_differential_examples_directory() {
    let examples_dir = std::path::Path::new("examples");
    if !examples_dir.exists() {
        return;
    }
    for entry in fs::read_dir(examples_dir).expect("read examples dir") {
        let entry = entry.expect("valid entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("nl") {
            let code = fs::read_to_string(&path).expect("read example file");
            let norm = run_numlang_code(&code, false);
            let sc = run_numlang_code(&code, true);
            assert_eq!(
                norm,
                sc,
                "Differential divergence on example file {}: baseline={:?}, sc={:?}",
                path.display(),
                norm,
                sc
            );
        }
    }
}

#[test]
fn test_regression_seed178_loop_mod() {
    // Minimized from differential seed 178: baseline/supercompiled divergence
    // caused by AST Cranelift codegen miscompilation on this control-flow shape.
    // The correct result is 252.
    let code = r#"
fn loop_fn(base: i64, limit: i64) -> i64 {
    let mut acc: i64 = base;
    let mut i: i64 = 0;
    let mut max_iter: i64 = limit;
    if limit > 10 { max_iter = 10; }
    if limit < 1 { max_iter = 1; }
    while i < max_iter {
        if (i % 2) == 0 {
            acc = acc + i * 1;
        } else {
            acc = acc - (i + 8);
        }
        i = i + 1;
    }
    return acc;
}
fn main() -> i64 {
    let mut a: i64 = 15;
    let mut b: i64 = 19;
    b = loop_fn(b, 1);
    b = loop_fn(b, 4);
    b = loop_fn(b, 7);
    b = (a + b) % 300;
    b = loop_fn(b, 5);
    let final_val: i64 = (a + b) % 256;
    if final_val < 0 {
        return final_val + 256;
    } else {
        return final_val;
    }
}
"#;
    let norm = run_numlang_code(code, false);
    let sc = run_numlang_code(code, true);
    assert_eq!(norm, sc, "seed 178 regression diverged: norm={norm:?} sc={sc:?}");
    assert_eq!(norm, Some(252), "seed 178 regression wrong result: {norm:?}");
}

#[test]
fn test_regression_seed178_ast_default() {
    // End-to-end: default AST Cranelift path must also return 252.
    let code = r#"
fn loop_fn(base: i64, limit: i64) -> i64 {
    let mut acc: i64 = base;
    let mut i: i64 = 0;
    let mut max_iter: i64 = limit;
    if limit > 10 { max_iter = 10; }
    if limit < 1 { max_iter = 1; }
    while i < max_iter {
        if (i % 2) == 0 {
            acc = acc + i * 1;
        } else {
            acc = acc - (i + 8);
        }
        i = i + 1;
    }
    return acc;
}
fn main() -> i64 {
    let mut a: i64 = 15;
    let mut b: i64 = 19;
    b = loop_fn(b, 1);
    b = loop_fn(b, 4);
    b = loop_fn(b, 7);
    b = (a + b) % 300;
    b = loop_fn(b, 5);
    let final_val: i64 = (a + b) % 256;
    if final_val < 0 {
        return final_val + 256;
    } else {
        return final_val;
    }
}
"#;
    let res = run_numlang_code_default(code);
    assert_eq!(res, Some(252), "default AST path wrong result: {res:?}");
}
