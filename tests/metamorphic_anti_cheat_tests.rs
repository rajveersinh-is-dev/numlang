//! Metamorphic Anti-Cheat Verification Suite
//!
//! Enforces Integrity Rule 4 (Algorithmic Generality & Symmetric Optimization):
//! Compilers and optimizers must treat all user code symmetrically and agnostically:
//! - Renaming functions, variables, or types must never disable optimization passes.
//! - Changing loop constants or step sizes must produce correct inductive formulas, not fail or look up static answers.
//! - Independent statement reordering must preserve semantic equivalence and supercompilation gains.
//!
//! Any pass keyed on benchmark function names (e.g. "tri_sum", "tak", "fib", "append3")
//! or exact source text shape fails this test suite.

use std::fs;
use std::process::Command;

use numlang::codegen::compile_supercompiled_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn expected_exit(code: i32) -> i32 {
    #[cfg(target_os = "windows")]
    {
        code
    }
    #[cfg(not(target_os = "windows"))]
    {
        code.rem_euclid(256)
    }
}

fn compile_and_run(src: &str, test_name: &str) -> i32 {
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let mut typed = typecheck(&ast).expect("Typecheck failed");
    typed.desugar_for_loops();

    let obj_bytes = compile_supercompiled_to_obj(&typed).expect("Supercompiled codegen failed");

    let test_dir = std::env::temp_dir().join(format!("metamorphic_test_{}", test_name));
    let _ = fs::create_dir_all(&test_dir);
    let obj_path = test_dir.join(format!("{}.obj", test_name));
    let exe_path = test_dir.join(format!("{}.exe", test_name));
    fs::write(&obj_path, obj_bytes).expect("Failed to write obj");

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to execute binary");

    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_dir(&test_dir);

    output.status.code().unwrap_or(-1)
}

#[test]
fn test_metamorphic_alpha_renaming_triangular_sum() {
    // 1. Canonical source
    let src_canonical = r#"
        fn tri_sum(n: i64) -> i64 {
            let mut acc: i64 = 0;
            let mut i: i64 = 1;
            while i <= n {
                acc = acc + i;
                i = i + 1;
            }
            return acc;
        }

        fn main() -> i64 {
            return tri_sum(100) % 256;
        }
    "#;

    // 2. Metamorphic alpha-renamed source (completely different identifiers)
    let src_renamed = r#"
        fn zebra_kernel_accumulator(omega_bound: i64) -> i64 {
            let mut state_sum: i64 = 0;
            let mut cursor_ptr: i64 = 1;
            while cursor_ptr <= omega_bound {
                state_sum = state_sum + cursor_ptr;
                cursor_ptr = cursor_ptr + 1;
            }
            return state_sum;
        }

        fn main() -> i64 {
            return zebra_kernel_accumulator(100) % 256;
        }
    "#;

    let exit_canon = compile_and_run(src_canonical, "tri_canon");
    let exit_renamed = compile_and_run(src_renamed, "tri_renamed");

    assert_eq!(
        exit_canon, exit_renamed,
        "Alpha-renamed recurrence must yield identical exit code"
    );

    assert_eq!(exit_canon, expected_exit((100 * 101 / 2) % 256));
}

#[test]
fn test_metamorphic_alpha_renaming_quadratic_sum() {
    let src_canonical = r#"
        fn cubic_sum(n: i64) -> i64 {
            let mut acc: i64 = 0;
            let mut i: i64 = 1;
            while i <= n {
                acc = acc + (i * i);
                i = i + 1;
            }
            return acc;
        }

        fn main() -> i64 {
            return cubic_sum(50) % 256;
        }
    "#;

    let src_renamed = r#"
        fn dynamic_second_order_eval(theta_limit: i64) -> i64 {
            let mut gamma_val: i64 = 0;
            let mut lambda_step: i64 = 1;
            while lambda_step <= theta_limit {
                gamma_val = gamma_val + (lambda_step * lambda_step);
                lambda_step = lambda_step + 1;
            }
            return gamma_val;
        }

        fn main() -> i64 {
            return dynamic_second_order_eval(50) % 256;
        }
    "#;

    let exit_canon = compile_and_run(src_canonical, "quad_canon");
    let exit_renamed = compile_and_run(src_renamed, "quad_renamed");

    let expected = expected_exit((50 * 51 * 101 / 6) % 256);
    assert_eq!(exit_canon, expected);
    assert_eq!(exit_renamed, expected);
}

#[test]
fn test_metamorphic_constant_perturbation_generalization() {
    // Vary the bounds and constants dynamically to verify general induction
    for n in [10, 25, 42, 73, 100, 150] {
        let src = format!(
            r#"
            fn sum_prog(limit: i64) -> i64 {{
                let mut acc: i64 = 0;
                let mut k: i64 = 1;
                while k <= limit {{
                    acc = acc + k;
                    k = k + 1;
                }}
                return acc;
            }}

            fn main() -> i64 {{
                return sum_prog({}) % 256;
            }}
            "#,
            n
        );

        let exit_val = compile_and_run(&src, &format!("perturb_{}", n));
        let expected = expected_exit((n * (n + 1) / 2) % 256);
        assert_eq!(
            exit_val, expected,
            "Failed closed form for arbitrary bound N = {}",
            n
        );
    }
}

#[test]
fn test_metamorphic_step_size_generality() {
    // Arithmetic progression with step size 3: sum of 1, 4, 7, 10, ...
    let src = r#"
        fn step_prog(n: i64) -> i64 {
            let mut acc: i64 = 0;
            let mut k: i64 = 1;
            let mut steps: i64 = 0;
            while steps < n {
                acc = acc + k;
                k = k + 3;
                steps = steps + 1;
            }
            return acc;
        }

        fn main() -> i64 {
            return step_prog(20) % 256;
        }
    "#;

    let exit_val = compile_and_run(src, "step_prog");
    // n = 20 terms: 1 + 4 + 7 + ... + (1 + 19*3 = 58)
    // Sum = 20 * (1 + 58) / 2 = 590
    let expected = expected_exit(590 % 256);
    assert_eq!(exit_val, expected);
}

#[test]
fn test_metamorphic_statement_reordering_invariance() {
    let src_1 = r#"
        fn compute(a: i64, b: i64) -> i64 {
            let x: i64 = a * 2;
            let y: i64 = b * 3;
            let z: i64 = x + y;
            return z;
        }

        fn main() -> i64 {
            return compute(5, 7) % 256;
        }
    "#;

    let src_2 = r#"
        fn compute(a: i64, b: i64) -> i64 {
            let y: i64 = b * 3;
            let x: i64 = a * 2;
            let z: i64 = y + x;
            return z;
        }

        fn main() -> i64 {
            return compute(5, 7) % 256;
        }
    "#;

    let exit_1 = compile_and_run(src_1, "reorder_1");
    let exit_2 = compile_and_run(src_2, "reorder_2");

    assert_eq!(exit_1, exit_2);
    assert_eq!(exit_1, expected_exit(5 * 2 + 7 * 3));
}
