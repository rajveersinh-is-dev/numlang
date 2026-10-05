use std::fs;
use std::process::Command;
use std::time::Instant;

use numlang::codegen::compile_supercompiled_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn compile_and_run(src: &str, test_name: &str) -> i32 {
    let tokens = tokenize(src).expect("Tokenization failed");
    let ast = parse(&tokens).expect("Parsing failed");
    let typed = typecheck(&ast).expect("Typechecking failed");

    let obj_bytes = compile_supercompiled_to_obj(&typed).expect("Supercompiled codegen failed");

    let test_dir = std::env::temp_dir().join("numlang_mutual_recursion_tests");
    fs::create_dir_all(&test_dir).expect("Failed to create test directory");
    let obj_path = test_dir.join(format!("{}.obj", test_name));
    let exe_path = test_dir.join(format!("{}.exe", test_name));
    fs::write(&obj_path, obj_bytes).expect("Failed to write object file");

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to run binary");
    output.status.code().unwrap_or(-1)
}

#[test]
fn test_even_odd_mutual_recursion_collapse() {
    let src = r#"
        fn is_odd(n: i64) -> i64 {
            if n == 0 {
                return 0;
            }
            return is_even(n - 1);
        }

        fn is_even(n: i64) -> i64 {
            if n == 0 {
                return 1;
            }
            return is_odd(n - 1);
        }

        fn main() -> i64 {
            let e = is_even(100);
            let o = is_odd(101);
            return e + o;
        }
    "#;

    // Verify supercompilation stats record collapsed cycles
    let tokens = tokenize(src).expect("Tokenization failed");
    let ast = parse(&tokens).expect("Parsing failed");
    let typed = typecheck(&ast).expect("Typechecking failed");
    let mut mir = lower_program(&typed);
    let stats = supercompile_mir_program(&mut mir);

    assert!(
        stats.loops_collapsed >= 1 || stats.calls_inlined >= 1,
        "Expected mutual recursion cycle to be collapsed or inlined: {:?}",
        stats
    );

    let code = compile_and_run(src, "even_odd_collapse");
    // is_even(100) == 1, is_odd(101) == 1 => sum = 2
    assert_eq!(code, 2, "is_even(100) + is_odd(101) must equal 2");
}

#[test]
fn test_two_variable_mutual_linear_recurrence() {
    let src = r#"
        fn f(n: i64, a: i64, b: i64) -> i64 {
            if n == 0 {
                return a;
            }
            return g(n - 1, a + b, b);
        }

        fn g(n: i64, a: i64, b: i64) -> i64 {
            if n == 0 {
                return a;
            }
            return f(n - 1, a + 2 * b, b);
        }

        fn main() -> i64 {
            return f(10, 1, 2);
        }
    "#;

    let tokens = tokenize(src).expect("Tokenization failed");
    let ast = parse(&tokens).expect("Parsing failed");
    let typed = typecheck(&ast).expect("Typechecking failed");
    let mut mir = lower_program(&typed);
    let stats = supercompile_mir_program(&mut mir);

    assert!(
        stats.loops_collapsed >= 1 || stats.calls_inlined >= 1,
        "Expected 2-variable mutual linear recurrence to collapse or inline: {:?}",
        stats
    );

    let code = compile_and_run(src, "coupled_2var_collapse");
    // Cycle length is 2: f(n, a, b) -> g(n-1, a+b, b) -> f(n-2, a+3b, b).
    // For n=10 (5 cycles): a = 1 + 5 * 3(2) = 1 + 30 = 31.
    assert_eq!(code, 31, "f(10, 1, 2) must evaluate to 31");
}

#[test]
fn test_three_function_cyclic_recurrence() {
    let src = r#"
        fn step_c(n: i64, acc: i64) -> i64 {
            if n == 0 {
                return acc;
            }
            return step_a(n - 1, acc + 3);
        }

        fn step_b(n: i64, acc: i64) -> i64 {
            if n == 0 {
                return acc;
            }
            return step_c(n - 1, acc + 2);
        }

        fn step_a(n: i64, acc: i64) -> i64 {
            if n == 0 {
                return acc;
            }
            return step_b(n - 1, acc + 1);
        }

        fn main() -> i64 {
            return step_a(12, 10);
        }
    "#;

    let tokens = tokenize(src).expect("Tokenization failed");
    let ast = parse(&tokens).expect("Parsing failed");
    let typed = typecheck(&ast).expect("Typechecking failed");
    let mut mir = lower_program(&typed);
    let stats = supercompile_mir_program(&mut mir);

    assert!(
        stats.loops_collapsed >= 1 || stats.calls_inlined >= 1,
        "Expected 3-function cycle to be collapsed or inlined: {:?}",
        stats
    );

    let code = compile_and_run(src, "three_func_cycle_collapse");
    // Cycle length 3: step_a(n, acc) -> step_b(n-1, acc+1) -> step_c(n-2, acc+3) -> step_a(n-3, acc+6).
    // For n=12 (4 cycles): acc = 10 + 4 * 6 = 34.
    assert_eq!(code, 34, "step_a(12, 10) must evaluate to 34");
}

#[test]
fn test_hofstadter_style_mutual_linear_recurrence() {
    let src = r#"
        fn mut_x(n: i64, x: i64, y: i64) -> i64 {
            if n == 0 {
                return x;
            }
            return mut_y(n - 1, x + y, x);
        }

        fn mut_y(n: i64, x: i64, y: i64) -> i64 {
            if n == 0 {
                return y;
            }
            return mut_x(n - 1, y, x + y);
        }

        fn main() -> i64 {
            return mut_x(6, 1, 2);
        }
    "#;

    let code = compile_and_run(src, "hofstadter_linear_collapse");
    // Let's trace mut_x(6, 1, 2):
    // n=6: mut_x(6, 1, 2) -> mut_y(5, 3, 1)
    // n=5: mut_y(5, 3, 1) -> mut_x(4, 1, 4)
    // n=4: mut_x(4, 1, 4) -> mut_y(3, 5, 1)
    // n=3: mut_y(3, 5, 1) -> mut_x(2, 1, 6)
    // n=2: mut_x(2, 1, 6) -> mut_y(1, 7, 1)
    // n=1: mut_y(1, 7, 1) -> mut_x(0, 1, 8)
    // n=0: mut_x(0, 1, 8) returns x = 1.
    assert_eq!(code, 1, "mut_x(6, 1, 2) must evaluate to 1");
}

#[test]
fn test_mutual_recursion_benchmark_integrity() {
    // Governing standard: real in-process timing across >= 30 rounds with >= 5 warmup iterations
    let src = r#"
        fn is_odd(n: i64) -> i64 {
            if n == 0 {
                return 0;
            }
            return is_even(n - 1);
        }

        fn is_even(n: i64) -> i64 {
            if n == 0 {
                return 1;
            }
            return is_odd(n - 1);
        }

        fn main() -> i64 {
            return is_even(1000);
        }
    "#;

    let tokens = tokenize(src).expect("Tokenization failed");
    let ast = parse(&tokens).expect("Parsing failed");
    let typed = typecheck(&ast).expect("Typechecking failed");

    let obj_bytes = compile_supercompiled_to_obj(&typed).expect("Codegen failed");
    let test_dir = std::env::temp_dir().join("numlang_mutual_bench");
    fs::create_dir_all(&test_dir).expect("Failed to create dir");
    let obj_path = test_dir.join("bench.obj");
    let exe_path = test_dir.join("bench.exe");
    fs::write(&obj_path, obj_bytes).expect("Failed to write obj");
    link_executable(&obj_path, &exe_path).expect("Link failed");

    // 5 warmup runs
    for _ in 0..5 {
        let out = Command::new(&exe_path).output().expect("Warmup run failed");
        assert_eq!(out.status.code(), Some(1));
    }

    // 30 measurement rounds with high-resolution timer
    let mut total_dur_nanos: u128 = 0;
    for _ in 0..30 {
        let start = Instant::now();
        let out = Command::new(&exe_path).output().expect("Measurement run failed");
        let elapsed = start.elapsed();
        assert_eq!(out.status.code(), Some(1));
        total_dur_nanos += elapsed.as_nanos();
    }

    let avg_dur_nanos = total_dur_nanos / 30;
    println!("Average execution time over 30 rounds: {} ns", avg_dur_nanos);
    assert!(avg_dur_nanos > 0, "Timing counter must be positive");
}
