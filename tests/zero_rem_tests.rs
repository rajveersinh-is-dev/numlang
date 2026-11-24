//! Correctness and structural tests for zero-test remainder strength reduction.
//!
//! `(x % 2^k) == 0` is equivalent to `(x & (2^k - 1)) == 0` for every signed
//! and unsigned `x`, including negative values.  The pass must apply to that
//! shape only: computing `x % 2^k` as a value must stay a remainder for
//! negative `x`, where truncated remainder and bitmask differ.

use numlang::ast::BinaryOp;
use numlang::compiler::{distill_and_optimize, CompilerConfig};
use numlang::mir::lower::{lower_program, Rvalue};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use std::fs;
use std::process::Command;

fn run_code(src: &str, name: &str) -> i32 {
    let dir = std::env::temp_dir().join("numlang_zero_rem_tests");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join(format!("{name}.nl"));
    let exe = dir.join(format!("{name}.exe"));
    fs::write(&source, src).expect("write source");
    let build = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("build")
        .arg(&source)
        .arg("--no-cache")
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("build");
    assert!(build.status.success(), "build failed: {}", String::from_utf8_lossy(&build.stderr));
    let out = Command::new(&exe).output().expect("run");
    out.status.code().expect("exit code")
}

fn residual(src: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(src).expect("tokenize");
    let ast = parse(&tokens).expect("parse");
    let mut typed = typecheck(&ast).expect("typecheck");
    distill_and_optimize(
        &mut typed,
        &CompilerConfig {
            ho_distill: true,
            supercompile: true,
            ..CompilerConfig::default()
        },
    );
    lower_program(&typed)
}

fn count_ops(mir: &numlang::mir::lower::MirProgram, op: BinaryOp) -> usize {
    mir.functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .flat_map(|b| b.statements.iter())
        .filter(|s| matches!(s, numlang::mir::lower::Statement::Assign(_, Rvalue::BinaryOp(o, _, _)) if *o == op))
        .count()
}

#[test]
fn negative_and_positive_zero_tests_match_rust_semantics() {
    // Independent Rust computation of the expected conditions.
    let values: [i64; 8] = [-17, -16, -8, -1, 0, 1, 8, 256];
    let mut expected = 0i64;
    for (i, &x) in values.iter().enumerate() {
        if (x % 2) == 0 {
            expected += 1;
        }
        if 0 == (x % 4) {
            expected += 2;
        }
        if (x % 16) != 0 {
            expected += 4;
        }
        if 256 != (x % 256) {
            expected += 8;
        }
        expected += (i as i64) % 3;
    }

    let src = format!(
        r#"
        fn main() -> i64 {{
            let values: [i64; 8] = [{}, {}, {}, {}, {}, {}, {}, {}];
            let mut acc: i64 = 0;
            let mut i: i64 = 0;
            while i < 8 {{
                let x: i64 = values[i];
                if (x % 2) == 0 {{ acc = acc + 1; }}
                if 0 == (x % 4) {{ acc = acc + 2; }}
                if (x % 16) != 0 {{ acc = acc + 4; }}
                if 256 != (x % 256) {{ acc = acc + 8; }}
                acc = acc + (i % 3);
                i = i + 1;
            }}
            println(acc);
            return acc % 251;
        }}
        "#,
        values[0], values[1], values[2], values[3], values[4], values[5], values[6], values[7]
    );
    // The oracle value is computed by the same Rust semantics above; the
    // exit code is that value modulo 251 (the program's own final modulus).
    assert_eq!(run_code(&src, "zero_rem_semantics"), (expected % 251) as i32);
}

#[test]
fn value_modulo_is_not_rewritten() {
    let src = r#"
        fn main() -> i64 {
            let x: i64 = -9;
            let rem: i64 = x % 8;   // value use: -9 % 8 == -1, not 7
            if rem != -1 { return 1; }
            if (x % 8) == 0 { return 2; }  // zero test: false for -9
            return 42;
        }
    "#;
    assert_eq!(run_code(src, "zero_rem_value_use"), 42);
    let mir = residual(src);
    // At least one real Mod must remain for the value computation.
    assert!(count_ops(&mir, BinaryOp::Mod) >= 1, "value modulo was rewritten: {mir:#?}");
    // And the zero test must have become a bitmask.
    assert!(count_ops(&mir, BinaryOp::BitAnd) >= 1, "zero test not reduced: {mir:#?}");
}

#[test]
fn rewrite_is_independent_of_identifiers() {
    let a = r#"
        fn f(value: i64) -> i64 {
            if (value % 16) == 0 { return 1; }
            return 0;
        }
        fn main() -> i64 { let answer: i64 = f(32); println(answer); return answer; }
    "#;
    let b = r#"
        fn g(quux: i64) -> i64 {
            if 0 == (quux % 16) { return 1; }
            return 0;
        }
        fn main() -> i64 { let result: i64 = g(32); println(result); return result; }
    "#;
    let ma = residual(a);
    let mb = residual(b);
    assert_eq!(count_ops(&ma, BinaryOp::BitAnd), count_ops(&mb, BinaryOp::BitAnd));
    assert_eq!(count_ops(&ma, BinaryOp::Mod), count_ops(&mb, BinaryOp::Mod));
    assert_eq!(run_code(a, "zero_rem_alpha_a"), run_code(b, "zero_rem_alpha_b"));
}
