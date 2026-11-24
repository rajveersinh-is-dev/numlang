//! Structural branch simplification in `generalize.rs` (if-conversion +
//! straight-line block coalescing).
//!
//! The tests drive the pass directly through the library API, independently of
//! any CLI hook.  They verify that closed programs keep their runtime answer
//! and their arithmetic, that symbolic-bound loops are never touched, and they
//! report interleaved A/B COMPUTE_NS medians for the two extended workloads.

use numlang::ast::BinaryOp;
use numlang::codegen::{compile_mir_to_obj, linker::link_executable};
use numlang::compiler::{distill_and_optimize, CompilerConfig};
use numlang::mir::lower::{lower_program, Rvalue, Statement};
use numlang::mir::supercompiler::generalize::simplify_bounded_control_mir_program;
use numlang::mir::supercompiler::{supercompile_mir_program_with_mode, SupercompileMode};
use numlang::mir::{Projection, Terminator};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const WINDOW_STATS: &str = "bench/showdown/extended/numlang/window_stats.nl";
const RECORD_PIPELINE: &str = "bench/showdown/extended/numlang/record_pipeline.nl";

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("read source")
}

/// Reproduce the exact `--no-cache --supercompile --mode distill` pipeline used
/// by the benchmark harness, optionally applying the structural pass just
/// before native code generation.
fn build(src_text: &str, simplify: bool, tag: &str) -> (numlang::mir::lower::MirProgram, PathBuf) {
    numlang::runtime_input::mark_compiler_driver();
    // The linker selects its timing entry point from this variable, matching
    // what the CLI does for `--bench`.
    std::env::set_var("NUMLANG_BENCH", "1");
    let tokens = tokenize(src_text).expect("tokenize");
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
    mir.enforce_runtime_computation = true;
    supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    if simplify {
        simplify_bounded_control_mir_program(&mut mir);
    }
    let obj = compile_mir_to_obj(&mir).expect("compile MIR");
    let stem = format!(
        "nl_w6c_{}_{}_{}",
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
    fs::write(&obj_path, obj).expect("write object");
    link_executable(&obj_path, &exe_path).expect("link");
    let _ = fs::remove_file(&obj_path);
    (mir, exe_path)
}

struct Run {
    printed: String,
    compute_ns: u128,
    exit: i32,
}

fn run(exe: &Path) -> Run {
    let out = Command::new(exe).output().expect("run benchmark binary");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let printed = stdout.lines().next().unwrap_or_default().trim().to_string();
    let compute_ns = stdout
        .lines()
        .find_map(|line| line.strip_prefix("COMPUTE_NS: "))
        .and_then(|s| s.trim().parse::<u128>().ok())
        .expect("COMPUTE_NS");
    Run {
        printed,
        compute_ns,
        exit: out.status.code().unwrap_or(-1),
    }
}

fn median(samples: &[u128]) -> u128 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

fn profile(mir: &numlang::mir::lower::MirProgram) -> (usize, usize, Vec<i64>) {
    let (mut arithmetic, mut branches) = (0usize, 0usize);
    let mut constants = Vec::new();
    for func in &mir.functions {
        for block in &func.blocks {
            match &block.terminator {
                Terminator::BranchIf { .. }
                | Terminator::Switch { .. }
                | Terminator::Fork { .. }
                | Terminator::TypeGuard { .. } => branches += 1,
                _ => {}
            }
            for succ in block.terminator.successors() {
                if succ.0 <= block.id.0 {
                    branches += 1;
                }
            }
            for Statement::Assign(place, rv) in &block.statements {
                match rv {
                    Rvalue::BinaryOp(op, _, _)
                        if !matches!(
                            op,
                            BinaryOp::Eq
                                | BinaryOp::Ne
                                | BinaryOp::Lt
                                | BinaryOp::Le
                                | BinaryOp::Gt
                                | BinaryOp::Ge
                        ) =>
                    {
                        arithmetic += 1;
                    }
                    Rvalue::UnaryOp(_, _) | Rvalue::Select(_, _, _) | Rvalue::Phi(_) => {
                        arithmetic += 1;
                    }
                    Rvalue::Constant(numlang::typecheck::typed_ast::TypedLiteral::Int(v, _))
                        if !place.projections.iter().any(|p| matches!(p, Projection::Index(_))) =>
                    {
                        constants.push(*v);
                    }
                    _ => {}
                }
            }
        }
    }
    (arithmetic, branches, constants)
}

#[test]
fn simplification_preserves_answer_and_retains_runtime_computation() {
    let src = source(WINDOW_STATS);
    let (_, base_exe) = build(&src, false, "base");
    let (simplified_mir, simplified_exe) = build(&src, true, "simplified");

    let base = run(&base_exe);
    let simplified = run(&simplified_exe);
    assert_eq!(base.printed, "248", "baseline stdout");
    assert_eq!(simplified.printed, "248", "simplified stdout");
    assert_eq!(base.exit, 248, "baseline exit code");
    assert_eq!(simplified.exit, 248, "simplified exit code");

    let (arith, branches, constants) = profile(&simplified_mir);
    assert!(arith > 0, "residual MIR must still contain arithmetic");
    assert!(branches > 0, "runtime loop control must remain");
    assert!(
        !constants.contains(&248),
        "residual MIR must not embed the final answer 248"
    );

    let _ = fs::remove_file(&base_exe);
    let _ = fs::remove_file(&simplified_exe);
}

#[test]
fn simplification_does_not_touch_symbolic_or_effectful_control() {
    // A loop whose bound comes from read_i64() must not be unrolled, and its
    // two-sided conditional must not be if-converted (both arms mutate `acc`).
    let src = r#"
        fn main() -> i64 {
            let n: i64 = read_i64();
            let mut acc: i64 = 0;
            let mut i: i64 = 0;
            while i < n {
                if (i * i) % 3 == 0 { acc = acc + i; } else { acc = acc + 1; }
                i = i + 1;
            }
            println(acc);
            return acc % 256;
        }
    "#;
    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();
    let mut mir = lower_program(&typed);
    let before = mir.clone();
    let changed = simplify_bounded_control_mir_program(&mut mir);
    assert_eq!(changed, 0, "two-arm diamond must be left alone");
    assert_eq!(mir.functions, before.functions);
}

#[test]
#[ignore = "timing measurement: run with --ignored --nocapture for A/B medians"]
fn interleaved_ab_timing() {
    let cases = [
        ("window_stats", WINDOW_STATS),
        ("record_pipeline", RECORD_PIPELINE),
    ];
    for (label, path) in cases {
        let src = source(path);
        let (_, base_exe) = build(&src, false, &format!("ab_base_{}", label));
        let (_, fast_exe) = build(&src, true, &format!("ab_fast_{}", label));
        let mut base = Vec::new();
        let mut fast = Vec::new();
        for _ in 0..9 {
            base.push(run(&base_exe).compute_ns);
            fast.push(run(&fast_exe).compute_ns);
        }
        println!(
            "{}: base={:?} median={} | simplified={:?} median={}",
            label,
            base,
            median(&base),
            fast,
            median(&fast)
        );
        let _ = fs::remove_file(&base_exe);
        let _ = fs::remove_file(&fast_exe);
    }
}
