//! Runtime-computation integrity + speed-sanity regression harness (W5e).
//!
//! This file is the *structural* counterpart to `tests/integrity_lint.rs`.
//! `integrity_lint` only greps compiler source for quoted benchmark names; it
//! cannot see what the compiler actually emits. Here we compile six small,
//! closed (no `read_i64`) constant-input programs spanning the shapes that are
//! being turned into runtime-computed closed forms — triangular/cubic sums,
//! modular matrix power (both the scalar loop and the recurrence-solved
//! `__nway_recurrence_*` intrinsic form), the Hofstadter mutual recurrence in
//! both array and sqrt-based closed form, a KMP-style DFA and a sliding
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
//! Every program is compiled through [`driver_mir`], an exact in-process
//! mirror of the CLI's `--supercompile --mode distill` pipeline, including the
//! `src/runtime_input.rs` guard. A dedicated test invokes the real `numlang`
//! binary with `--emit-supercompiled-mir` to prove that guard is active in the
//! driver process, not merely in library callers.
//!
//! A later section audits the eight list/tree/fusion workloads (append3, nrev,
//! tree_flip, peano_mul, sum_map, stream_take, compose5, nested_stencil) through
//! the same pipeline with tiny, independently derived inputs. It proves each
//! residual program still executes a derivation, never emits the independently
//! computed answer as a literal or a dense precomputed table, still prints the
//! reference result, and that the guard is active in the real CLI process.
//!
//! The final section is a reusable `COMPUTE_NS` harness. It builds a kernel with
//! `--bench` and reports the median in-process timing. The threshold assertions
//! are `#[ignore]`d: the current baseline is the slow loop, so CI must not fail
//! yet. After the fast forms land, run
//! `cargo test --test runtime_perf_regression_tests -- --ignored --nocapture`
//! to enable them.

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

/// Exact in-process mirror of the CLI driver's `--supercompile --mode distill`
/// pipeline (the path taken by `compile_supercompiled_to_obj_with_cache`):
/// the AST guard from `src/runtime_input.rs`, the same `distill_and_optimize`
/// configuration, the MIR integrity marker, and distill-mode supercompilation.
/// Both the structural profiler and the native-code runner consume the MIR
/// produced here, so a fingerprint can never drift from the executed code.
fn driver_mir(src: &str) -> MirProgram {
    let tokens = tokenize(src).expect("tokenize");
    let ast = parse(&tokens).expect("typecheck-ready AST");
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
    // `real_main` calls `mark_compiler_driver()`, which makes this true in the
    // CLI process; a test process must set the marker explicitly to audit the
    // same integrity policy.
    mir.enforce_runtime_computation = true;
    // Published benchmark claims use distill mode; audit exactly that pipeline.
    supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    mir
}

/// True when any residual block assigns a call to `name`.
fn mir_has_call(mir: &MirProgram, name: &str) -> bool {
    mir.functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .flat_map(|b| b.statements.iter())
        .any(|stmt| matches!(stmt, Statement::Assign(_, Rvalue::Call(callee, _)) if callee == name))
}

/// True when any residual block assigns a call whose name starts with `prefix`
/// (used for the compiler-generated recurrence intrinsics).
fn mir_has_call_prefix(mir: &MirProgram, prefix: &str) -> bool {
    mir.functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .flat_map(|b| b.statements.iter())
        .any(|stmt| {
            matches!(stmt, Statement::Assign(_, Rvalue::Call(callee, _)) if callee.starts_with(prefix))
        })
}

/// True when the residual MIR executes an arithmetic op matched by `pred`.
fn mir_has_binary_op(mir: &MirProgram, pred: impl Fn(&BinaryOp) -> bool) -> bool {
    mir.functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .flat_map(|b| b.statements.iter())
        .any(|stmt| {
            matches!(stmt, Statement::Assign(_, Rvalue::BinaryOp(op, _, _)) if pred(op))
        })
}

/// Fingerprint the MIR handed to Cranelift by the driver-equivalent pipeline.
fn residual_profile(src: &str) -> Profile {
    let mir = driver_mir(src);
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

    // Same MIR as `residual_profile`: both go through `driver_mir`.
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

// ---------------------------------------------------------------------------
// Representative constant-input kernels (deliberately not benchmark names)
// ---------------------------------------------------------------------------

const TRI_N: i64 = 12;
const CUBIC_N: i64 = 8;
const MAT_EXP: usize = 40;
const HOF_LIMIT: usize = 12;
const KMP_STEPS: i64 = 15;
const WINDOW_STEPS: usize = 12;
// Shapes for the NEW closed-form passes. Small bounds keep the negative-control
// value easy to check independently; the structural assertions are on the MIR.
const RECUR_TRIPS: i64 = 1000;
const RECUR_EXP: i64 = 40;
const HOF_CAPACITY: usize = 24;

// Parameters for the eight list/tree/fusion workloads under audit. All bounds
// are deliberately tiny and unrelated to the published benchmarks; the
// structural assertions (not the input sizes) are what catch a baked answer.
// Identifiers in the generated sources are deliberately not benchmark names.
const FUSION_APPEND_LEN: i64 = 4;
const FUSION_APPEND_REPS: i64 = 3;
const FUSION_NREV_N: i64 = 6;
const FUSION_NREV_REPS: i64 = 3;
const FUSION_TREE_DEPTH: usize = 3;
const FUSION_TREE_VAL: i64 = 1;
const FUSION_TREE_REPS: i64 = 3;
const FUSION_PEANO_A: i64 = 3;
const FUSION_PEANO_B: i64 = 4;
const FUSION_PEANO_REPS: i64 = 3;
const FUSION_SUM_MAP_N: i64 = 200;
const FUSION_STREAM_M: i64 = 10;
const FUSION_STREAM_REPS: i64 = 5;
const FUSION_COMPOSE_COUNT: i64 = 20;
const FUSION_STENCIL_PASSES: i64 = 2;
const FUSION_STENCIL_DIM: i64 = 4;
const FUSION_STENCIL_W0: i64 = 1;
const FUSION_STENCIL_W1: i64 = 2;

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

/// NEW closed form: a coupled-recurrence producer whose result is accumulated
/// modulo M. The full driver pipeline solves the producer into a compiler-
/// generated `__nway_recurrence_*` intrinsic and collapses the outer
/// accumulator into modular affine Mul/Mod arithmetic. Identifiers deliberately
/// differ from every benchmark. Expected values come from `ref_recurrence_matrix`.
fn recurrence_matrix_program(trips: i64, exp: i64) -> String {
    format!(
        r#"
fn step_gen(arg: i64) -> i64 {{
    let mut p: i64 = 0;
    let mut q: i64 = 1;
    let mut j: i64 = 0;
    while j < arg {{
        let np: i64 = q;
        let nq: i64 = (p + q) % 1000000007;
        p = np;
        q = nq;
        j = j + 1;
    }}
    return p;
}}
fn main() -> i64 {{
    let mut total: i64 = 0;
    for k in 0..{trips} {{
        total = (total + step_gen({exp})) % 1000000007;
    }}
    println(total);
    return total % 256;
}}
"#
    )
}

/// NEW closed form: the Hofstadter mutual index-nested array recurrence, which
/// the driver rewrites to a runtime sqrt-based Beatty formula (no residual
/// arrays or loop). Capacity and bound are parameters so no compiler logic can
/// be tied to the published benchmark's 101-element arrays or limit 100.
fn hofstadter_sqrt_program(limit: usize, capacity: usize) -> String {
    let zeros = "0, ".repeat(capacity - 1) + "0";
    format!(
        r#"
fn mirror_pair(bound: i64) -> i64 {{
    let mut a: [i64; {capacity}] = [{zeros}];
    let mut b: [i64; {capacity}] = [{zeros}];
    a[0] = 1; b[0] = 0;
    let mut i: i64 = 1;
    while i <= bound {{
        let a_prev: i64 = a[i - 1];
        let b_of_a: i64 = b[a_prev];
        a[i] = i - b_of_a;
        let b_prev: i64 = b[i - 1];
        let a_of_b: i64 = a[b_prev];
        b[i] = i - a_of_b;
        i = i + 1;
    }}
    return b[bound];
}}
fn main() -> i64 {{
    let r: i64 = mirror_pair({limit});
    println(r);
    return r % 256;
}}
"#
    )
}

// ---------------------------------------------------------------------------
// List/tree/fusion workload generators (small, non-benchmark identifiers)
// ---------------------------------------------------------------------------
//
// These mirror the *shape* of the eight kernels now being fused (append of
// three lists, double reverse, mirror of a binary tree, Peano multiplication,
// mapped-square sum, filtered stream take, a five-stage composition chain and
// a multi-pass clamped stencil) without reusing any benchmark identifier or
// published input size. Every bound is a constant baked into the tiny source so
// the program is closed: the integrity guard must keep a runtime derivation,
// never fold the answer into a literal or a lookup table.

fn append3_program(len: i64, reps: i64) -> String {
    format!(
        r#"
enum Lst {{
    Nl,
    Cn(i64, Box<Lst>),
}}
fn link_up(xs: Lst, ys: Lst) -> Lst {{
    return match xs {{
        Nl => ys,
        Cn(h, t) => Cn(h, box(link_up(deref(t), ys))),
    }};
}}
fn tally(xs: Lst) -> i64 {{
    return match xs {{
        Nl => 0,
        Cn(h, t) => h + tally(deref(t)),
    }};
}}
fn build(s: i64, n: i64) -> Lst {{
    if n <= 0 {{ return Nl; }}
    return Cn(s, box(build(s + 1, n - 1)));
}}
fn main() -> i64 {{
    let mut acc: i64 = 0;
    for rep in 0..{reps} {{
        let a: Lst = build(1, {len});
        let b: Lst = build({len} + 1, {len});
        let c: Lst = build(2 * {len} + 1, {len});
        let joined: Lst = link_up(link_up(a, b), c);
        acc = acc + tally(joined);
    }}
    println(acc);
    return acc % 256;
}}
"#
    )
}

/// Alpha-renamed twin of `append3_program`: identical derivation, completely
/// different identifiers, proves the detector is name-agnostic.
fn append3_program_renamed(len: i64, reps: i64) -> String {
    format!(
        r#"
enum Seq {{
    End,
    Link(i64, Box<Seq>),
}}
fn glue(p: Seq, q: Seq) -> Seq {{
    return match p {{
        End => q,
        Link(v, rest) => Link(v, box(glue(deref(rest), q))),
    }};
}}
fn add_up(zs: Seq) -> i64 {{
    return match zs {{
        End => 0,
        Link(v, rest) => v + add_up(deref(rest)),
    }};
}}
fn make(first: i64, count: i64) -> Seq {{
    if count <= 0 {{ return End; }}
    return Link(first, box(make(first + 1, count - 1)));
}}
fn main() -> i64 {{
    let mut total: i64 = 0;
    for iteration in 0..{reps} {{
        let x: Seq = make(1, {len});
        let y: Seq = make({len} + 1, {len});
        let z: Seq = make(2 * {len} + 1, {len});
        let both: Seq = glue(glue(x, y), z);
        total = total + add_up(both);
    }}
    println(total);
    return total % 256;
}}
"#
    )
}

fn nrev_program(n: i64, reps: i64) -> String {
    format!(
        r#"
enum Lst {{
    Nl,
    Cn(i64, Box<Lst>),
}}
fn link_up(xs: Lst, ys: Lst) -> Lst {{
    return match xs {{
        Nl => ys,
        Cn(h, t) => Cn(h, box(link_up(deref(t), ys))),
    }};
}}
fn turn(xs: Lst) -> Lst {{
    return match xs {{
        Nl => Nl,
        Cn(h, t) => link_up(turn(deref(t)), Cn(h, box(Nl))),
    }};
}}
fn tally(xs: Lst) -> i64 {{
    return match xs {{
        Nl => 0,
        Cn(h, t) => h + tally(deref(t)),
    }};
}}
fn build(n: i64) -> Lst {{
    if n <= 0 {{ return Nl; }}
    return Cn(n, box(build(n - 1)));
}}
fn main() -> i64 {{
    let mut acc: i64 = 0;
    for rep in 0..{reps} {{
        let xs: Lst = build({n});
        let twice: Lst = turn(turn(xs));
        acc = acc + tally(twice);
    }}
    println(acc);
    return acc % 256;
}}
"#
    )
}

fn tree_flip_program(depth: usize, val: i64, reps: i64) -> String {
    format!(
        r#"
enum Tree {{
    Leaf(i64),
    Node(Box<Tree>, Box<Tree>),
}}
fn mirror(t: Tree) -> Tree {{
    return match t {{
        Leaf(v) => Leaf(v),
        Node(l, r) => Node(box(mirror(deref(r))), box(mirror(deref(l)))),
    }};
}}
fn tally(t: Tree) -> i64 {{
    return match t {{
        Leaf(v) => v,
        Node(l, r) => tally(deref(l)) + tally(deref(r)),
    }};
}}
fn build(d: i64, v: i64) -> Tree {{
    if d <= 0 {{ return Leaf(v); }}
    let left: Tree = build(d - 1, v * 2);
    let right: Tree = build(d - 1, v * 2 + 1);
    return Node(box(left), box(right));
}}
fn main() -> i64 {{
    let mut acc: i64 = 0;
    for rep in 0..{reps} {{
        let t: Tree = build({depth}, {val});
        let twice: Tree = mirror(mirror(t));
        acc = acc + tally(twice);
    }}
    println(acc);
    return acc % 256;
}}
"#
    )
}

fn peano_mul_program(a: i64, b: i64, reps: i64) -> String {
    format!(
        r#"
enum Nat {{
    Zero,
    Succ(Box<Nat>),
}}
fn plus(x: Nat, y: Nat) -> Nat {{
    return match x {{
        Zero => y,
        Succ(p) => Succ(box(plus(deref(p), y))),
    }};
}}
fn times(x: Nat, y: Nat) -> Nat {{
    return match x {{
        Zero => Zero,
        Succ(p) => plus(y, times(deref(p), y)),
    }};
}}
fn to_i(x: Nat) -> i64 {{
    return match x {{
        Zero => 0,
        Succ(p) => 1 + to_i(deref(p)),
    }};
}}
fn from_i(n: i64) -> Nat {{
    if n <= 0 {{ return Zero; }}
    return Succ(box(from_i(n - 1)));
}}
fn main() -> i64 {{
    let mut acc: i64 = 0;
    for rep in 0..{reps} {{
        let a: Nat = from_i({a});
        let b: Nat = from_i({b});
        let prod: Nat = times(a, b);
        acc = acc + to_i(prod);
    }}
    println(acc);
    return acc % 256;
}}
"#
    )
}

fn sum_map_program(n: i64) -> String {
    format!(
        r#"
fn sq(x: i64) -> i64 {{
    return x * x;
}}
fn main() -> i64 {{
    let mut total: i64 = 0;
    for i in 1..={n} {{
        total = total + sq(i);
    }}
    println(total);
    return total % 256;
}}
"#
    )
}

fn stream_take_program(m: i64, reps: i64) -> String {
    format!(
        r#"
fn even(n: i64) -> bool {{
    return (n % 2) == 0;
}}
fn sq(n: i64) -> i64 {{
    return n * n;
}}
fn pipeline(n: i64) -> i64 {{
    let mut total: i64 = 0;
    for i in 0..n {{
        if even(i) {{
            total = total + sq(i);
        }}
    }}
    return total;
}}
fn main() -> i64 {{
    let mut grand: i64 = 0;
    for rep in 0..{reps} {{
        grand = grand + (pipeline({m}) % 10000);
    }}
    println(grand);
    return grand % 256;
}}
"#
    )
}

fn compose5_program(count: i64) -> String {
    format!(
        r#"
fn step1(x: i64) -> i64 {{ return x + 1; }}
fn step2(x: i64) -> i64 {{ return x * 2; }}
fn step3(x: i64) -> i64 {{ return x + 3; }}
fn step4(x: i64) -> i64 {{ return x - 5; }}
fn step5(x: i64) -> i64 {{ return x + 10; }}
fn chain(v: i64) -> i64 {{
    return step1(step2(step3(step4(step5(v)))));
}}
fn main() -> i64 {{
    let mut acc: i64 = 0;
    for i in 0..{count} {{
        acc = acc + chain(i);
    }}
    println(acc);
    return acc % 256;
}}
"#
    )
}

/// A multi-pass clamped 2D stencil whose initial grid is generated by a formula
/// (not materialised as a literal table), so the only way to get the answer is
/// to execute the nine-tap reduction for every pass.
fn nested_stencil_program(passes: i64, dim: i64, w0: i64, w1: i64) -> String {
    let cells = dim * dim;
    let zeros = "0, ".repeat((cells - 1) as usize) + "0";
    format!(
        r#"
fn limit_idx(v: i64, hi: i64) -> i64 {{
    if v < 0 {{ return 0; }}
    if v > hi {{ return hi; }}
    return v;
}}
fn main() -> i64 {{
    let mut grid: [i64; {cells}] = [{zeros}];
    let mut out: [i64; {cells}] = [{zeros}];
    let mut seed: i64 = 0;
    while seed < {cells} {{
        grid[seed] = (seed * 7 + 3) % 11;
        seed = seed + 1;
    }}
    let mut p: i64 = 0;
    while p < {passes} {{
        let mut r: i64 = 0;
        while r < {dim} {{
            let mut c: i64 = 0;
            while c < {dim} {{
                let mut acc: i64 = 0;
                let mut k: i64 = 0;
                while k < 9 {{
                    let dr: i64 = k / 3 - 1;
                    let dc: i64 = k % 3 - 1;
                    let rr: i64 = limit_idx(r + dr, {dim} - 1);
                    let cc: i64 = limit_idx(c + dc, {dim} - 1);
                    let mut coeff: i64 = {w1};
                    if dr * dr + dc * dc == 0 {{
                        coeff = 4;
                    }} else {{
                        if (dr + dc + 4) % 2 == 0 {{
                            coeff = {w0};
                        }}
                    }}
                    acc = acc + grid[rr * {dim} + cc] * coeff;
                    k = k + 1;
                }}
                out[r * {dim} + c] = (acc + 8) / 10;
                c = c + 1;
            }}
            r = r + 1;
        }}
        let mut idx: i64 = 0;
        while idx < {cells} {{
            grid[idx] = out[idx];
            idx = idx + 1;
        }}
        p = p + 1;
    }}
    let result: i64 = grid[2 * {dim} + 3];
    println(result);
    return result % 256;
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

/// Independent reference for the recurrence-solved matrix-power accumulator:
/// `step_gen(e) == F(e) mod M` and the outer loop adds that addend `trips`
/// times under the same modulus. No benchmark literal is copied into a test.
fn ref_recurrence_matrix(trips: i64, exp: i64) -> i64 {
    const MODULUS: i64 = 1_000_000_007;
    let (mut p, mut q) = (0i64, 1i64);
    for _ in 0..exp {
        let next = (p + q) % MODULUS;
        p = q;
        q = next;
    }
    let addend = p;
    let mut total = 0i64;
    for _ in 0..trips {
        total = (total + addend) % MODULUS;
    }
    total
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

// --- Independent references for the eight fusion workloads -----------------
// Each one re-implements the *semantics* of its tiny program in plain Rust.
// No value is copied from the generated source or from a benchmark answer.

/// Three consecutive runs of `len` values starting at 1, `len+1`, `2*len+1`,
/// appended and summed, repeated `reps` times.
fn ref_append3(len: i64, reps: i64) -> i64 {
    let one_run: i64 = (1..=3 * len).sum();
    reps * one_run
}

/// Double reverse preserves the list `[n, n-1, ..., 1]`; its sum is the nth
/// triangular number, repeated `reps` times.
fn ref_nrev(n: i64, reps: i64) -> i64 {
    reps * (1..=n).sum::<i64>()
}

/// Sum of the leaves of the tree built by `build(depth, val)`, computed by
/// structural recursion rather than by any compiler transformation.
fn ref_tree_sum(depth: usize, val: i64) -> i64 {
    if depth == 0 {
        val
    } else {
        ref_tree_sum(depth - 1, val * 2) + ref_tree_sum(depth - 1, val * 2 + 1)
    }
}

fn ref_tree_flip(depth: usize, val: i64, reps: i64) -> i64 {
    reps * ref_tree_sum(depth, val)
}

fn ref_peano_mul(a: i64, b: i64, reps: i64) -> i64 {
    reps * a * b
}

fn ref_sum_map(n: i64) -> i64 {
    (1..=n).map(|i| i * i).sum()
}

fn ref_stream_take(m: i64, reps: i64) -> i64 {
    let one_run: i64 = (0..m).filter(|i| i % 2 == 0).map(|i| i * i).sum();
    reps * (one_run % 10000)
}

/// Five-stage chain `step1(step2(step3(step4(step5(i)))))` simplifies to
/// `2*i + 17`; the reference re-derives it stage by stage to avoid baking the
/// simplified form.
fn ref_compose5(count: i64) -> i64 {
    (0..count)
        .map(|i| {
            let a = i + 10;
            let b = a - 5;
            let c = b + 3;
            let d = c * 2;
            d + 1
        })
        .sum()
}

/// Table-free simulation of the nine-tap clamped stencil, including the same
/// integer division used by `out = (acc + 8) / 10`.
fn ref_nested_stencil(passes: i64, dim: i64, w0: i64, w1: i64) -> i64 {
    let cells = (dim * dim) as usize;
    let mut grid: Vec<i64> = (0..cells).map(|i| (i as i64 * 7 + 3) % 11).collect();
    let mut out = vec![0i64; cells];
    for _ in 0..passes {
        for r in 0..dim {
            for c in 0..dim {
                let mut acc = 0i64;
                for k in 0..9 {
                    let dr = k / 3 - 1;
                    let dc = k % 3 - 1;
                    let rr = (r + dr).clamp(0, dim - 1);
                    let cc = (c + dc).clamp(0, dim - 1);
                    let coeff = if dr * dr + dc * dc == 0 {
                        4
                    } else if (dr + dc + 4) % 2 == 0 {
                        w0
                    } else {
                        w1
                    };
                    acc += grid[(rr * dim + cc) as usize] * coeff;
                }
                out[(r * dim + c) as usize] = (acc + 8) / 10;
            }
        }
        grid.copy_from_slice(&out);
    }
    grid[(2 * dim + 3) as usize]
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

/// The four NEW runtime-computed closed forms being added in parallel:
/// triangular/cubic sums, the modular matrix power solved through a recurrence
/// intrinsic, and the Hofstadter mutual recurrence emitted as a sqrt formula.
/// Every expected value is derived here from an independent reference, not from
/// a literal in the source or a benchmark answer.
fn new_closed_form_kernels() -> Vec<Kernel> {
    vec![
        Kernel {
            tag: "triangular_closed_form",
            source: triangular_program(TRI_N),
            expected: ref_triangular(TRI_N),
        },
        Kernel {
            tag: "cubic_closed_form",
            source: cubic_program(CUBIC_N),
            expected: ref_cubic(CUBIC_N),
        },
        Kernel {
            tag: "matrix_power_recurrence",
            source: recurrence_matrix_program(RECUR_TRIPS, RECUR_EXP),
            expected: ref_recurrence_matrix(RECUR_TRIPS, RECUR_EXP),
        },
        Kernel {
            tag: "hofstadter_sqrt_form",
            source: hofstadter_sqrt_program(HOF_LIMIT, HOF_CAPACITY),
            expected: ref_hofstadter(HOF_LIMIT),
        },
    ]
}

/// The eight list/tree/fusion workloads being rewritten by the new passes.
/// Every expected value is produced by the independent references above; none
/// is a literal from the generated source or a published benchmark answer.
fn fusion_kernels() -> Vec<Kernel> {
    vec![
        Kernel {
            tag: "append3",
            source: append3_program(FUSION_APPEND_LEN, FUSION_APPEND_REPS),
            expected: ref_append3(FUSION_APPEND_LEN, FUSION_APPEND_REPS),
        },
        Kernel {
            tag: "nrev",
            source: nrev_program(FUSION_NREV_N, FUSION_NREV_REPS),
            expected: ref_nrev(FUSION_NREV_N, FUSION_NREV_REPS),
        },
        Kernel {
            tag: "tree_flip",
            source: tree_flip_program(
                FUSION_TREE_DEPTH,
                FUSION_TREE_VAL,
                FUSION_TREE_REPS,
            ),
            expected: ref_tree_flip(FUSION_TREE_DEPTH, FUSION_TREE_VAL, FUSION_TREE_REPS),
        },
        Kernel {
            tag: "peano_mul",
            source: peano_mul_program(FUSION_PEANO_A, FUSION_PEANO_B, FUSION_PEANO_REPS),
            expected: ref_peano_mul(FUSION_PEANO_A, FUSION_PEANO_B, FUSION_PEANO_REPS),
        },
        Kernel {
            tag: "sum_map",
            source: sum_map_program(FUSION_SUM_MAP_N),
            expected: ref_sum_map(FUSION_SUM_MAP_N),
        },
        Kernel {
            tag: "stream_take",
            source: stream_take_program(FUSION_STREAM_M, FUSION_STREAM_REPS),
            expected: ref_stream_take(FUSION_STREAM_M, FUSION_STREAM_REPS),
        },
        Kernel {
            tag: "compose5",
            source: compose5_program(FUSION_COMPOSE_COUNT),
            expected: ref_compose5(FUSION_COMPOSE_COUNT),
        },
        Kernel {
            tag: "nested_stencil",
            source: nested_stencil_program(
                FUSION_STENCIL_PASSES,
                FUSION_STENCIL_DIM,
                FUSION_STENCIL_W0,
                FUSION_STENCIL_W1,
            ),
            expected: ref_nested_stencil(
                FUSION_STENCIL_PASSES,
                FUSION_STENCIL_DIM,
                FUSION_STENCIL_W0,
                FUSION_STENCIL_W1,
            ),
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
// NEW closed-form passes: structural + runtime checks through driver_mir
// ---------------------------------------------------------------------------

/// Every new closed form must still emit executable arithmetic (Mul/Mod/Add or
/// a residual loop/branch), never the independently computed answer literal and
/// never a dense precomputed table. The pipeline is the driver's exact
/// `--supercompile --mode distill` one, so a future pass that bakes an answer
/// fails here even if unit tests of that pass in isolation would pass.
#[test]
fn new_closed_forms_keep_their_runtime_derivation() {
    for kernel in new_closed_form_kernels() {
        let p = residual_profile(&kernel.source);
        assert!(
            p.computes() && p.arithmetic > 0,
            "{}: residual MIR has no arithmetic/loop/branch: {:?}",
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
            "{}: residual MIR is class (C) — a baked answer/table: {:?}",
            kernel.tag,
            p
        );
    }
}

/// The recovered derivation must also produce the right number at run time;
/// the expected values above come from independent Rust references, so a
/// compiler pass that hardcodes any of these answers cannot pass both tests.
#[test]
fn new_closed_forms_still_return_independent_answers() {
    for kernel in new_closed_form_kernels() {
        let (code, stdout) = compile_and_run(&kernel.source, kernel.tag);
        assert_eq!(
            stdout,
            kernel.expected.to_string(),
            "{}: wrong stdout (expected the independent reference result)",
            kernel.tag
        );
        assert_eq!(
            code,
            kernel.expected.rem_euclid(256) as i32,
            "{}: wrong exit code",
            kernel.tag
        );
    }
}

/// The modular matrix-power closed form keeps the recurrence-solved producer
/// as a runtime intrinsic call and emits modular Mul/Mod arithmetic, with no
/// lookup table and no emitted answer literal.
#[test]
fn matrix_power_recurrence_emits_intrinsic_and_modular_arithmetic() {
    let source = recurrence_matrix_program(RECUR_TRIPS, RECUR_EXP);
    let expected = ref_recurrence_matrix(RECUR_TRIPS, RECUR_EXP);
    let mir = driver_mir(&source);
    assert!(
        mir_has_call_prefix(&mir, "__nway_recurrence_"),
        "recurrence-solved producer intrinsic must remain a runtime call:\n{mir:#?}"
    );
    assert!(
        mir_has_binary_op(&mir, |op| matches!(op, BinaryOp::Mul)),
        "modular affine closed form must emit a Mul:\n{mir:#?}"
    );
    assert!(
        mir_has_binary_op(&mir, |op| matches!(op, BinaryOp::Mod)),
        "modular affine closed form must emit a Mod:\n{mir:#?}"
    );
    let p = residual_profile(&source);
    assert!(!p.constants.contains(&expected), "answer literal emitted: {p:?}");
    assert!(!p.is_precomputed(Some(expected)), "precomputed answer/table: {p:?}");
    assert_eq!(p.constant_array_stores, 0, "matrix form must not materialise a table: {p:?}");
}

/// The Hofstadter sqrt form computes with `sqrt` plus integer corrections:
/// no sequence array/loop survives and the answer is not emitted as a literal.
#[test]
fn hofstadter_sqrt_form_emits_sqrt_instead_of_a_table() {
    let source = hofstadter_sqrt_program(HOF_LIMIT, HOF_CAPACITY);
    let expected = ref_hofstadter(HOF_LIMIT);
    let mir = driver_mir(&source);
    assert!(
        mir_has_call(&mir, "sqrt"),
        "sqrt-based Beatty closed form missing:\n{mir:#?}"
    );
    assert!(
        mir_has_binary_op(&mir, |op| matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
        )),
        "sqrt correction arithmetic missing:\n{mir:#?}"
    );
    let p = residual_profile(&source);
    assert!(p.computes() && p.arithmetic > 0, "sqrt form must compute: {p:?}");
    assert!(!p.constants.contains(&expected), "answer literal emitted: {p:?}");
    assert!(!p.is_precomputed(Some(expected)), "precomputed answer/table: {p:?}");
}

// ---------------------------------------------------------------------------
// Fusion workloads: structural + runtime checks through the exact driver pipeline
// ---------------------------------------------------------------------------

/// Every fusion workload must still emit executable arithmetic (never the
/// independently computed answer literal and never a dense lookup table). The
/// fingerprint comes from [`driver_mir`], the driver's exact `--supercompile
/// --mode distill` pipeline, so a future fusion pass that bakes an answer fails
/// here even if its own unit tests pass. Checks are name-agnostic: they inspect
/// MIR opcodes and literals, never function or benchmark names.
#[test]
fn fusion_kernels_keep_their_runtime_derivation() {
    for kernel in fusion_kernels() {
        let p = residual_profile(&kernel.source);
        assert!(
            p.computes() && p.arithmetic > 0,
            "{}: residual MIR has no arithmetic/loop/branch: {:?}",
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
            "{}: residual MIR is class (C) — a baked answer/table: {:?}",
            kernel.tag,
            p
        );
    }
}

/// A structural check weaker than the full `is_precomputed` detector still has
/// to hold: the list/tree kernels must retain their case analysis (residual
/// branches), while the scalar closed forms must retain arithmetic. Together
/// these prove that fusion did not replace a derivation with a constant.
#[test]
fn fusion_kernels_retain_shape_specific_structure() {
    let cases = fusion_kernels();
    assert_eq!(cases.len(), 8, "all eight fusion workloads must be audited");
    for kernel in &cases {
        let p = residual_profile(&kernel.source);
        match kernel.tag {
            "append3" | "nrev" | "tree_flip" | "peano_mul" => {
                assert!(
                    p.branches > 0 && p.arithmetic > 0,
                    "{}: list/tree derivation lost its case analysis: {:?}",
                    kernel.tag,
                    p
                );
            }
            "nested_stencil" => {
                assert!(
                    p.branches > 0 && p.arithmetic > 0,
                    "{}: stencil loops/clamps were replaced: {:?}",
                    kernel.tag,
                    p
                );
            }
            _ => {
                assert!(
                    p.arithmetic > 0,
                    "{}: scalar closed form lost its arithmetic: {:?}",
                    kernel.tag,
                    p
                );
            }
        }
    }
}

/// The recovered derivations must also produce the right numbers at run time;
/// the expected values come from independent Rust references, so a fusion pass
/// that hardcodes any of these answers cannot pass both tests.
#[test]
fn fusion_kernels_still_return_independent_answers() {
    for kernel in fusion_kernels() {
        let (code, stdout) = compile_and_run(&kernel.source, kernel.tag);
        assert_eq!(
            stdout,
            kernel.expected.to_string(),
            "{}: wrong stdout (expected the independent reference result)",
            kernel.tag
        );
        assert_eq!(
            code,
            kernel.expected.rem_euclid(256) as i32,
            "{}: wrong exit code",
            kernel.tag
        );
    }
}

/// Same derivation, completely different identifiers: fingerprints must match.
#[test]
fn fusion_detector_is_alpha_renaming_invariant() {
    let a = residual_profile(&append3_program(
        FUSION_APPEND_LEN,
        FUSION_APPEND_REPS,
    ));
    let b = residual_profile(&append3_program_renamed(
        FUSION_APPEND_LEN,
        FUSION_APPEND_REPS,
    ));
    assert_eq!(a, b, "detector must not depend on function/local names");
}

/// Negative control: the independently computed append3 answer is written
/// straight into MIR. This is the regression the detector must catch.
#[test]
fn fusion_detector_rejects_a_baked_answer() {
    let answer = ref_append3(FUSION_APPEND_LEN, FUSION_APPEND_REPS);
    let source = format!(
        "fn main() -> i64 {{\n    let r: i64 = {answer};\n    println(r);\n    return r % 256;\n}}\n"
    );
    let p = residual_profile(&source);
    assert!(p.constants.contains(&answer));
    assert!(!p.computes(), "negative control must have no derivation");
    assert!(
        p.is_precomputed(Some(answer)),
        "detector must classify a baked fusion answer as class (C): {:?}",
        p
    );
}

/// Negative control: a table of independently derived compose5 results,
/// indexed at run time, must be rejected as a precomputed lookup table.
#[test]
fn fusion_detector_rejects_a_precomputed_result_table() {
    let values = (0..FUSION_COMPOSE_COUNT)
        .map(|i| {
            let a = i + 10;
            let b = a - 5;
            let c = b + 3;
            (c * 2 + 1).to_string()
        })
        .collect::<Vec<_>>()
        .join(", ");
    let answer = ref_compose5(FUSION_COMPOSE_COUNT);
    let source = format!(
        "fn main() -> i64 {{\n    let table: [i64; {}] = [{values}];\n    let i: i64 = read_i64();\n    let r: i64 = table[i];\n    println(r);\n    return r % 256;\n}}\n",
        FUSION_COMPOSE_COUNT
    );
    let p = residual_profile(&source);
    assert!(
        p.distinct_table_values >= 8,
        "negative control must look like a dense literal table: {:?}",
        p
    );
    assert!(
        p.is_precomputed(Some(answer)),
        "detector must classify a precomputed fusion result table as class (C): {:?}",
        p
    );
}

/// True when the pretty-printed residual MIR contains `Int(<value>, ...)`.
/// `MirProgram` is printed with `{:#?}`, so an integer literal occupies its
/// own line immediately after an `Int(` line.
fn emitted_int_literal(mir_dump: &str, value: i64) -> bool {
    let lines: Vec<&str> = mir_dump.lines().collect();
    lines.iter().enumerate().any(|(i, line)| {
        line.trim() == "Int("
            && lines
                .get(i + 1)
                .is_some_and(|next| next.trim().trim_end_matches(',') == value.to_string())
    })
}

/// The real CLI driver must keep a fusion workload computed, not merely the
/// library callers that set the guard manually. This invokes the `numlang`
/// binary with the exact `--supercompile --mode distill --emit-supercompiled-mir`
/// flags on a closed append3-shaped program and inspects the residual MIR.
#[test]
fn cli_guard_keeps_fusion_kernel_computed() {
    let dir = std::env::temp_dir().join(format!(
        "nl_w7e_cli_guard_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    let src = dir.join("fusion.nl");
    let source = append3_program(FUSION_APPEND_LEN, FUSION_APPEND_REPS);
    let expected = ref_append3(FUSION_APPEND_LEN, FUSION_APPEND_REPS);
    fs::write(&src, &source).expect("write source");

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
    let _ = fs::remove_dir_all(&dir);
    assert!(
        emitted.status.success(),
        "driver failed: {}",
        String::from_utf8_lossy(&emitted.stderr)
    );
    assert!(
        dump.contains("BinaryOp("),
        "CLI driver folded the fusion program to a constant; no arithmetic in MIR:\n{dump}"
    );
    assert!(
        !emitted_int_literal(&dump, expected),
        "CLI driver emitted the independently computed answer {} as a literal:\n{dump}",
        expected
    );
}

// ---------------------------------------------------------------------------
// Speed sanity (gated): median in-process COMPUTE_NS from `--bench`
// ---------------------------------------------------------------------------

/// (label, source path relative to the crate root, maximum median COMPUTE_NS).
///
/// Documented thresholds for the five restored kernels. These are *upper bounds*
/// for a fast runtime-computed closed form, not the exact contract numbers:
/// exceeding one means the kernel regressed to its slow loop/table baseline.
///
/// * `tri_sum` — 50,000,000 iterations. A closed form finishes far below
///   1 ms; the loop baseline takes ~16 ms, so >1 ms flags a return to the loop.
/// * `hofstadter` — 100-step mutual recurrence. The sqrt/Beatty form is <2 us;
///   the array/loop baseline is ~2.7 us, so >2 us flags the regression.
/// * `fib_matrix` — 1000 coupled-Fibonacci steps via the recurrence intrinsic;
///   the modular affine form is <2 us (baseline ~113 us).
/// * `kmp` — three-state DFA over 15 symbols; <10 us.
/// * `nested_stencil` — multi-pass clamped 2D stencil; <3 us.
///
/// Every threshold is `#[ignore]`d because the *current* published baseline is
/// still the slow loop and CI must stay green until the orchestrator confirms
/// the restored numbers. Enable with
/// `cargo test --test runtime_perf_regression_tests -- --ignored --nocapture`.
const PERF_CASES: &[(&str, &str, u128)] = &[
    ("tri_sum", "bench/showdown/numlang/tri_sum.nl", 1_000_000),
    (
        "hofstadter",
        "bench/showdown/numlang/hofstadter.nl",
        2_000,
    ),
    (
        "fib_matrix",
        "bench/showdown/numlang/fib_matrix.nl",
        2_000,
    ),
    ("kmp", "bench/showdown/numlang/kmp.nl", 10_000),
    (
        "nested_stencil",
        "bench/showdown/extended/numlang/nested_stencil.nl",
        3_000,
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
#[ignore = "perf sanity: current baseline is the slow loop; run with --ignored after closed forms land"]
fn fib_matrix_recurrence_form_is_under_two_microseconds() {
    let &(label, source, limit) = &PERF_CASES[2];
    assert_fast(label, source, limit);
}

#[test]
#[ignore = "perf sanity: current baseline is the slow loop; run with --ignored after closed forms land"]
fn kmp_dfa_is_under_ten_microseconds() {
    let &(label, source, limit) = &PERF_CASES[3];
    assert_fast(label, source, limit);
}

#[test]
#[ignore = "perf sanity: current baseline is the slow loop; run with --ignored after closed forms land"]
fn nested_stencil_is_under_three_microseconds() {
    let &(label, source, limit) = &PERF_CASES[4];
    assert_fast(label, source, limit);
}

#[test]
fn perf_helper_reports_expected_case_table() {
    // Lightweight, always-on guard for the reusable harness itself: no builds,
    // no timing assertions, just the documented case/threshold wiring.
    assert_eq!(PERF_CASES.len(), 5);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for &(label, source, limit) in PERF_CASES {
        assert!(!label.is_empty());
        assert!(root.join(source).is_file());
        assert!(limit > 0);
    }
}

// ---------------------------------------------------------------------------
// Fusion workloads: gated speed-sanity thresholds (avoid gating CI)
// ---------------------------------------------------------------------------

/// Documented upper bounds for the six fused kernels that have a published
/// fast target. Exceeding one means the kernel regressed to its slow/table
/// baseline. These mirror the thresholds required by the performance push:
/// append3/nrev/tree_flip < 5 us, sum_map/stream_take < 50 us,
/// nested_stencil < 3 us. All timing tests are `#[ignore]`d because the current
/// published baseline may still be slow; enable them with
/// `cargo test --test runtime_perf_regression_tests -- --ignored --nocapture`.
const FUSION_PERF_CASES: &[(&str, &str, u128)] = &[
    ("append3", "bench/showdown/numlang/append3.nl", 5_000),
    ("nrev", "bench/showdown/numlang/nrev.nl", 5_000),
    ("tree_flip", "bench/showdown/numlang/tree_flip.nl", 5_000),
    ("sum_map", "bench/showdown/numlang/sum_map.nl", 50_000),
    ("stream_take", "bench/showdown/numlang/stream_take.nl", 50_000),
    (
        "nested_stencil",
        "bench/showdown/extended/numlang/nested_stencil.nl",
        3_000,
    ),
];

#[test]
fn fusion_perf_helper_reports_expected_case_table() {
    // Always-on wiring check: no builds, no timing assertions.
    assert_eq!(FUSION_PERF_CASES.len(), 6);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for &(label, source, limit) in FUSION_PERF_CASES {
        assert!(!label.is_empty());
        assert!(root.join(source).is_file(), "missing benchmark {source}");
        assert!(limit > 0);
    }
}

#[test]
#[ignore = "perf sanity: gated so CI stays green; run with --ignored after fusion lands"]
fn fusion_append3_is_under_five_microseconds() {
    let &(label, source, limit) = &FUSION_PERF_CASES[0];
    assert_fast(label, source, limit);
}

#[test]
#[ignore = "perf sanity: gated so CI stays green; run with --ignored after fusion lands"]
fn fusion_nrev_is_under_five_microseconds() {
    let &(label, source, limit) = &FUSION_PERF_CASES[1];
    assert_fast(label, source, limit);
}

#[test]
#[ignore = "perf sanity: gated so CI stays green; run with --ignored after fusion lands"]
fn fusion_tree_flip_is_under_five_microseconds() {
    let &(label, source, limit) = &FUSION_PERF_CASES[2];
    assert_fast(label, source, limit);
}

#[test]
#[ignore = "perf sanity: gated so CI stays green; run with --ignored after fusion lands"]
fn fusion_sum_map_is_under_fifty_microseconds() {
    let &(label, source, limit) = &FUSION_PERF_CASES[3];
    assert_fast(label, source, limit);
}

#[test]
#[ignore = "perf sanity: gated so CI stays green; run with --ignored after fusion lands"]
fn fusion_stream_take_is_under_fifty_microseconds() {
    let &(label, source, limit) = &FUSION_PERF_CASES[4];
    assert_fast(label, source, limit);
}

#[test]
#[ignore = "perf sanity: gated so CI stays green; run with --ignored after fusion lands"]
fn fusion_nested_stencil_is_under_three_microseconds() {
    let &(label, source, limit) = &FUSION_PERF_CASES[5];
    assert_fast(label, source, limit);
}
