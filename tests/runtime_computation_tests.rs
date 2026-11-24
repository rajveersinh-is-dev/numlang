//! Integrity regression: a self-contained constant-input program must be
//! emitted as executable computation, not as the final answer baked into MIR.
//!
//! The guard is whole-program and structural: a program with no runtime input
//! (`read_i64`, opaque main parameters, ...) may not have its computation
//! constant-evaluated by the compiler.  This test uses a tiny sum 1..10 kernel
//! so the answer (55) is easy to spot in the residual MIR.
//!
//! In addition to the library-level pipeline check,
//! `cli_driver_guard_keeps_closed_program_computed` invokes the real `numlang`
//! binary with `--supercompile --mode distill` and verifies that the driver
//! process itself keeps the arithmetic in the emitted residual MIR (and does
//! not emit 55 as a literal).

use numlang::compiler::{distill_and_optimize, CompilerConfig};
use numlang::mir::lower::{lower_program, Rvalue, Statement};
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typed_ast::TypedLiteral;
use numlang::typecheck::typecheck;
use std::fs;
use std::process::Command;

const SUM_1_TO_10: &str = r#"
fn sum_1_to_n(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc + i;
        i = i + 1;
    }
    return acc;
}

fn main() -> i64 {
    let res: i64 = sum_1_to_n(10);
    println(res);
    return res % 256;
}
"#;

fn residual_mir(source: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(source).expect("tokenize");
    let parsed = parse(&tokens).expect("parse");
    let mut typed = typecheck(&parsed).expect("typecheck");
    let previous_guard = numlang::runtime_input::set_ast_runtime_computation_guard(true);
    distill_and_optimize(
        &mut typed,
        &CompilerConfig {
            ho_distill: true,
            supercompile: true,
            ..CompilerConfig::default()
        },
    );
    numlang::runtime_input::set_ast_runtime_computation_guard(previous_guard);
    let mut mir = lower_program(&typed);
    // The real compiler driver sets this marker before supercompilation.
    mir.enforce_runtime_computation = true;
    supercompile_mir_program(&mut mir);
    mir
}

#[test]
fn constant_input_program_computes_at_runtime() {
    let mir = residual_mir(SUM_1_TO_10);
    let statements: Vec<&Statement> = mir
        .functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .flat_map(|b| b.statements.iter())
        .collect();

    // The final answer must never appear as a standalone literal.
    let emits_answer_literal = statements.iter().any(|stmt| {
        matches!(
            stmt,
            Statement::Assign(_, Rvalue::Constant(TypedLiteral::Int(55, _)))
        )
    });
    assert!(
        !emits_answer_literal,
        "closed constant-input program was folded to its answer literal 55:\n{mir:#?}"
    );

    // It must contain arithmetic (or a loop) that executes on every run.
    let has_arithmetic = statements.iter().any(|stmt| {
        matches!(
            stmt,
            Statement::Assign(_, Rvalue::BinaryOp(_, _, _))
        )
    });
    let has_loop = mir.functions.iter().any(|f| {
        f.blocks
            .iter()
            .any(|b| matches!(b.terminator, numlang::mir::Terminator::BranchIf { .. }))
    });
    assert!(
        has_arithmetic || has_loop,
        "residual MIR has neither arithmetic nor a loop:\n{mir:#?}"
    );
}

#[test]
fn constant_input_program_still_returns_the_same_answer() {
    let dir = std::env::temp_dir().join(format!(
        "numlang_runtime_computation_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let src = dir.join("sum.nl");
    std::fs::write(&src, SUM_1_TO_10).expect("write source");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg("--supercompile")
        .arg("--no-cache")
        .arg(&src)
        .output()
        .expect("run numlang");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(output.status.code(), Some(55), "unexpected run: {output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).trim_start().starts_with("55"),
        "unexpected stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// True when the pretty-printed residual MIR contains `Int(<value>, ...)`.
/// The driver prints `MirProgram` with `{:#?}`, so an integer literal occupies
/// its own line immediately after an `Int(` line.
fn emitted_int_literal(mir_dump: &str, value: i64) -> bool {
    let lines: Vec<&str> = mir_dump.lines().collect();
    lines.iter().enumerate().any(|(i, line)| {
        line.trim() == "Int("
            && lines
                .get(i + 1)
                .is_some_and(|next| next.trim().trim_end_matches(',') == value.to_string())
    })
}

/// The guard in `src/runtime_input.rs` must be active in the *real CLI driver*,
/// not only in library callers that manually set it. This test builds the tiny
/// closed `SUM_1_TO_10` program with the actual `numlang` binary using the
/// exact `--supercompile --mode distill` flags and inspects the residual MIR
/// the driver prints: it must still contain arithmetic and must not contain
/// the independently known answer (55) as a literal.
#[test]
fn cli_driver_guard_keeps_closed_program_computed() {
    let dir = std::env::temp_dir().join(format!(
        "numlang_cli_guard_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    let src = dir.join("sum.nl");
    fs::write(&src, SUM_1_TO_10).expect("write source");

    let emitted = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg(&src)
        .arg("--supercompile")
        .arg("--mode")
        .arg("distill")
        .arg("--no-cache")
        .arg("--emit-supercompiled-mir")
        .output()
        .expect("spawn numlang --emit-supercompiled-mir");
    let dump = String::from_utf8_lossy(&emitted.stdout);
    assert!(
        emitted.status.success(),
        "driver failed: {}",
        String::from_utf8_lossy(&emitted.stderr)
    );
    assert!(
        dump.contains("BinaryOp("),
        "CLI driver folded the closed program to a constant; residual MIR has no arithmetic:\n{dump}"
    );
    assert!(
        !emitted_int_literal(&dump, 55),
        "CLI driver emitted the independently computed answer 55 as a literal:\n{dump}"
    );

    // The same driver invocation must still compute the correct result.
    let run = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg("--supercompile")
        .arg("--mode")
        .arg("distill")
        .arg("--no-cache")
        .arg(&src)
        .output()
        .expect("spawn numlang run");
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(run.status.code(), Some(55), "wrong run exit code: {run:?}");
    assert!(
        String::from_utf8_lossy(&run.stdout)
            .trim_start()
            .starts_with("55"),
        "wrong run stdout: {}",
        String::from_utf8_lossy(&run.stdout)
    );
}
