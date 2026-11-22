//! Profiling / equivalence helpers for supercompiler specialization latency.
//!
//! Run the profile target with:
//!   NUMLANG_PROFILE_SUPERCOMPILE=1 cargo test --test specialization_latency_tests -- --nocapture
//!
//! Instrumentation is completely inert when the environment variable is absent,
//! so the same tests can be run normally as regression checks.

use std::fs;
use std::path::{Path, PathBuf};

use numlang::compiler::{distill_and_optimize, CompilerConfig};
use numlang::mir::lower::{lower_program, MirProgram, Statement};
use numlang::mir::Projection;
use numlang::mir::supercompiler::{supercompile_mir_program, SupercompilerStats};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::typed_ast::TypedProgram;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn load_typed(path: &Path) -> TypedProgram {
    let source = fs::read_to_string(root().join(path)).expect("read benchmark source");
    let tokens = tokenize(&source).expect("tokenize");
    let ast = parse(&tokens).expect("parse");
    typecheck(&ast).expect("typecheck")
}

fn specialize_with_stats(path: &Path) -> (MirProgram, SupercompilerStats) {
    let mut typed = load_typed(path);
    let config = CompilerConfig {
        ho_distill: true,
        supercompile: true,
        ..CompilerConfig::default()
    };
    distill_and_optimize(&mut typed, &config);
    let mut mir = lower_program(&typed);
    let stats = supercompile_mir_program(&mut mir);
    eprintln!(
        "[profile-test] stats: nodes={} branches_pruned={} loops_collapsed={} knots_tied={} calls_inlined={} bce={}",
        stats.nodes_explored,
        stats.branches_pruned,
        stats.loops_collapsed,
        stats.knots_tied,
        stats.calls_inlined,
        stats.sc_bce_eliminated
    );
    (mir, stats)
}

fn specialize(path: &Path) -> MirProgram {
    specialize_with_stats(path).0
}

/// Stable FNV-1a fingerprint over the residual MIR. All MirProgram fields are
/// Vec-based, so Debug output has deterministic ordering across processes.
fn residual_fingerprint(mir: &MirProgram) -> u64 {
    let repr = format!("{:?}", mir.functions);
    let mut h = 0xcbf29ce484222325u64;
    for b in repr.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[derive(Clone, Copy)]
struct ExpectedSpecialization {
    label: &'static str,
    path: &'static str,
    nodes: usize,
    branches_pruned: usize,
    loops_collapsed: usize,
    knots_tied: usize,
    calls_inlined: usize,
    sc_bce_eliminated: usize,
    blocks: usize,
    fingerprint: u64,
}

/// Regression guard for the driving-clone optimizations: residuals and all
/// published supercompiler statistics must stay byte-for-byte identical for
/// generic programs (linear lists, nested loops, symbolic expression trees,
/// composition chains, and sliding windows).
///
/// The expectations below were recorded from the unoptimized driver before
/// removing per-node MIR block clones and per-ancestor state clones.
#[test]
fn specialization_is_equivalent_for_generic_programs() {
    let expected: &[ExpectedSpecialization] = &[
        ExpectedSpecialization {
            label: "append3",
            path: "bench/showdown/numlang/append3.nl",
            nodes: 33,
            branches_pruned: 0,
            loops_collapsed: 0,
            knots_tied: 0,
            calls_inlined: 0,
            sc_bce_eliminated: 0,
            blocks: 39,
            fingerprint: 0xcef952778760fc53,
        },
        ExpectedSpecialization {
            label: "expr_eval",
            path: "bench/showdown/extended/numlang/expr_eval.nl",
            nodes: 512,
            branches_pruned: 0,
            loops_collapsed: 0,
            knots_tied: 0,
            calls_inlined: 0,
            sc_bce_eliminated: 0,
            blocks: 19,
            fingerprint: 0xfd46f33764d2b05b,
        },
        ExpectedSpecialization {
            label: "tri_sum",
            path: "bench/showdown/numlang/tri_sum.nl",
            nodes: 10,
            branches_pruned: 3,
            loops_collapsed: 2,
            knots_tied: 0,
            calls_inlined: 0,
            sc_bce_eliminated: 0,
            blocks: 4,
            fingerprint: 0x4265fa024c93880d,
        },
        ExpectedSpecialization {
            label: "compose5",
            path: "bench/showdown/numlang/compose5.nl",
            nodes: 12,
            branches_pruned: 2,
            loops_collapsed: 1,
            knots_tied: 0,
            calls_inlined: 0,
            sc_bce_eliminated: 0,
            blocks: 8,
            fingerprint: 0xf80ca5f2992c91e2,
        },
        ExpectedSpecialization {
            label: "window_stats",
            path: "bench/showdown/extended/numlang/window_stats.nl",
            nodes: 18,
            branches_pruned: 0,
            loops_collapsed: 0,
            knots_tied: 0,
            calls_inlined: 0,
            sc_bce_eliminated: 0,
            blocks: 16,
            fingerprint: 0x1c273861292399df,
        },
    ];

    for expected in expected {
        let ExpectedSpecialization {
            label,
            path,
            nodes,
            branches_pruned,
            loops_collapsed,
            knots_tied,
            calls_inlined,
            sc_bce_eliminated,
            blocks,
            fingerprint,
        } = *expected;
        let (mir, stats) = specialize_with_stats(Path::new(path));
        let actual_blocks = mir.functions.iter().map(|f| f.blocks.len()).sum::<usize>();
        assert_eq!(stats.nodes_explored, nodes, "{label} nodes_explored");
        assert_eq!(stats.branches_pruned, branches_pruned, "{label} branches_pruned");
        assert_eq!(stats.loops_collapsed, loops_collapsed, "{label} loops_collapsed");
        assert_eq!(stats.knots_tied, knots_tied, "{label} knots_tied");
        assert_eq!(stats.calls_inlined, calls_inlined, "{label} calls_inlined");
        assert_eq!(stats.sc_bce_eliminated, sc_bce_eliminated, "{label} sc_bce_eliminated");
        assert_eq!(actual_blocks, blocks, "{label} residual blocks");
        assert_eq!(
            residual_fingerprint(&mir),
            fingerprint,
            "{label} residual MIR fingerprint"
        );
    }
}

#[test]
fn profile_append3_and_expr_eval() {
    for (label, path) in [
        ("append3", "bench/showdown/numlang/append3.nl"),
        (
            "expr_eval",
            "bench/showdown/extended/numlang/expr_eval.nl",
        ),
    ] {
        eprintln!("[profile-test] ==== {label} ====");
        let mir = specialize(Path::new(path));
        eprintln!(
            "[profile-test] {label}: {} functions, {} total blocks",
            mir.functions.len(),
            mir.functions.iter().map(|f| f.blocks.len()).sum::<usize>()
        );
    }
}

#[test]
fn report_array_write_distribution() {
    for (label, path) in [
        ("append3", "bench/showdown/numlang/append3.nl"),
        (
            "expr_eval",
            "bench/showdown/extended/numlang/expr_eval.nl",
        ),
    ] {
        let mut typed = load_typed(Path::new(path));
        let config = CompilerConfig {
            ho_distill: true,
            supercompile: true,
            ..CompilerConfig::default()
        };
        distill_and_optimize(&mut typed, &config);
        let mir = lower_program(&typed);
        for f in &mir.functions {
            let writes = f
                .blocks
                .iter()
                .flat_map(|b| b.statements.iter())
                .filter(|stmt| match stmt {
                    Statement::Assign(dest, _) => dest
                        .projections
                        .iter()
                        .any(|p| matches!(p, Projection::Index(_))),
                })
                .count();
            eprintln!(
                "[array-writes] {label} fn={} blocks={} indexed_writes={writes}",
                f.name,
                f.blocks.len()
            );
        }
    }
}

#[test]
fn specialization_is_deterministic() {
    // The instrumentation must not perturb residuals.  This compares two
    // otherwise identical full-specialization runs of the same source.
    let path = Path::new("bench/showdown/numlang/append3.nl");
    let a = specialize(path);
    let b = specialize(path);
    assert_eq!(
        format!("{:?}", a.functions),
        format!("{:?}", b.functions),
        "specialization must be deterministic"
    );
}
