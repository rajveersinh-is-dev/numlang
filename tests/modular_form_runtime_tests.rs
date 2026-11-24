//! Runtime-algorithm modular affine forms.
//!
//! A closed (no runtime-input) program may not have its computation replaced
//! by the final answer literal.  The runtime-computation guard therefore skips
//! the constant-addend affine collapse for such programs.  This test exercises
//! the complementary path in `distill.rs`: when the accumulator addend is a
//! pure call with source-literal arguments, the call is kept and the loop is
//! replaced by Mul/Mod/Add operations derived from the MIR.  No part of the
//! final answer is emitted as a literal.
//!
//! The checks are name- and value-agnostic (an alpha-renamed twin must produce
//! the same structural profile) and verify the emitted program actually runs
//! to the same output/exit as the source computation.

use numlang::ast::BinaryOp;
use numlang::codegen::compile_mir_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::distill::collapse_modular_accumulators_with_const_eval;
use numlang::mir::Terminator;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typed_ast::TypedLiteral;
use numlang::typecheck::typecheck;
use std::fs;
use std::process::Command;

const COUPLED_FIB: &str = r#"
fn stepper(arg: i64) -> i64 {
    let mut p: i64 = 0;
    let mut q: i64 = 1;
    let mut j: i64 = 0;
    while j < arg {
        let np: i64 = q;
        let nq: i64 = (p + q) % 1000000007;
        p = np;
        q = nq;
        j = j + 1;
    }
    return p;
}
fn main() -> i64 {
    let mut total: i64 = 0;
    let mut k: i64 = 0;
    for k in 0..1000 {
        total = (total + stepper(40)) % 1000000007;
    }
    println(total);
    return total % 256;
}
"#;

// Alpha-renamed twin: different function/variable names, identical structure.
const COUPLED_FIB_RENAMED: &str = r#"
fn zeta(limit: i64) -> i64 {
    let mut left: i64 = 0;
    let mut right: i64 = 1;
    let mut cursor: i64 = 0;
    while cursor < limit {
        let next_left: i64 = right;
        let next_right: i64 = (left + right) % 1000000007;
        left = next_left;
        right = next_right;
        cursor = cursor + 1;
    }
    return left;
}
fn main() -> i64 {
    let mut acc: i64 = 0;
    let mut idx: i64 = 0;
    for idx in 0..1000 {
        acc = (acc + zeta(40)) % 1000000007;
    }
    println(acc);
    return acc % 256;
}
"#;

// The additive producer is impure (it prints), so relocating it out of the
// loop would change observable behaviour.  The pass must not fire.
const IMPURE_PRODUCER: &str = r#"
fn noisy(arg: i64) -> i64 {
    println(arg);
    return arg + 1;
}
fn main() -> i64 {
    let mut total: i64 = 0;
    let mut k: i64 = 0;
    while k < 1000 {
        total = (total + noisy(7)) % 1000000007;
        k = k + 1;
    }
    println(total);
    return total % 256;
}
"#;

#[derive(Debug, Default, PartialEq, Eq)]
struct Shape {
    mul: usize,
    mod_: usize,
    pure_calls: usize,
    answer_literal: usize,
    const_stores: usize,
    main_branchy_blocks: usize,
}

fn profile(mir: &MirProgram, answer: i64) -> Shape {
    let mut s = Shape::default();
    for func in &mir.functions {
        for block in &func.blocks {
            if func.name == "main" && matches!(block.terminator, Terminator::BranchIf { .. }) {
                s.main_branchy_blocks += 1;
            }
            for Statement::Assign(dest, rv) in &block.statements {
                match rv {
                    Rvalue::BinaryOp(BinaryOp::Mul, _, _) => s.mul += 1,
                    Rvalue::BinaryOp(BinaryOp::Mod, _, _) => s.mod_ += 1,
                    Rvalue::Call(name, _) if name != "println" && name != "print" => {
                        s.pure_calls += 1
                    }
                    Rvalue::Constant(TypedLiteral::Int(v, _)) => {
                        if *v == answer {
                            s.answer_literal += 1;
                        }
                        if dest.projections.iter().any(|p| {
                            matches!(p, numlang::mir::Projection::Index(_))
                        }) {
                            s.const_stores += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    s
}

fn closed_mir(src: &str) -> MirProgram {
    let tokens = tokenize(src).expect("tokenize");
    let parsed = parse(&tokens).expect("parse");
    let typed = typecheck(&parsed).expect("typecheck");
    let mut mir = lower_program(&typed);
    mir.enforce_runtime_computation = true;
    collapse_modular_accumulators_with_const_eval(&mut mir, false);
    mir
}

fn run_mir(mir: &MirProgram, tag: &str) -> (i32, String) {
    let stem = format!("nl_modform_{}_{}", tag, std::process::id());
    let dir = std::env::temp_dir();
    let obj = dir.join(format!("{}.obj", stem));
    let exe = dir.join(format!("{}.exe", stem));
    let bytes = compile_mir_to_obj(mir).expect("compile residual MIR");
    fs::write(&obj, bytes).expect("write object");
    link_executable(&obj, &exe).expect("link");
    let out = Command::new(&exe).output().expect("run");
    let _ = fs::remove_file(&obj);
    let _ = fs::remove_file(&exe);
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    )
}

#[test]
fn closed_program_emits_algorithm_without_answer_literal() {
    let answer = 334_154_286i64;
    for (tag, src) in [
        ("named", COUPLED_FIB),
        ("renamed", COUPLED_FIB_RENAMED),
    ] {
        let mir = closed_mir(src);
        let shape = profile(&mir, answer);
        assert_eq!(shape.answer_literal, 0, "{tag}: answer literal emitted: {shape:?}");
        assert_eq!(shape.const_stores, 0, "{tag}: lookup table emitted: {shape:?}");
        assert!(shape.mul > 0 && shape.mod_ > 0, "{tag}: no arithmetic emitted: {shape:?}");
        assert_eq!(shape.pure_calls, 1, "{tag}: producer call not preserved once: {shape:?}");
        assert_eq!(shape.main_branchy_blocks, 0, "{tag}: accumulator loop not collapsed: {shape:?}");
    }
}

#[test]
fn alpha_renamed_variants_have_identical_profiles() {
    // The transformation derives its operands from MIR structure only.
    assert_eq!(
        profile(&closed_mir(COUPLED_FIB), 334_154_286),
        profile(&closed_mir(COUPLED_FIB_RENAMED), 334_154_286)
    );
}

#[test]
fn closed_program_still_computes_the_same_answer_at_runtime() {
    let mir = closed_mir(COUPLED_FIB);
    let (code, out) = run_mir(&mir, "closed");
    assert_eq!(out, "334154286", "wrong stdout");
    // The bench/runtime entry truncates the returned i64 to 8 bits on every
    // platform here; the audit suite documents the same behaviour.
    assert_eq!(code, 46, "wrong exit code");
}

#[test]
fn impure_producer_is_not_relocated() {
    let mir = closed_mir(IMPURE_PRODUCER);
    let shape = profile(&mir, -1);
    assert!(
        shape.main_branchy_blocks > 0,
        "impure producer must keep the source loop: {shape:?}"
    );
}
