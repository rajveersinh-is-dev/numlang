//! Differential and structural-generality tests for the mutual index-nested
//! array recurrence pass (`recurrence::collapse_mutual_array_recurrences`).
//!
//! The closed form is deliberately checked against the *exact* source
//! recurrence over the whole domain, not against the commonly quoted (but
//! false for this recurrence) `floor(n/phi)` / `floor(n/phi^2)` formula.

use std::fs;
use std::process::Command;

use numlang::codegen::linker::link_executable;
use numlang::codegen::compile_supercompiled_to_obj;
use numlang::compiler::{distill_and_optimize, CompilerConfig};
use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::recurrence::{
    collapse_mutual_array_recurrences, mutual_recurrence_closed_form,
    simulate_mutual_recurrence, MutualSide,
};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn compile_and_run(src: &str, tag: &str) -> (i32, String) {
    let tokens = tokenize(src).expect("tokenize");
    let ast = parse(&tokens).expect("parse");
    let typed = typecheck(&ast).expect("typecheck");
    let obj = compile_supercompiled_to_obj(&typed).expect("supercompiled codegen");

    let dir = std::env::temp_dir();
    let stem = format!("nl_mutrec_{}_{}", tag, std::process::id());
    let obj_path = dir.join(format!("{}.obj", stem));
    let exe_path = dir.join(format!("{}.exe", stem));
    fs::write(&obj_path, obj).expect("write object");
    assert!(link_executable(&obj_path, &exe_path).is_ok());
    let out = Command::new(&exe_path).output().expect("run binary");
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    )
}

fn lower(src: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(src).expect("tokenize");
    let ast = parse(&tokens).expect("parse");
    let mut typed = typecheck(&ast).expect("typecheck");
    distill_and_optimize(
        &mut typed,
        &CompilerConfig {
            supercompile: true,
            ..Default::default()
        },
    );
    lower_program(&typed)
}

#[test]
fn closed_form_matches_direct_recurrence_for_n_0_to_200() {
    let (direct_first, direct_second) = simulate_mutual_recurrence(1, 0, 200);
    for n in 0..=200 {
        assert_eq!(
            mutual_recurrence_closed_form(n as i64, MutualSide::First),
            direct_first[n],
            "first sequence disagrees at n={n}"
        );
        assert_eq!(
            mutual_recurrence_closed_form(n as i64, MutualSide::Second),
            direct_second[n],
            "second sequence disagrees at n={n}"
        );
    }
    // The value produced by the benchmark source.
    assert_eq!(direct_second[100], 62);
    assert_eq!(mutual_recurrence_closed_form(100, MutualSide::Second), 62);
}

#[test]
fn naive_beatty_formula_is_not_the_source_recurrence() {
    // Documents why the pass uses the Fibonacci-corrected form instead of the
    // frequently quoted Beatty identity.
    let phi = (1.0f64 + 5.0f64.sqrt()) / 2.0;
    let (direct_first, direct_second) = simulate_mutual_recurrence(1, 0, 100);
    assert_eq!((100.0 / phi).floor() as i64, 61);
    assert_eq!((100.0 / (phi * phi)).floor() as i64, 38);
    assert_ne!(direct_second[100], (100.0 / phi).floor() as i64);
    assert_ne!(direct_second[100], (100.0 / (phi * phi)).floor() as i64);
    assert_ne!(direct_second[100], 100 - direct_first[100]);
    // Even the best simple shift has sporadic Fibonacci-indexed exceptions.
    let shifted = |n: i64| -> i64 { ((n + 1) as f64 / phi).floor() as i64 };
    let exceptions: Vec<i64> = (0..=100)
        .filter(|&n| direct_second[n as usize] != shifted(n))
        .collect();
    assert_eq!(exceptions, vec![1, 4, 12, 33, 88]);
}

#[test]
fn benchmark_source_still_prints_and_returns_the_same_answer() {
    let src = include_str!("../bench/showdown/numlang/hofstadter.nl");
    let (code, stdout) = compile_and_run(src, "hofstadter");
    assert_eq!(code, 62);
    assert_eq!(stdout, "62");
}

#[test]
fn pass_is_name_and_bound_agnostic() {
    // Same structural shape, different function/local/array names, different
    // array capacity and trip count.  No compiler logic may depend on the
    // benchmark's names, its 101-element arrays, the limit 100 or the answer 62.
    let capacity = 61;
    let limit = 60;
    let zeros = vec!["0"; capacity].join(", ");
    let src = format!(
        r#"
        fn zeta(bound: i64) -> i64 {{
            let mut p: [i64; {cap}] = [{zeros}];
            let mut q: [i64; {cap}] = [{zeros}];
            p[0] = 1;
            q[0] = 0;
            for j in 1..=bound {{
                let pv: i64 = p[j - 1];
                let qp: i64 = q[pv];
                p[j] = j - qp;
                let qv: i64 = q[j - 1];
                let pq: i64 = p[qv];
                q[j] = j - pq;
            }}
            return q[bound];
        }}
        fn main() -> i64 {{
            let r: i64 = zeta({limit});
            println(r);
            return r % 256;
        }}
        "#,
        cap = capacity,
        zeros = zeros,
        limit = limit,
    );

    let mut program = lower(&src);
    let rewrites = collapse_mutual_array_recurrences(&mut program);
    assert_eq!(rewrites, 1, "structurally identical shape must be detected");

    let expected = simulate_mutual_recurrence(1, 0, limit as usize).1[limit as usize] % 256;
    let (code, stdout) = compile_and_run(&src, "renamed");
    assert_eq!(code, expected as i32);
    assert_eq!(stdout, expected.to_string());
}

#[test]
fn pass_ignores_unrelated_programs() {
    let src = r#"
        fn plain(n: i64) -> i64 {
            let mut a: i64 = 0;
            let mut i: i64 = 0;
            while i < n {
                a = a + i;
                i = i + 1;
            }
            return a;
        }
        fn main() -> i64 {
            let r: i64 = plain(10);
            println(r);
            return r % 256;
        }
    "#;
    let mut program = lower(src);
    assert_eq!(collapse_mutual_array_recurrences(&mut program), 0);
}

#[test]
fn rewritten_function_has_no_array_or_loop() {
    let src = include_str!("../bench/showdown/numlang/hofstadter.nl");
    let mut program = lower(src);
    assert_eq!(collapse_mutual_array_recurrences(&mut program), 1);
    let func = program
        .functions
        .iter()
        .find(|f| f.name == "hofstadter_m")
        .expect("function");
    assert!(func.blocks.len() == 1, "closed form is a single block");
    assert!(
        !func
            .locals
            .iter()
            .any(|d| matches!(d.ty, numlang::typecheck::types::Type::Array(..))),
        "residual must not keep the sequence arrays"
    );
    assert!(
        func.blocks[0].statements.iter().any(
            |s| matches!(s, numlang::mir::lower::Statement::Assign(_, numlang::mir::lower::Rvalue::Call(name, _)) if name == "sqrt")
        ),
        "residual must compute the square root at run time"
    );
}
