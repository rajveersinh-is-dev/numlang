use std::fs;
use std::process::Command;

fn run_numlang_code(code: &str) -> (Option<i32>, String, String) {
    let id = format!(
        "{}_{:?}_{}",
        std::process::id(),
        std::thread::current().id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
    .replace(['(', ')', ' '], "_");
    let test_dir = std::env::temp_dir().join(format!("nl_futamura_p43_{}", id));
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run");
    cmd.arg(&src_file);

    let output = cmd.output().expect("Failed to execute numlang runner");
    let _ = fs::remove_dir_all(&test_dir);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.code(), stdout, stderr)
}

#[test]
fn test_no_identical_function_bodies_in_minspec() {
    let content = fs::read_to_string("src/stdlib/minspec.nl")
        .expect("Failed to read src/stdlib/minspec.nl");

    // Extract function bodies for first_futamura, second_futamura_compiler, third_futamura_cogen
    let fn1_start = content.find("fn first_futamura(").expect("first_futamura not found");
    let fn2_start = content.find("fn second_futamura_compiler(").expect("second_futamura_compiler not found");
    let fn3_start = content.find("fn third_futamura_cogen(").expect("third_futamura_cogen not found");
    let fn_end = content.find("fn verify_soundness(").expect("verify_soundness not found");

    let fn1_text = &content[fn1_start..fn2_start];
    let fn2_text = &content[fn2_start..fn3_start];
    let fn3_text = &content[fn3_start..fn_end];

    // Assert that none of the bodies are byte-for-byte identical
    assert_ne!(
        fn1_text.trim(),
        fn2_text.trim(),
        "first_futamura and second_futamura_compiler must not be identical copy-pastes"
    );
    assert_ne!(
        fn2_text.trim(),
        fn3_text.trim(),
        "second_futamura_compiler and third_futamura_cogen must not be identical copy-pastes"
    );
    assert_ne!(
        fn1_text.trim(),
        fn3_text.trim(),
        "first_futamura and third_futamura_cogen must not be identical copy-pastes"
    );

    // Verify key distinct concepts exist
    assert!(
        fn2_text.contains("specialize_compiler"),
        "second_futamura_compiler must invoke specialize_compiler"
    );
    assert!(
        fn3_text.contains("specialize_cogen"),
        "third_futamura_cogen must invoke specialize_cogen"
    );
    assert!(
        fn3_text.contains("run_cogen"),
        "third_futamura_cogen must invoke run_cogen"
    );
}

#[test]
fn test_projections_structural_disparity() {
    let minspec = fs::read_to_string("src/stdlib/minspec.nl")
        .expect("Failed to read src/stdlib/minspec.nl");

    let harness = r#"
fn main() -> i64 {
    let interp: SpecExpr = make_interp_ast();
    let compiler: SpecExpr = specialize_compiler(interp);
    let cogen: SpecExpr = specialize_cogen();

    // Verify structural disparity across meta-programs
    let eq_interp_comp: bool = expr_eq(interp, compiler);
    let eq_comp_cogen: bool = expr_eq(compiler, cogen);
    let eq_interp_cogen: bool = expr_eq(interp, cogen);

    if eq_interp_comp { return 10; }
    if eq_comp_cogen { return 20; }
    if eq_interp_cogen { return 30; }

    return 42;
}
"#;
    let minspec_stripped = minspec.replacen("fn main()", "fn _orig_main()", 1);
    let combined = format!("{}\n{}", minspec_stripped, harness);
    let (code, stdout, stderr) = run_numlang_code(&combined);
    assert_eq!(
        code,
        Some(42),
        "Meta-programs across 1st, 2nd, and 3rd projections must be structurally distinct! stdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
}

#[test]
fn test_cogen_produces_compiler_which_produces_specialized_code() {
    let minspec = fs::read_to_string("src/stdlib/minspec.nl")
        .expect("Failed to read src/stdlib/minspec.nl");

    let harness = r#"
fn main() -> i64 {
    // 1. Generate cogen: cogen = min_spec(min_spec, min_spec)
    let cogen: SpecExpr = specialize_cogen();

    // 2. Generate compiler: compiler = cogen(interp)
    let interp: SpecExpr = make_interp_ast();
    let compiler: SpecExpr = run_cogen(cogen, interp);

    // 3. Compile program 1: specialized = compiler(1)
    let p3: SpecExpr = run_compiler(compiler, 1);

    // 4. Compare with 1st Futamura: p1 = min_spec(interp, 1)
    let p1: SpecExpr = first_futamura(1);

    if expr_eq(p1, p3) == false {
        return 10;
    }

    // 5. Evaluate target program: (5 + 10) * 2 = 30
    let e0: SpecEnv = SpecEnv::Nil;
    let b0: Box<SpecEnv> = box(e0);
    let env: SpecEnv = SpecEnv::Cons(1, 5, b0);
    let res: i64 = spec_eval(p3, env);

    if res != 30 {
        return 20;
    }

    return 42;
}
"#;
    let minspec_stripped = minspec.replacen("fn main()", "fn _orig_main()", 1);
    let combined = format!("{}\n{}", minspec_stripped, harness);
    let (code, stdout, stderr) = run_numlang_code(&combined);
    assert_eq!(
        code,
        Some(42),
        "Cogen must generate compiler which generates correct specialized code! stdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
}

#[test]
fn test_all_programs_parity_across_projections() {
    let minspec = fs::read_to_string("src/stdlib/minspec.nl")
        .expect("Failed to read src/stdlib/minspec.nl");

    let harness = r#"
fn main() -> i64 {
    // Check program 1: (x + 10) * 2 with x = 7 -> (7 + 10) * 2 = 34
    if verify_soundness(1, 7) == false { return 1; }

    // Check program 2: (x * 3) + 7 with x = 5 -> (5 * 3) + 7 = 22
    if verify_soundness(2, 5) == false { return 2; }

    // Check program 3: x * x with x = 9 -> 9 * 9 = 81
    if verify_soundness(3, 9) == false { return 3; }

    return 42;
}
"#;
    let minspec_stripped = minspec.replacen("fn main()", "fn _orig_main()", 1);
    let combined = format!("{}\n{}", minspec_stripped, harness);
    let (code, stdout, stderr) = run_numlang_code(&combined);
    assert_eq!(
        code,
        Some(42),
        "verify_soundness must hold across all 3 projection levels for all programs! stdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
}
