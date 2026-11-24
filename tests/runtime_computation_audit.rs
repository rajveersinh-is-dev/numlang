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
//! The detector is exercised from four directions: two alpha-equivalent constant-input
//! programs that must stay in class (B), one runtime-input program that must stay in
//! class (A) and track two different inputs, a deliberately hardcoded negative
//! control that must be classified as (C), and the NEW runtime-computed closed
//! forms (triangular/cubic, recurrence-solved modular matrix power, Hofstadter
//! sqrt form) which must stay in class (B). All residual MIR is produced by
//! [`driver_mir`], an exact in-process mirror of the CLI's
//! `--supercompile --mode distill` pipeline including the runtime-input guard.

use numlang::ast::BinaryOp;
use numlang::codegen::compile_mir_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::compiler::{distill_and_optimize, CompilerConfig};
use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::{supercompile_mir_program_with_mode, SupercompileMode};
use numlang::mir::{Projection, Terminator};
use numlang::parser::parse;
use numlang::runtime_input::set_ast_runtime_computation_guard;
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

/// Exact in-process mirror of the CLI's `--supercompile --mode distill`
/// pipeline: AST/MIR runtime-computation guard, `distill_and_optimize`, and
/// distill-mode supercompilation with the published `size` objective. Both the
/// structural profiler and the native-code runner use this same MIR.
fn driver_mir(src: &str) -> MirProgram {
    let tokens = tokenize(src).expect("tokenize");
    let ast = parse(&tokens).expect("parse");
    let mut typed = typecheck(&ast).expect("typecheck");
    let previous_guard = set_ast_runtime_computation_guard(true);
    distill_and_optimize(
        &mut typed,
        &CompilerConfig {
            ho_distill: true,
            supercompile: true,
            ..Default::default()
        },
    );
    set_ast_runtime_computation_guard(previous_guard);
    let mut mir = lower_program(&typed);
    // The CLI's `real_main` calls `mark_compiler_driver()`, making this true.
    mir.enforce_runtime_computation = true;
    supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    mir
}

/// Fingerprint the MIR that is actually handed to the native backend by the
/// driver's `--supercompile --mode distill` pipeline.
fn residual_profile(src: &str) -> Profile {
    let mir = driver_mir(src);
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
    let mir = driver_mir(src);
    let obj = compile_mir_to_obj(&mir).expect("supercompiled codegen");
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

// NEW runtime-computed closed forms. Deliberately non-benchmark identifiers and
// small bounds; expected values come from the independent references below.
const TRIANGULAR_CLOSED: &str = r#"
    fn tally(bound: i64) -> i64 {
        let mut acc: i64 = 0;
        let mut i: i64 = 1;
        while i <= bound { acc = acc + i; i = i + 1; }
        return acc;
    }
    fn main() -> i64 {
        let r: i64 = tally(12);
        println(r);
        return r % 256;
    }
"#;

const CUBIC_CLOSED: &str = r#"
    fn tally(bound: i64) -> i64 {
        let mut acc: i64 = 0;
        let mut i: i64 = 1;
        while i <= bound { acc = acc + i * i * i; i = i + 1; }
        return acc;
    }
    fn main() -> i64 {
        let r: i64 = tally(8);
        println(r);
        return r % 256;
    }
"#;

// Coupled-recurrence producer accumulated modulo M: the driver solves the
// producer to a `__nway_recurrence_*` intrinsic and emits modular arithmetic.
const MODULAR_MATRIX_RECUR: &str = r#"
    fn step_gen(arg: i64) -> i64 {
        let mut p: i64 = 0;
        let mut q: i64 = 1;
        let mut j: i64 = 0;
        while j < arg {
            let np: i64 = q;
            let nq: i64 = (p + q) % 1000000007;
            p = np; q = nq; j = j + 1;
        }
        return p;
    }
    fn main() -> i64 {
        let mut total: i64 = 0;
        for k in 0..1000 {
            total = (total + step_gen(40)) % 1000000007;
        }
        println(total);
        return total % 256;
    }
"#;

// Mutual index-nested array recurrence rewritten to a sqrt-based Beatty form.
const HOFSTADTER_SQRT: &str = r#"
    fn mirror_pair(bound: i64) -> i64 {
        let mut a: [i64; 24] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut b: [i64; 24] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        a[0] = 1; b[0] = 0;
        let mut i: i64 = 1;
        while i <= bound {
            let a_prev: i64 = a[i - 1];
            let b_of_a: i64 = b[a_prev];
            a[i] = i - b_of_a;
            let b_prev: i64 = b[i - 1];
            let a_of_b: i64 = a[b_prev];
            b[i] = i - a_of_b;
            i = i + 1;
        }
        return b[bound];
    }
    fn main() -> i64 {
        let r: i64 = mirror_pair(12);
        println(r);
        return r % 256;
    }
"#;

fn mir_has_call(mir: &MirProgram, name: &str) -> bool {
    mir.functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .flat_map(|b| b.statements.iter())
        .any(|stmt| matches!(stmt, Statement::Assign(_, Rvalue::Call(callee, _)) if callee == name))
}

fn mir_has_call_prefix(mir: &MirProgram, prefix: &str) -> bool {
    mir.functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .flat_map(|b| b.statements.iter())
        .any(|stmt| {
            matches!(stmt, Statement::Assign(_, Rvalue::Call(callee, _)) if callee.starts_with(prefix))
        })
}

// Independent reference computations; no benchmark answer is copied into a test.
fn ref_triangular(n: i64) -> i64 {
    (1..=n).sum()
}

fn ref_cubic(n: i64) -> i64 {
    (1..=n).map(|i| i * i * i).sum()
}

fn ref_modular_matrix(trips: i64, exp: i64) -> i64 {
    const M: i64 = 1_000_000_007;
    let (mut p, mut q) = (0i64, 1i64);
    for _ in 0..exp {
        let next = (p + q) % M;
        p = q;
        q = next;
    }
    let mut total = 0i64;
    for _ in 0..trips {
        total = (total + p) % M;
    }
    total
}

fn ref_hofstadter(limit: usize) -> i64 {
    let mut a = vec![0i64; limit + 2];
    let mut b = vec![0i64; limit + 2];
    a[0] = 1;
    for i in 1..=limit {
        let av = a[i - 1] as usize;
        let bv = b[i - 1] as usize;
        a[i] = i as i64 - b[av];
        b[i] = i as i64 - a[bv];
    }
    b[limit]
}

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

// ---------------------------------------------------------------------------
// NEW closed-form passes (triangular/cubic, recurrence-solved modular matrix
// power, Hofstadter sqrt form) audited through the exact driver pipeline.
// ---------------------------------------------------------------------------

#[test]
fn new_closed_forms_are_class_b_not_baked_answers() {
    let cases: [(&str, &str, i64); 4] = [
        ("triangular", TRIANGULAR_CLOSED, ref_triangular(12)),
        ("cubic", CUBIC_CLOSED, ref_cubic(8)),
        (
            "matrix_recurrence",
            MODULAR_MATRIX_RECUR,
            ref_modular_matrix(1000, 40),
        ),
        ("hofstadter_sqrt", HOFSTADTER_SQRT, ref_hofstadter(12)),
    ];
    for (tag, src, expected) in cases {
        let p = residual_profile(src);
        assert!(
            p.computes() && p.arithmetic > 0,
            "{tag}: residual MIR has no arithmetic/loop/branch: {p:?}"
        );
        assert!(
            !p.constants.contains(&expected),
            "{tag}: independently computed answer {expected} emitted as a literal: {p:?}"
        );
        assert!(
            !p.is_precomputed(Some(expected)),
            "{tag}: residual MIR is class (C) — baked answer/table: {p:?}"
        );
        // The derivation must still produce the independently computed value.
        let (code, out) = compile_and_run(src, tag);
        assert_eq!(out, expected.to_string(), "{tag}: wrong stdout");
        assert_eq!(
            code,
            expected.rem_euclid(256) as i32,
            "{tag}: wrong exit code"
        );
    }
}

#[test]
fn matrix_recurrence_keeps_its_intrinsic_and_modular_arithmetic() {
    let mir = driver_mir(MODULAR_MATRIX_RECUR);
    let expected = ref_modular_matrix(1000, 40);
    assert!(
        mir_has_call_prefix(&mir, "__nway_recurrence_"),
        "recurrence-solved producer must stay a runtime call:\n{mir:#?}"
    );
    let has_mul_mod = mir.functions.iter().flat_map(|f| f.blocks.iter()).any(|b| {
        let mul = b.statements.iter().any(
            |Statement::Assign(_, rv)| matches!(rv, Rvalue::BinaryOp(BinaryOp::Mul, _, _)),
        );
        let module = b.statements.iter().any(
            |Statement::Assign(_, rv)| matches!(rv, Rvalue::BinaryOp(BinaryOp::Mod, _, _)),
        );
        mul && module
    });
    assert!(has_mul_mod, "modular affine Mul/Mod missing:\n{mir:#?}");
    let p = residual_profile(MODULAR_MATRIX_RECUR);
    assert!(!p.constants.contains(&expected), "answer literal: {p:?}");
    assert!(!p.is_precomputed(Some(expected)), "precomputed answer: {p:?}");
}

#[test]
fn hofstadter_sqrt_form_keeps_arithmetic_and_drops_the_table() {
    let mir = driver_mir(HOFSTADTER_SQRT);
    let expected = ref_hofstadter(12);
    assert!(
        mir_has_call(&mir, "sqrt"),
        "sqrt-based closed form must call sqrt at run time:\n{mir:#?}"
    );
    let has_arithmetic = mir.functions.iter().flat_map(|f| f.blocks.iter()).any(|b| {
        b.statements.iter().any(|Statement::Assign(_, rv)| {
            matches!(
                rv,
                Rvalue::BinaryOp(
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div,
                    _,
                    _
                )
            )
        })
    });
    assert!(has_arithmetic, "sqrt correction arithmetic missing:\n{mir:#?}");
    let p = residual_profile(HOFSTADTER_SQRT);
    assert!(!p.constants.contains(&expected), "answer literal: {p:?}");
    assert!(!p.is_precomputed(Some(expected)), "precomputed answer: {p:?}");
}

