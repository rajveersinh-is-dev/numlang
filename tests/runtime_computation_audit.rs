//! Runtime-computation integrity audit (name-agnostic, structural).
//!
//! INTEGRITY_RULES.md §1 forbids "pre-loaded lookup tables, hardcoded recurrence
//! outputs, precalculated benchmark answers" and requires that "everything must be
//! computed as is". `tests/integrity_lint.rs` only greps `src/*.rs` for a handful of
//! quoted benchmark *names*; it never inspects the code the compiler actually emits.
//!
//! This test closes that gap with a semantic, name-agnostic check on **residual MIR** —
//! the code that is handed to Cranelift. It classifies each residual program as:
//!
//!   (A) computes from runtime input        -> residual reads input and has arithmetic
//!   (B) computes a formula from literals   -> residual has arithmetic, no answer literal
//!   (C) emits the answer/table as a literal -> residual has no real computation and either
//!                                              contains the final answer as a constant or
//!                                              is dominated by a block of constant array
//!                                              stores (a precomputed lookup table)
//!
//! Only (C) is rejected. The check knows no function names, no benchmark names and no
//! particular answer values: it is parameterised by an independently computed expected
//! answer for the small programs under test. A future optimisation that replaces the
//! computation with a precomputed answer therefore fails this test, while genuine formula
//! synthesis (B) and runtime-input computation (A) remain allowed.
//!
//! The detector is exercised from three directions: two alpha-equivalent constant-input
//! programs that must stay in class (B), one runtime-input program that must stay in
//! class (A) and track two different inputs, and a deliberately hardcoded negative
//! control that must be classified as (C).

use numlang::ast::BinaryOp;
use numlang::codegen::compile_supercompiled_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::compiler::{distill_and_optimize, CompilerConfig};
use numlang::mir::lower::{lower_program, Rvalue, Statement};
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::mir::{Projection, Terminator};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typed_ast::TypedLiteral;
use numlang::typecheck::typecheck;
use std::fs;
use std::process::Command;

/// Structural fingerprint of a residual MIR program.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Profile {
    statements: usize,
    /// Arithmetic / selection operations (Add, Mul, Mod, Unary, Select, Phi).
    arithmetic: usize,
    /// Conditional or looping control flow (BranchIf, Switch, Fork, back-edges).
    branches: usize,
    /// Calls to `read_*` intrinsics — evidence of class (A).
    runtime_reads: usize,
    /// All integer literals emitted anywhere in the residual program.
    constants: Vec<i64>,
    /// `arr[const] = <integer literal>` stores — a precomputed lookup table.
    constant_array_stores: usize,
}

impl Profile {
    /// True when the residual program really executes a derivation rather than
    /// merely returning a value that was decided at compile time.
    fn computes(&self) -> bool {
        self.arithmetic > 0 || self.branches > 0 || self.runtime_reads > 0
    }

    /// Structural class-(C) detector. `answer` is the independently computed
    /// result of the program, or `None` when the result is input-dependent.
    fn is_precomputed(&self, answer: Option<i64>) -> bool {
        // C1: the final answer is emitted as a literal and there is no computation.
        let answer_literal = answer.is_some_and(|a| self.constants.contains(&a)) && !self.computes();
        // C2: a precomputed table is materialised as constant indexed stores. A
        // final `%256`-style modulo (one or two ops) does not make this a derivation.
        let constant_table = self.constant_array_stores >= 16
            && self.runtime_reads == 0
            && self.arithmetic <= 2;
        answer_literal || constant_table
    }
}

/// Reproduce the exact pipeline used by `compile_supercompiled_to_obj` and
/// fingerprint the MIR that is actually handed to the native backend.
fn residual_profile(src: &str) -> Profile {
    let tokens = tokenize(src).expect("tokenize");
    let ast = parse(&tokens).expect("parse");
    let mut typed = typecheck(&ast).expect("typecheck");
    distill_and_optimize(
        &mut typed,
        &CompilerConfig {
            ho_distill: true,
            supercompile: true,
            ..Default::default()
        },
    );
    let mut mir = lower_program(&typed);
    supercompile_mir_program(&mut mir);

    let mut p = Profile::default();
    for func in &mir.functions {
        for block in &func.blocks {
            let id = block.id.0;
            match block.terminator {
                Terminator::BranchIf { .. }
                | Terminator::Switch { .. }
                | Terminator::Fork { .. }
                | Terminator::TypeGuard { .. } => p.branches += 1,
                _ => {}
            }
            for succ in block.terminator.successors() {
                if succ.0 <= id {
                    p.branches += 1; // back-edge => residual loop
                }
            }
            for stmt in &block.statements {
                p.statements += 1;
                let Statement::Assign(place, rvalue) = stmt;
                match rvalue {
                    Rvalue::BinaryOp(op, _, _) => {
                        if !matches!(op, BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge) {
                            p.arithmetic += 1;
                        }
                    }
                    Rvalue::UnaryOp(_, _) | Rvalue::Select(_, _, _) | Rvalue::Phi(_) => {
                        p.arithmetic += 1;
                    }
                    Rvalue::Call(name, _) if name.starts_with("read") => p.runtime_reads += 1,
                    Rvalue::Constant(TypedLiteral::Int(v, _)) => {
                        p.constants.push(*v);
                        if place.projections.iter().any(|p| matches!(p, Projection::Index(_))) {
                            p.constant_array_stores += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    p
}

fn compile_and_run(src: &str, tag: &str) -> (i32, String) {
    let stem = format!("nl_audit_{}_{}", tag, std::process::id());
    let dir = std::env::temp_dir();
    let obj_path = dir.join(format!("{}.obj", stem));
    let exe_path = dir.join(format!("{}.exe", stem));
    let tokens = tokenize(src).expect("tokenize");
    let ast = parse(&tokens).expect("parse");
    let typed = typecheck(&ast).expect("typecheck");
    let obj = compile_supercompiled_to_obj(&typed).expect("supercompiled codegen");
    fs::write(&obj_path, obj).expect("write object");
    let linked = link_executable(&obj_path, &exe_path);
    assert!(linked.is_ok(), "link failed: {:?}", linked.err());
    let out = Command::new(&exe_path).output().expect("run binary");
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    )
}

// Class-(B) representatives: deliberately non-benchmark function names, small
// inputs, and an alpha-renamed twin to prove the check is name-agnostic.
const FIB_LIKE: &str = r#"
    fn zebra_step(k: i64) -> i64 {
        if k <= 1 { return k; } else { return zebra_step(k - 1) + zebra_step(k - 2); }
    }
    fn main() -> i64 {
        let result: i64 = zebra_step(12);
        println(result);
        return result % 256;
    }
"#;

const FIB_LIKE_RENAMED: &str = r#"
    fn q(alpha: i64) -> i64 {
        if alpha <= 1 { return alpha; } else { return q(alpha - 1) + q(alpha - 2); }
    }
    fn main() -> i64 {
        let z: i64 = q(12);
        println(z);
        return z % 256;
    }
"#;

// Class-(A) representative: the value comes from `read_i64()` at execution.
const RUNTIME_INPUT: &str = r#"
    fn zebra_mix(bound: i64) -> i64 {
        let mut acc: i64 = 0;
        let mut i: i64 = 1;
        while i <= bound {
            if (i * i) % 3 == 0 { acc = acc + i; } else { acc = acc + 1; }
            i = i + 1;
        }
        return acc;
    }
    fn main() -> i64 {
        let n: i64 = read_i64();
        let r: i64 = zebra_mix(n);
        println(r);
        return r % 256;
    }
"#;

// Negative control: the source itself returns a literal, so the residual is
// exactly the class-(C) shape this test must reject.
const HARDCODED: &str = r#"
    fn main() -> i64 {
        let r: i64 = 78;
        println(r);
        return r % 256;
    }
"#;

fn reference_mix(n: i64) -> i64 {
    (1..=n)
        .map(|i| if (i * i) % 3 == 0 { i } else { 1 })
        .sum()
}

#[test]
fn constant_input_programs_keep_their_derivation() {
    // Independent expected value: F(12) = 144.
    let expected = 144i64;
    for (tag, src) in [("zebra", FIB_LIKE), ("renamed", FIB_LIKE_RENAMED)] {
        let p = residual_profile(src);
        assert!(
            p.computes(),
            "{}: residual MIR has no computation (statements={:?})",
            tag,
            p
        );
        assert!(
            !p.constants.contains(&expected),
            "{}: residual MIR emits the final answer {} as a literal: {:?}",
            tag,
            expected,
            p
        );
        assert!(
            !p.is_precomputed(Some(expected)),
            "{}: residual MIR is class (C): {:?}",
            tag,
            p
        );
        let (code, out) = compile_and_run(src, tag);
        assert_eq!(out, expected.to_string(), "{}: wrong stdout", tag);
        let expected_exit = if cfg!(windows) { expected } else { expected % 256 };
        assert_eq!(code, expected_exit as i32, "{}: wrong exit code", tag);
    }
}

#[test]
fn runtime_input_program_is_allowed_and_tracks_its_input() {
    let p = residual_profile(RUNTIME_INPUT);
    assert!(
        p.runtime_reads > 0 && p.computes(),
        "class (A) program must read input and compute: {:?}",
        p
    );
    assert!(
        !p.is_precomputed(None),
        "input-dependent program must not be class (C): {:?}",
        p
    );

    // Build the `read_i64()` entry point and prove two different inputs yield two
    // different, independently verified outputs (no table can satisfy both).
    let stem = format!("nl_audit_runtime_{}", std::process::id());
    let src_path = std::env::temp_dir().join(format!("{}.nl", stem));
    let exe_path = std::env::temp_dir().join(format!("{}.exe", stem));
    fs::write(&src_path, RUNTIME_INPUT).expect("write temp source");
    let build = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("build")
        .arg(&src_path)
        .arg("--bench")
        .arg("--supercompile")
        .arg("--no-cache")
        .arg("-o")
        .arg(&exe_path)
        .output()
        .expect("spawn numlang");
    assert!(
        build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    for n in [12i64, 15i64] {
        let out = Command::new(&exe_path)
            .arg(n.to_string())
            .output()
            .expect("run runtime-input binary");
        let printed = String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        assert_eq!(
            printed,
            reference_mix(n).to_string(),
            "runtime input {} must be computed, not fetched",
            n
        );
    }
    let _ = fs::remove_file(&src_path);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn detector_rejects_emitted_answer_literal() {
    let p = residual_profile(HARDCODED);
    assert_eq!(p.statements, 3, "negative control should be minimal");
    assert!(p.constants.contains(&78), "answer literal must be present");
    assert!(!p.computes(), "negative control has no derivation");
    assert!(
        p.is_precomputed(Some(78)),
        "the detector must classify a pure literal emission as (C): {:?}",
        p
    );
}

#[test]
fn detector_is_path_independent() {
    // Guard against a detector that only works on one particular source path:
    // alpha-renaming must produce byte-identical structural profiles.
    assert_eq!(residual_profile(FIB_LIKE), residual_profile(FIB_LIKE_RENAMED));
}

