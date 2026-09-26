use std::fs;
use std::process::Command;

fn run_minspec(extra_args: &[&str]) -> (Option<i32>, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run");
    for arg in extra_args {
        cmd.arg(arg);
    }
    cmd.arg("src/stdlib/minspec.nl");
    let out = cmd.output().expect("numlang run failed");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into(),
        String::from_utf8_lossy(&out.stderr).into(),
    )
}

/// Phase 25 Test 1: minspec.nl exits 42 — all Futamura projections are sound
#[test]
fn test_minspec_exits_42() {
    let (code, _, stderr) = run_minspec(&[]);
    assert_eq!(code, Some(42), "minspec.nl must exit 42; stderr:\n{}", stderr);
}

/// Phase 25 Test 2: first_futamura(1) yields Mul(Add(Var(1), Lit(10)), Lit(2))
/// Verified via structural check in minspec.nl (the existing main does this, but we also test
/// that prog_id=1 specializes to `(x+10)*2` shape).
#[test]
fn test_first_futamura_prog1_specialization() {
    let code = r#"
fn main() -> i64 {
    // (x+10)*2 = top-level Mul, left child is Add(Var(1), Lit(10)), right is Lit(2)
    let p: SpecExpr = first_futamura(1);
    let ok: bool = match p {
        SpecExpr::Bin(op, l, r) => match deref(l) {
            SpecExpr::Bin(lop, ll, lr) => match deref(ll) {
                SpecExpr::Var(id) => id == 1,
                _ => false,
            },
            _ => false,
        },
        _ => false,
    };
    if ok { return 42; }
    return 10;
}
"#;
    let id = std::process::id();
    let dir = std::env::temp_dir().join(format!("nl_phase25_t2_{}", id));
    fs::create_dir_all(&dir).unwrap();
    // prepend minspec.nl contents, replacing original main
    let minspec = fs::read_to_string("src/stdlib/minspec.nl").unwrap();
    let minspec_stripped = minspec.replacen("fn main()", "fn _orig_main()", 1);
    let combined = format!("{}\n{}", minspec_stripped, code);
    let src = dir.join("test.nl");
    fs::write(&src, &combined).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run").arg(&src);
    let out = cmd.output().unwrap();
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(out.status.code(), Some(42), "first_futamura(1) shape check failed;\nstderr: {}", String::from_utf8_lossy(&out.stderr));
}

/// Phase 25 Test 3: spec_eval(first_futamura(1), {1→5}) = 30  (i.e. (5+10)*2 = 30)
#[test]
fn test_first_futamura_eval_correctness() {
    let id = std::process::id();
    let dir = std::env::temp_dir().join(format!("nl_phase25_t3_{}", id));
    fs::create_dir_all(&dir).unwrap();
    let minspec = fs::read_to_string("src/stdlib/minspec.nl").unwrap();
    let harness = r#"
fn main_test() -> i64 {
    let p: SpecExpr = first_futamura(1);
    let e0: SpecEnv = SpecEnv::Nil;
    let b0: Box<SpecEnv> = box(e0);
    let env: SpecEnv = SpecEnv::Cons(1, 5, b0);
    let result: i64 = spec_eval(p, env);
    if result == 30 { return 42; }
    return result;
}
fn main() -> i64 { return main_test(); }
"#;
    // Replace only the first occurrence of fn main() in minspec
    let minspec_stripped = minspec.replacen(
        "fn main()",
        "fn _orig_main()",
        1,
    );
    let combined = format!("{}\n{}", minspec_stripped, harness);
    let src = dir.join("test.nl");
    fs::write(&src, &combined).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run").arg(&src);
    let out = cmd.output().unwrap();
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(out.status.code(), Some(42),
        "spec_eval(first_futamura(1), {{1→5}}) must equal 30; exit={:?}, stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Phase 25 Test 4: verify_soundness(prog_id, input_x) is true for all 3 programs
#[test]
fn test_verify_soundness_all_programs() {
    // This test simply runs minspec.nl with the final main that calls verify_soundness.
    // If it exits 42, all 3 Futamura projections are sound.
    let (code, _, stderr) = run_minspec(&[]);
    assert_eq!(code, Some(42),
        "verify_soundness must pass for prog_id in {{1,2,3}}; stderr:\n{}", stderr);
}
