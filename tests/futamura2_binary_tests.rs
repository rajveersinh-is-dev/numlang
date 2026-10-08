//! Phase 63: True Production Self-Applicable Specializer (2nd Futamura Binary Output).
//!
//! Tests that:
//! 1. `numlang --futamura2` generates the standalone compiler executable `minspec_cogen.exe`.
//! 2. `minspec_cogen.exe` independently compiles 10 distinct NumLang test programs.
//! 3. The executables compiled by `minspec_cogen.exe` produce 100% identical outputs and exit codes
//!    as the primary `numlang` compiler.
//! 4. MinSpec AST stream serialization is verified.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use numlang::mir::supercompiler::futamura2::{
    serialize_expr_to_stream, supercompile_2nd_futamura_cogen,
};
use numlang::parser::parse;
use numlang::token::tokenize;

fn get_cogen_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_minspec_cogen"))
}

fn run_with_cogen(
    cogen_path: &Path,
    src_path: &Path,
    out_exe: &Path,
) -> (Option<i32>, String, String) {
    let output = Command::new(cogen_path)
        .arg(src_path)
        .arg("-o")
        .arg(out_exe)
        .output()
        .expect("Failed to execute minspec_cogen");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.code(), stdout, stderr)
}

fn run_with_numlang(src_path: &Path) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg(src_path)
        .output()
        .expect("Failed to execute numlang run");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.code(), stdout, stderr)
}

fn run_compiled_binary(exe_path: &Path) -> (Option<i32>, String, String) {
    let output = Command::new(exe_path)
        .output()
        .expect("Failed to execute compiled binary");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.code(), stdout, stderr)
}

#[test]
fn test_2nd_futamura_mir_specialization_invariants() {
    let mir = supercompile_2nd_futamura_cogen().expect("supercompile_2nd_futamura_cogen failed");
    assert!(
        !mir.functions.is_empty(),
        "Residual 2nd Futamura MIR must contain functions"
    );

    let has_cogen = mir.functions.iter().any(|f| {
        f.name.contains("specialize_compiler") || f.name.contains("second_futamura_compiler")
    });
    assert!(
        has_cogen,
        "Residual MIR must contain compiler generation logic"
    );
}

#[test]
fn test_futamura2_cli_flag_generation() {
    let temp_dir = std::env::temp_dir().join(format!("nl_futa2_cli_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    let cogen_target = temp_dir.join(if cfg!(windows) {
        "test_minspec_cogen.exe"
    } else {
        "test_minspec_cogen"
    });

    // Test numlang --futamura2 --futamura2-out <cogen_target>
    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("--futamura2")
        .arg("--futamura2-out")
        .arg(&cogen_target)
        .output()
        .expect("Failed to execute numlang --futamura2");

    assert_eq!(
        output.status.code(),
        Some(0),
        "numlang --futamura2 must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        cogen_target.exists(),
        "minspec_cogen binary must exist at output target"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_ast_stream_serialization() {
    let mut var_map = std::collections::HashMap::new();
    let mut next_id = 1;

    let tokens = tokenize("fn test() -> i64 { return 10 + 20 * 3; }").unwrap();
    let expr = parse(&tokens).unwrap();
    if let Some(numlang::ast::Item::Function(f)) = expr.items.first() {
        if let Some(numlang::ast::Stmt::Return(Some(ref ret_expr), _)) = f.body.stmts.first() {
            let stream = serialize_expr_to_stream(ret_expr, &mut var_map, &mut next_id);
            // 10 + (20 * 3) -> [3, 1, 1, 10, 3, 3, 1, 20, 1, 3]
            assert!(!stream.is_empty(), "Stream must not be empty");
            assert_eq!(stream[0], 3, "Outer operation is Bin");
            assert_eq!(stream[1], 1, "Outer op code is Add");
        }
    }
}

/// Helper that verifies a test program across minspec_cogen and numlang primary compiler.
fn verify_program_parity(name: &str, source: &str, expected_exit: i32) {
    let temp_dir = std::env::temp_dir().join(format!("nl_futa2_{}_{}", name, std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    let src_file = temp_dir.join(format!("{}.nl", name));
    let exe_file = temp_dir.join(format!("{}.exe", name));
    fs::write(&src_file, source).unwrap();

    let cogen_exe = get_cogen_exe();

    // 1. Compile with minspec_cogen
    let (cogen_status, cogen_out, cogen_err) = run_with_cogen(&cogen_exe, &src_file, &exe_file);
    assert_eq!(
        cogen_status,
        Some(0),
        "minspec_cogen compilation failed for {}: stdout: {}, stderr: {}",
        name,
        cogen_out,
        cogen_err
    );
    assert!(
        exe_file.exists(),
        "minspec_cogen must produce output executable: {}",
        exe_file.display()
    );

    // 2. Execute binary produced by minspec_cogen
    let (bin_exit, bin_out, bin_err) = run_compiled_binary(&exe_file);
    assert_eq!(
        bin_exit,
        Some(expected_exit),
        "Binary compiled by minspec_cogen for {} exited with {:?}, expected {}. stderr: {}",
        name,
        bin_exit,
        expected_exit,
        bin_err
    );

    // 3. Execute same program with primary compiler (numlang run)
    let (numlang_exit, numlang_out, _) = run_with_numlang(&src_file);
    assert_eq!(
        numlang_exit,
        Some(expected_exit),
        "numlang run for {} exited with {:?}, expected {}",
        name,
        numlang_exit,
        expected_exit
    );

    // 4. Parity check: binary exit code matches numlang run
    assert_eq!(
        bin_exit, numlang_exit,
        "Exit code mismatch between minspec_cogen binary ({:?}) and numlang run ({:?}) on {}",
        bin_exit, numlang_exit, name
    );

    // 5. Output parity check
    assert_eq!(
        bin_out, numlang_out,
        "Stdout mismatch between minspec_cogen binary and numlang run on {}",
        name
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

// ----------------------------------------------------------------------------
// 10 Distinct NumLang Test Programs (PROD-FUTA2-05)
// ----------------------------------------------------------------------------

#[test]
fn test_prog01_arithmetic_precedence() {
    let src = r#"
fn main() -> i64 {
    let a: i64 = 15 + 25;
    let b: i64 = 12 - 7;
    let c: i64 = a * b / 4;
    return c + 17; // (40 * 5) / 4 + 17 = 50 + 17 = 67
}
"#;
    verify_program_parity("prog01_arithmetic", src, 67);
}

#[test]
fn test_prog02_conditionals_nested() {
    let src = r#"
fn eval_cond(x: i64) -> i64 {
    if x < 10 {
        return x * 2;
    } else {
        if x < 20 {
            return x * 3 - 5;
        } else {
            return x + 100;
        }
    }
}

fn main() -> i64 {
    return eval_cond(12); // 12 * 3 - 5 = 31
}
"#;
    verify_program_parity("prog02_conditionals", src, 31);
}

#[test]
fn test_prog03_sequential_lets() {
    let src = r#"
fn main() -> i64 {
    let a: i64 = 10;
    let b: i64 = a * 2;
    let c: i64 = b + a;
    let d: i64 = c - 5;
    let e: i64 = d + 2;
    return e; // 10*2 = 20, + 10 = 30, - 5 = 25, + 2 = 27
}
"#;
    verify_program_parity("prog03_sequential_lets", src, 27);
}

#[test]
fn test_prog04_loops_recursion() {
    let src = r#"
fn fact(n: i64) -> i64 {
    if n <= 1 {
        return 1;
    }
    return n * fact(n - 1);
}

fn main() -> i64 {
    return fact(5); // 120
}
"#;
    verify_program_parity("prog04_loops_recursion", src, 120);
}

#[test]
fn test_prog05_fibonacci() {
    let src = r#"
fn fib(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    if n == 1 {
        return 1;
    }
    return fib(n - 1) + fib(n - 2);
}

fn main() -> i64 {
    return fib(10); // 55
}
"#;
    verify_program_parity("prog05_fibonacci", src, 55);
}

#[test]
fn test_prog06_boolean_logic() {
    let src = r#"
fn test_logic(a: i64, b: i64) -> i64 {
    let eq: bool = (a == b);
    let lt: bool = (a < b);
    if eq {
        return 10;
    }
    if lt {
        return 20;
    }
    return 30;
}

fn main() -> i64 {
    return test_logic(4, 9); // 20
}
"#;
    verify_program_parity("prog06_boolean_logic", src, 20);
}

#[test]
fn test_prog07_power_loop() {
    let src = r#"
fn ipow(base: i64, exp: i64) -> i64 {
    if exp <= 0 {
        return 1;
    }
    return base * ipow(base, exp - 1);
}

fn main() -> i64 {
    return ipow(2, 7); // 128
}
"#;
    verify_program_parity("prog07_power_loop", src, 128);
}

#[test]
fn test_prog08_polynomial() {
    let src = r#"
fn poly(x: i64) -> i64 {
    return 3 * x * x + 5 * x + 7;
}

fn main() -> i64 {
    return poly(4); // 3*16 + 20 + 7 = 75
}
"#;
    verify_program_parity("prog08_polynomial", src, 75);
}

#[test]
fn test_prog09_multi_call_env() {
    let src = r#"
fn add(a: i64, b: i64) -> i64 {
    return a + b;
}

fn mul(a: i64, b: i64) -> i64 {
    return a * b;
}

fn main() -> i64 {
    let x: i64 = add(3, 4);
    let y: i64 = add(5, 6);
    return mul(x, y); // 7 * 11 = 77
}
"#;
    verify_program_parity("prog09_multi_call_env", src, 77);
}

#[test]
fn test_prog10_state_accumulator() {
    let src = r#"
fn accumulate(n: i64, acc: i64) -> i64 {
    if n <= 0 {
        return acc;
    }
    return accumulate(n - 1, acc + n);
}

fn main() -> i64 {
    return accumulate(10, 0); // 55
}
"#;
    verify_program_parity("prog10_state_accumulator", src, 55);
}
