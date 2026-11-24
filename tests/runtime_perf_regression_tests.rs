//! Runtime-computation integrity + speed-sanity regression harness (W5e).
//!
//! This file is the *structural* counterpart to `tests/integrity_lint.rs`.
//! `integrity_lint` only greps compiler source for quoted benchmark names; it
//! cannot see what the compiler actually emits. Here we compile six small,
//! closed (no `read_i64`) constant-input programs spanning the shapes that are
//! being turned into runtime-computed closed forms — triangular/cubic sums,
//! modular matrix power, a mutual recurrence, a KMP-style DFA and a sliding
//! window — and fingerprint the **residual MIR handed to the native backend**.
//!
//! Every check is name-agnostic: it looks at MIR opcodes, literals and control
//! flow, never at function/benchmark names. Expected answers are produced by
//! independent Rust reference interpreters/formulas in this file, not copied
//! into the assertions as magic numbers. A regression that replaces a
//! derivation with a baked answer (`let r = <answer>;`) or with a dense
//! precomputed array is rejected by `is_precomputed`; a genuine closed form
//! (arithmetic over literals, still executed) is accepted.
//!
//! The second half of the file is a reusable `COMPUTE_NS` harness. It builds a
//! kernel with `--bench` and reports the median in-process timing. The
//! threshold assertions are `#[ignore]`d: the current baseline is the slow
//! loop, so CI must not fail yet. After the fast closed forms land, run
//! `cargo test --test runtime_perf_regression_tests -- --ignored --nocapture`
//! to enable them.

use numlang::ast::BinaryOp;
use numlang::codegen::compile_mir_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::compiler::{distill_and_optimize, CompilerConfig};
use numlang::mir::lower::{lower_program, Rvalue, Statement};
use numlang::mir::supercompiler::{supercompile_mir_program_with_mode, SupercompileMode};
use numlang::mir::{Projection, Terminator};
use numlang::parser::parse;
use numlang::runtime_input::set_ast_runtime_computation_guard;
use numlang::token::tokenize;
use numlang::typecheck::typed_ast::TypedLiteral;
use numlang::typecheck::typecheck;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

// ---------------------------------------------------------------------------
// MIR fingerprint
// ---------------------------------------------------------------------------

/// Structural fingerprint of the residual MIR program.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Profile {
    statements: usize,
    /// Arithmetic / selection ops (Add, Mul, Mod, Unary, Select, Phi). Comparisons
    /// are not counted here — they are counted as branches via terminators.
    arithmetic: usize,
    /// Conditional / looping control flow (BranchIf, Switch, Fork, back-edges).
    branches: usize,
    /// Calls to `read_*` intrinsics (class-A, runtime-input programs).
    runtime_reads: usize,
    /// Every integer literal anywhere in the residual program.
    constants: Vec<i64>,
    /// `arr[const] = <integer literal>` stores (one way to materialise a table).
    constant_array_stores: usize,
    /// Distinct "interesting" literals: excludes 0/1 (loop init/increment) and
    /// 256 (the `% 256` exit-code convention of these tiny drivers). A dense
    /// block of *these* is the signature of a precomputed lookup table.
    distinct_table_values: usize,
}

impl Profile {
    /// True when the program really executes a derivation rather than only
    /// returning a value that was decided at compile time.
    fn computes(&self) -> bool {
        self.arithmetic > 0 || self.branches > 0 || self.runtime_reads > 0
    }

    /// Structural detector for class (C) — a precomputed answer.
    ///
    /// * `C1` — the independently computed answer is emitted as a literal and
    ///   there is no computation at all (the compiler constant-folded the
    ///   whole program).
    /// * `C2` — a dense block of distinct non-trivial literals (or >=16 indexed
    ///   constant stores) is materialised with essentially no arithmetic and no
    ///   control flow: that is a lookup table, not a derivation. The index may
    ///   come from runtime input — a precomputed table stays precomputed — so
    ///   `runtime_reads` is deliberately not an exemption here. This fires even
    ///   when the final answer itself is produced by an index expression rather
    ///   than by a standalone literal.
    fn is_precomputed(&self, answer: Option<i64>) -> bool {
        let answer_literal = answer.is_some_and(|a| self.constants.contains(&a));
        let c1 = answer_literal && !self.computes();
        let dense_literal_block =
            self.distinct_table_values >= 8 || self.constant_array_stores >= 16;
        let c2 = dense_literal_block && self.arithmetic <= 2 && self.branches == 0;
        c1 || c2
    }
}

/// Reproduce the driver pipeline (including its integrity guard) and fingerprint
/// the MIR that is actually handed to Cranelift.
fn residual_profile(src: &str) -> Profile {
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
    mir.enforce_runtime_computation = true;
    // Published benchmark claims use distill mode; audit exactly that pipeline.
    supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");

    let mut profile = Profile::default();
    let mut interesting = BTreeSet::new();
    for func in &mir.functions {
        for block in &func.blocks {
            let id = block.id.0;
            match block.terminator {
                Terminator::BranchIf { .. }
                | Terminator::Switch { .. }
                | Terminator::Fork { .. }
                | Terminator::TypeGuard { .. } => profile.branches += 1,
                _ => {}
            }
            for succ in block.terminator.successors() {
                if succ.0 <= id {
                    profile.branches += 1; // back-edge => residual loop
                }
            }
            for stmt in &block.statements {
                profile.statements += 1;
                let Statement::Assign(place, rvalue) = stmt;
                match rvalue {
                    Rvalue::BinaryOp(op, _, _) => {
                        if !matches!(
                            op,
                            BinaryOp::Eq
                                | BinaryOp::Ne
                                | BinaryOp::Lt
                                | BinaryOp::Le
                                | BinaryOp::Gt
                                | BinaryOp::Ge
                        ) {
                            profile.arithmetic += 1;
                        }
                    }
                    Rvalue::UnaryOp(_, _) | Rvalue::Select(_, _, _) | Rvalue::Phi(_) => {
                        profile.arithmetic += 1;
                    }
                    Rvalue::Call(name, _) if name.starts_with("read") => {
                        profile.runtime_reads += 1;
                    }
                    Rvalue::Constant(TypedLiteral::Int(value, _)) => {
                        profile.constants.push(*value);
                        if place
                            .projections
                            .iter()
                            .any(|p| matches!(p, Projection::Index(_)))
                        {
                            profile.constant_array_stores += 1;
                        }
                        if *value != 0 && *value != 1 && *value != 256 {
                            interesting.insert(*value);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    profile.distinct_table_values = interesting.len();
    profile
}

/// Compile a source to a native executable, run it once and return
/// `(exit_code, trimmed_stdout)`.
fn compile_and_run(src: &str, tag: &str) -> (i32, String) {
    let stem = format!(
        "nl_w5e_{}_{}_{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let dir = std::env::temp_dir();
    let obj_path = dir.join(format!("{}.obj", stem));
    let exe_path = dir.join(format!("{}.exe", stem));

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
    // Same MIR as `residual_profile`: the driver's integrity marker is applied
    // explicitly because a test process is not marked as the compiler driver.
    let mut mir = lower_program(&typed);
    mir.enforce_runtime_computation = true;
    // Published benchmark claims use distill mode; audit exactly that pipeline.
    supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
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

// ---------------------------------------------------------------------------
// Representative constant-input kernels (deliberately not benchmark names)
// ---------------------------------------------------------------------------

const TRI_N: i64 = 12;
const CUBIC_N: i64 = 8;
const MAT_EXP: usize = 40;
const HOF_LIMIT: usize = 12;
const KMP_STEPS: i64 = 15;
const WINDOW_STEPS: usize = 12;

fn triangular_program(n: i64) -> String {
    format!(
        r#"
fn folded_tally(bound: i64) -> i64 {{
    let mut sum: i64 = 0;
    let mut i: i64 = 1;
    while i <= bound {{ sum = sum + i; i = i + 1; }}
    return sum;
}}
fn main() -> i64 {{
    let r: i64 = folded_tally({n});
    println(r);
    return r % 256;
}}
"#
    )
}

/// Alpha-renamed twin of `triangular_program` — proves the detector does not
/// depend on spelling of identifiers.
fn triangular_program_renamed(n: i64) -> String {
    format!(
        r#"
fn q(alpha: i64) -> i64 {{
    let mut total: i64 = 0;
    let mut j: i64 = 1;
    while j <= alpha {{ total = total + j; j = j + 1; }}
    return total;
}}
fn main() -> i64 {{
    let out: i64 = q({n});
    println(out);
    return out % 256;
}}
"#
    )
}

fn cubic_program(n: i64) -> String {
    format!(
        r#"
fn cube_tally(bound: i64) -> i64 {{
    let mut sum: i64 = 0;
    let mut i: i64 = 1;
    while i <= bound {{ sum = sum + i * i * i; i = i + 1; }}
    return sum;
}}
fn main() -> i64 {{
    let r: i64 = cube_tally({n});
    println(r);
    return r % 256;
}}
"#
    )
}

/// Repeated squaring of the 2x2 Fibonacci matrix `[[1,1],[1,0]]` modulo M,
/// with the four entries kept as scalars (arrays-as-parameters are not part of
/// the tested language surface). Multiplying by the base maps
/// `[a00,a01,a10,a11] -> [a00+a01, a00, a10+a11, a10] mod M`, so the top-left
/// entry after `e` steps is F(e+1).
fn matrix_power_program(exp: usize) -> String {
    format!(
        r#"
fn pow2(e: i64) -> i64 {{
    let mut a00: i64 = 1;
    let mut a01: i64 = 0;
    let mut a10: i64 = 0;
    let mut a11: i64 = 1;
    let mut i: i64 = 0;
    while i < e {{
        let n00: i64 = (a00 + a01) % 1000000007;
        let n01: i64 = a00;
        let n10: i64 = (a10 + a11) % 1000000007;
        let n11: i64 = a10;
        a00 = n00; a01 = n01; a10 = n10; a11 = n11;
        i = i + 1;
    }}
    return a00;
}}
fn main() -> i64 {{
    let r: i64 = pow2({exp});
    println(r);
    return r % 256;
}}
"#
    )
}

fn hofstadter_program(limit: usize) -> String {
    let zeros = "0, ".repeat(23) + "0";
    format!(
        r#"
fn mutual(limit: i64) -> i64 {{
    let mut f: [i64; 24] = [{zeros}];
    let mut m: [i64; 24] = [{zeros}];
    f[0] = 1; m[0] = 0;
    let mut i: i64 = 1;
    while i <= limit {{
        let f_prev: i64 = f[i - 1];
        let m_of_f: i64 = m[f_prev];
        f[i] = i - m_of_f;
        let m_prev: i64 = m[i - 1];
        let f_of_m: i64 = f[m_prev];
        m[i] = i - f_of_m;
        i = i + 1;
    }}
    return m[limit];
}}
fn main() -> i64 {{
    let r: i64 = mutual({limit});
    println(r);
    return r % 256;
}}
"#
    )
}

fn kmp_program(steps: i64) -> String {
    format!(
        r#"
fn state_machine(steps: i64) -> i64 {{
    let mut state: i64 = 0;
    let mut hits: i64 = 0;
    let mut i: i64 = 0;
    while i < steps {{
        let symbol: i64 = (i * 73 + 19) % 2;
        if state == 0 {{
            if symbol == 1 {{ state = 1; }} else {{ state = 0; }}
        }} else {{
            if state == 1 {{
                if symbol == 0 {{ state = 2; }} else {{ state = 1; }}
            }} else {{
                if symbol == 1 {{ hits = hits + 1; state = 1; }} else {{ state = 0; }}
            }}
        }}
        i = i + 1;
    }}
    return hits;
}}
fn main() -> i64 {{
    let r: i64 = state_machine({steps});
    println(r);
    return r % 256;
}}
"#
    )
}

/// A four-tap sliding window over the generated sequence `(37*i+11) % 13`.
/// The taps are scalars: array reads inside a residual loop are currently
/// miscompiled by the Classic supercompiler, which is outside this test's
/// ownership; the scalar form keeps the *shape* (rolling window) under test.
fn sliding_window_program(steps: usize) -> String {
    format!(
        r#"
fn windowed(steps: i64) -> i64 {{
    let mut total: i64 = 0;
    let mut i: i64 = 0;
    let mut w0: i64 = 0;
    let mut w1: i64 = 0;
    let mut w2: i64 = 0;
    let mut w3: i64 = 0;
    while i < steps {{
        let v: i64 = (i * 37 + 11) % 13;
        w0 = w1; w1 = w2; w2 = w3; w3 = v;
        if i >= 3 {{ total = total + w0 + w1 + w2 + w3; }}
        i = i + 1;
    }}
    return total;
}}
fn main() -> i64 {{
    let r: i64 = windowed({steps});
    println(r);
    return r % 256;
}}
"#
    )
}

// ---------------------------------------------------------------------------
// Independent reference computations (no answer literals in the assertions)
// ---------------------------------------------------------------------------

fn ref_triangular(n: i64) -> i64 {
    (1..=n).sum()
}

fn ref_cubic(n: i64) -> i64 {
    (1..=n).map(|i| i * i * i).sum()
}

/// Iterative 2x2 matrix power modulo M; returns the top-left entry.
fn ref_matrix_power(exp: usize) -> i64 {
    let modulus = 1_000_000_007i64;
    let base = [[1i64, 1], [1, 0]];
    let mut acc = [[1i64, 0], [0, 1]];
    for _ in 0..exp {
        let mut next = [[0i64; 2]; 2];
        for i in 0..2 {
            for j in 0..2 {
                next[i][j] = (acc[i][0] * base[0][j] + acc[i][1] * base[1][j]) % modulus;
            }
        }
        acc = next;
    }
    acc[0][0]
}

/// Straightforward array simulation of the mutual f/m recurrence.
fn ref_hofstadter(limit: usize) -> i64 {
    let mut f = vec![0i64; limit + 2];
    let mut m = vec![0i64; limit + 2];
    f[0] = 1;
    for i in 1..=limit {
        let f_prev = f[i - 1] as usize;
        let m_prev = m[i - 1] as usize;
        f[i] = i as i64 - m[f_prev];
        m[i] = i as i64 - f[m_prev];
    }
    m[limit]
}

/// Simple table-free simulation of the three-state matcher.
fn ref_kmp(steps: i64) -> i64 {
    let mut state = 0i64;
    let mut hits = 0i64;
    for i in 0..steps {
        let symbol = (i * 73 + 19) % 2;
        match state {
            0 => state = if symbol == 1 { 1 } else { 0 },
            1 => state = if symbol == 0 { 2 } else { 1 },
            _ => {
                if symbol == 1 {
                    hits += 1;
                    state = 1;
                } else {
                    state = 0;
                }
            }
        }
    }
    hits
}

fn ref_sliding_window(steps: usize) -> i64 {
    let values: Vec<i64> = (0..steps).map(|i| (i as i64 * 37 + 11) % 13).collect();
    (0..steps)
        .filter(|&j| j + 3 < steps)
        .map(|j| values[j] + values[j + 1] + values[j + 2] + values[j + 3])
        .sum()
}

struct Kernel {
    tag: &'static str,
    source: String,
    expected: i64,
}

fn kernels() -> Vec<Kernel> {
    vec![
        Kernel {
            tag: "triangular",
            source: triangular_program(TRI_N),
            expected: ref_triangular(TRI_N),
        },
        Kernel {
            tag: "cubic",
            source: cubic_program(CUBIC_N),
            expected: ref_cubic(CUBIC_N),
        },
        Kernel {
            tag: "matrix_power",
            source: matrix_power_program(MAT_EXP),
            expected: ref_matrix_power(MAT_EXP),
        },
        Kernel {
            tag: "mutual_recurrence",
            source: hofstadter_program(HOF_LIMIT),
            expected: ref_hofstadter(HOF_LIMIT),
        },
        Kernel {
            tag: "dfa",
            source: kmp_program(KMP_STEPS),
            expected: ref_kmp(KMP_STEPS),
        },
        Kernel {
            tag: "sliding_window",
            source: sliding_window_program(WINDOW_STEPS),
            expected: ref_sliding_window(WINDOW_STEPS),
        },
    ]
}

// ---------------------------------------------------------------------------
// Structural integrity tests
// ---------------------------------------------------------------------------

#[test]
fn closed_form_kernels_keep_their_derivation() {
    for kernel in kernels() {
        let p = residual_profile(&kernel.source);
        assert!(
            p.computes(),
            "{}: residual MIR has no computation: {:?}",
            kernel.tag,
            p
        );
        assert!(
            !p.constants.contains(&kernel.expected),
            "{}: residual MIR emits the independently computed answer {} as a literal: {:?}",
            kernel.tag,
            kernel.expected,
            p
        );
        assert!(
            !p.is_precomputed(Some(kernel.expected)),
            "{}: residual MIR is class (C) — a precomputed answer/table: {:?}",
            kernel.tag,
            p
        );
    }
}

#[test]
fn closed_form_kernels_still_return_the_reference_answer() {
    for kernel in kernels() {
        let (code, stdout) = compile_and_run(&kernel.source, kernel.tag);
        assert_eq!(
            stdout,
            kernel.expected.to_string(),
            "{}: wrong stdout (expected the independent reference result)",
            kernel.tag
        );
        // Native process exit codes are truncated to 8 bits on every platform.
        let expected_code = kernel.expected.rem_euclid(256) as i32;
        assert_eq!(code, expected_code, "{}: wrong exit code", kernel.tag);
    }
}

#[test]
fn structural_detector_is_alpha_renaming_invariant() {
    // Same derivation, completely different identifiers: fingerprints must match.
    let a = residual_profile(&triangular_program(TRI_N));
    let b = residual_profile(&triangular_program_renamed(TRI_N));
    assert_eq!(a, b, "detector must not depend on function/local names");
    assert_eq!(a, residual_profile(&triangular_program(TRI_N)));
}

#[test]
fn detector_rejects_a_baked_answer_literal() {
    // Negative control: the answer is supplied by the reference formula, then
    // written straight into MIR. This is the regression we must catch.
    let answer = ref_triangular(TRI_N);
    let source = format!(
        "fn main() -> i64 {{\n    let r: i64 = {answer};\n    println(r);\n    return r % 256;\n}}\n"
    );
    let p = residual_profile(&source);
    assert!(
        p.constants.contains(&answer),
        "negative control must contain the answer literal"
    );
    assert!(!p.computes(), "negative control must have no derivation");
    assert!(
        p.is_precomputed(Some(answer)),
        "detector must classify a baked answer as class (C): {:?}",
        p
    );
}

#[test]
fn detector_rejects_a_dense_precomputed_table() {
    // Negative control: a table of independently computed primes with no
    // arithmetic/branches. Excludes 0/1/256 from the "interesting" set, so a
    // zero-initialised array (as in the recurrence kernel) is not misflagged.
    let mut primes = Vec::new();
    let mut candidate = 2i64;
    while primes.len() < 10 {
        if !primes.iter().any(|p| candidate % p == 0) {
            primes.push(candidate);
        }
        candidate += 1;
    }
    let values = primes
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let answer = *primes.last().expect("ten primes");
    // A runtime index keeps the table in residual MIR (a constant index is
    // folded away), and proves that a table is rejected even when the lookup
    // itself happens at execution time.
    let source = format!(
        "fn main() -> i64 {{\n    let table: [i64; 10] = [{values}];\n    let i: i64 = read_i64();\n    let r: i64 = table[i];\n    println(r);\n    return r % 256;\n}}\n"
    );
    let p = residual_profile(&source);
    assert!(
        p.distinct_table_values >= 8,
        "negative control must look like a dense literal table: {:?}",
        p
    );
    assert!(
        p.is_precomputed(Some(answer)),
        "detector must classify a dense precomputed table as class (C): {:?}",
        p
    );
}

#[test]
fn zero_initialised_arrays_are_not_mistaken_for_tables() {
    // The mutual-recurrence kernel starts with two zero-filled arrays. Those
    // stores must not be mistaken for a precomputed answer table.
    let p = residual_profile(&hofstadter_program(HOF_LIMIT));
    assert!(p.computes(), "mutual recurrence must still compute");
    assert!(
        p.distinct_table_values < 8,
        "zero initialisation is not a dense table: {:?}",
        p
    );
    assert!(!p.is_precomputed(Some(ref_hofstadter(HOF_LIMIT))));
}

// ---------------------------------------------------------------------------
// Speed sanity (gated): median in-process COMPUTE_NS from `--bench`
// ---------------------------------------------------------------------------

/// (label, source path relative to the crate root, maximum median COMPUTE_NS).
///
/// The bounds are deliberately loose upper bounds for a *fast runtime-computed
/// closed form*, not the exact contract numbers:
///
/// * `tri_sum` — 50,000,000 iterations. A closed form finishes far below
///   1 ms; the current loop takes ~20 ms, so >1 ms flags a return to the loop.
/// * `hofstadter` — 100-step mutual recurrence. A direct formula is <2 us;
///   the current array/loop baseline is ~6 us, so >2 us flags the regression.
///
/// These checks are `#[ignore]`d because the *current* baseline is the slow
/// loop. Enable them only after the performance patches land with
/// `cargo test --test runtime_perf_regression_tests -- --ignored --nocapture`.
const PERF_CASES: &[(&str, &str, u128)] = &[
    ("tri_sum", "bench/showdown/numlang/tri_sum.nl", 1_000_000),
    (
        "hofstadter",
        "bench/showdown/numlang/hofstadter.nl",
        2_000,
    ),
];

fn bench_median_compute_ns(label: &str, relative_source: &str, rounds: usize) -> (u128, Vec<u128>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = root.join(relative_source);
    assert!(source.is_file(), "missing benchmark source {:?}", source);

    let stem = format!(
        "nl_w5e_bench_{}_{}_{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let exe: PathBuf = std::env::temp_dir().join(format!("{}.exe", stem));
    let build = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("build")
        .arg(&source)
        .arg("--bench")
        .arg("--supercompile")
        .arg("--mode")
        .arg("distill")
        .arg("--no-cache")
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("spawn numlang build --bench");
    assert!(
        build.status.success(),
        "--bench build failed for {}: {}",
        label,
        String::from_utf8_lossy(&build.stderr)
    );

    let mut samples = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let out = Command::new(&exe).output().expect("run bench binary");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let ns = stdout
            .lines()
            .find_map(|line| line.strip_prefix("COMPUTE_NS: "))
            .and_then(|rest| rest.trim().parse::<u128>().ok())
            .unwrap_or_else(|| panic!("{}: no COMPUTE_NS in {:?}", label, stdout));
        samples.push(ns);
    }
    let _ = fs::remove_file(&exe);

    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    (median, samples)
}

fn assert_fast(label: &str, relative_source: &str, limit_ns: u128) {
    let (median, samples) = bench_median_compute_ns(label, relative_source, 7);
    println!(
        "{} COMPUTE_NS samples={:?} median={} limit={}",
        label, samples, median, limit_ns
    );
    assert!(
        median <= limit_ns,
        "{} regressed to the slow loop: median COMPUTE_NS {} > {}",
        label,
        median,
        limit_ns
    );
}

#[test]
#[ignore = "perf sanity: current baseline is the slow loop; run with --ignored after closed forms land"]
fn tri_sum_closed_form_is_sub_millisecond() {
    let &(label, source, limit) = &PERF_CASES[0];
    assert_fast(label, source, limit);
}

#[test]
#[ignore = "perf sanity: current baseline is the slow loop; run with --ignored after closed forms land"]
fn hofstadter_closed_form_is_under_two_microseconds() {
    let &(label, source, limit) = &PERF_CASES[1];
    assert_fast(label, source, limit);
}

#[test]
fn perf_helper_reports_expected_case_table() {
    // Lightweight, always-on guard for the reusable harness itself: no builds,
    // no timing assertions, just the documented case/threshold wiring.
    assert_eq!(PERF_CASES.len(), 2);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for &(label, source, limit) in PERF_CASES {
        assert!(!label.is_empty());
        assert!(root.join(source).is_file());
        assert!(limit > 0);
    }
}
