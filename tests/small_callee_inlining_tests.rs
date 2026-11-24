//! Structural tests for the small-pure-callee inlining pass in strength reduction.
//!
//! The pass copies a leaf function's arithmetic and guards into its caller so
//! the native backend can optimize them together.  It must never evaluate the
//! program at compile time: the residual MIR has to keep loops, arithmetic and
//! branches, and recursive callees must be left alone.

use numlang::codegen::compile_supercompiled_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::compiler::{distill_and_optimize, CompilerConfig};
use numlang::mir::lower::{lower_program, Rvalue, Statement};
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use std::fs;

fn residual(src: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(src).expect("tokenize");
    let ast = parse(&tokens).expect("parse");
    let mut typed = typecheck(&ast).expect("typecheck");
    numlang::runtime_input::set_ast_runtime_computation_guard(true);
    distill_and_optimize(
        &mut typed,
        &CompilerConfig {
            ho_distill: true,
            supercompile: true,
            ..Default::default()
        },
    );
    let mut mir = lower_program(&typed);
    mir.enforce_runtime_computation = true;
    supercompile_mir_program(&mut mir);
    mir
}

fn count_calls(mir: &numlang::mir::lower::MirProgram, name: &str) -> usize {
    mir.functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .flat_map(|b| b.statements.iter())
        .filter(|s| matches!(s, Statement::Assign(_, Rvalue::Call(n, _)) if n == name))
        .count()
}

fn run(src: &str, tag: &str) -> (i32, String) {
    // Use the same integrity policy as the compiler driver: closed programs
    // must keep their computation executable.
    numlang::runtime_input::mark_compiler_driver();
    numlang::runtime_input::set_ast_runtime_computation_guard(true);
    let tokens = tokenize(src).expect("tokenize");
    let ast = parse(&tokens).expect("parse");
    let typed = typecheck(&ast).expect("typecheck");
    let obj = compile_supercompiled_to_obj(&typed).expect("codegen");
    let dir = std::env::temp_dir();
    let stem = format!("nl_inline_{}_{}", tag, std::process::id());
    let obj_path = dir.join(format!("{}.obj", stem));
    let exe_path = dir.join(format!("{}.exe", stem));
    fs::write(&obj_path, &obj).expect("write obj");
    link_executable(&obj_path, &exe_path).expect("link");
    let out = std::process::Command::new(&exe_path).output().expect("run");
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    )
}

const LEAF_CALLER: &str = r#"
    fn bump(v: i64, k: i64) -> i64 {
        let t: i64 = v * k;
        if t < 0 { return 0 - t; }
        return t + 1;
    }
    fn main() -> i64 {
        let mut total: i64 = 0;
        let mut i: i64 = 0;
        while i < 12 {
            total = total + bump(i - 5, 3);
            i = i + 1;
        }
        println(total);
        return total % 256;
    }
"#;

#[test]
fn leaf_call_is_inlined_but_computation_stays_runtime() {
    let mir = residual(LEAF_CALLER);
    assert_eq!(count_calls(&mir, "bump"), 0, "leaf call should be inlined");

    let mut arithmetic = 0usize;
    let mut branches = 0usize;
    let mut loops = 0usize;
    for f in &mir.functions {
        for (bi, b) in f.blocks.iter().enumerate() {
            for Statement::Assign(_, rv) in &b.statements {
                if matches!(rv, Rvalue::BinaryOp(_, _, _) | Rvalue::UnaryOp(_, _) | Rvalue::Select(_, _, _)) {
                    arithmetic += 1;
                }
            }
            if matches!(b.terminator, numlang::mir::Terminator::BranchIf { .. }) {
                branches += 1;
            }
            if b.terminator.successors().iter().any(|s| s.0 <= bi) {
                loops += 1;
            }
        }
    }
    assert!(arithmetic > 0, "residual arithmetic must remain");
    assert!(branches > 0, "residual guards must remain");
    assert!(loops > 0, "residual loop must remain");
    assert_eq!(run(LEAF_CALLER, "leaf"), (115, "115".to_string()));
}

const RECURSIVE_CALLEE: &str = r#"
    fn countdown(n: i64) -> i64 {
        if n <= 0 { return 0; }
        return 1 + countdown(n - 1);
    }
    fn main() -> i64 {
        let r: i64 = countdown(9);
        println(r);
        return r % 256;
    }
"#;

#[test]
fn recursive_callee_is_not_inlined() {
    let mir = residual(RECURSIVE_CALLEE);
    assert!(
        count_calls(&mir, "countdown") > 0,
        "recursive callee must stay a runtime call"
    );
    let (exit, out) = run(RECURSIVE_CALLEE, "recursive");
    assert_eq!(exit, 9);
    assert!(out.starts_with("9"));
}
