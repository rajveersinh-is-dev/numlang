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
use numlang::mir::supercompiler::supercompile_mir_program;
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

fn specialize(path: &Path) -> MirProgram {
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
    mir
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
