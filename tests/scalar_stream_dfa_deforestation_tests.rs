//! Structural tests for scalar stream-DFA deforestation.
//!
//! A recursive producer builds a finite algebraic stream and a family of
//! mutually tail-recursive consumers walks it.  The pass must fuse them into a
//! single scalar loop that still executes the transition semantics at run time.
//! The checks below are name-agnostic: two alpha-equivalent sources (different
//! enum, function and local names) must produce the same structural fingerprint
//! and the same independently computed answer.  No benchmark name, literal
//! answer or precomputed table is involved.

use numlang::codegen::compile_mir_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::compiler::{distill_and_optimize, CompilerConfig};
use numlang::mir::lower::{lower_program, Rvalue, Statement};
use numlang::mir::supercompiler::{supercompile_mir_program_with_mode, SupercompileMode};
use numlang::mir::{BasicBlockId, Terminator};
use numlang::parser::parse;
use numlang::runtime_input::set_ast_runtime_computation_guard;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use std::collections::HashSet;
use std::fs;
use std::process::Command;

const CHAIN_DFA: &str = r#"
    enum Chain { End, Link(i64, Box<Chain>) }
    fn build_chain(k: i64) -> Chain {
        if k <= 0 { return End; }
        let bit: i64 = (k * 73 + 19) % 2;
        return Link(bit, box(build_chain(k - 1)));
    }
    fn accept_even(s: Chain) -> i64 {
        return match s {
            End => 0,
            Link(v, tl) => match v {
                1 => 1 + accept_odd(deref(tl)),
                _ => accept_even(deref(tl)),
            },
        };
    }
    fn accept_odd(s: Chain) -> i64 {
        return match s {
            End => 0,
            Link(v, tl) => match v {
                0 => accept_even(deref(tl)),
                _ => accept_odd(deref(tl)),
            },
        };
    }
    fn main() -> i64 {
        let mut total: i64 = 0;
        for i in 0..100 {
            let stream: Chain = build_chain(15);
            total = total + accept_even(stream);
        }
        println(total);
        return total % 256;
    }
"#;

const CHAIN_DFA_RENAMED: &str = r#"
    enum Sequence { Stop, Node(i64, Box<Sequence>) }
    fn generate(count: i64) -> Sequence {
        if count <= 0 { return Stop; }
        let symbol: i64 = (count * 73 + 19) % 2;
        return Node(symbol, box(generate(count - 1)));
    }
    fn first_mode(xs: Sequence) -> i64 {
        return match xs {
            Stop => 0,
            Node(head, tail) => match head {
                1 => 1 + second_mode(deref(tail)),
                _ => first_mode(deref(tail)),
            },
        };
    }
    fn second_mode(xs: Sequence) -> i64 {
        return match xs {
            Stop => 0,
            Node(head, tail) => match head {
                0 => first_mode(deref(tail)),
                _ => second_mode(deref(tail)),
            },
        };
    }
    fn main() -> i64 {
        let mut acc: i64 = 0;
        for j in 0..100 {
            let seq: Sequence = generate(15);
            acc = acc + first_mode(seq);
        }
        println(acc);
        return acc % 256;
    }
"#;

/// Independent simulation of the DFA over the generated bit sequence.
fn reference_answer(steps: i64, repeats: i64) -> i64 {
    let mut total = 0i64;
    for _ in 0..repeats {
        let mut state = 0i64;
        let mut hits = 0i64;
        for n in (1..=steps).rev() {
            let bit = (n * 73 + 19) % 2;
            if state == 0 {
                if bit == 1 {
                    hits += 1;
                    state = 1;
                }
            } else if bit == 0 {
                state = 0;
            }
        }
        total += hits;
    }
    total
}

fn optimized_mir(src: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(src).expect("tokenize");
    let ast = parse(&tokens).expect("parse");
    let mut typed = typecheck(&ast).expect("typecheck");
    let previous = set_ast_runtime_computation_guard(true);
    distill_and_optimize(
        &mut typed,
        &CompilerConfig {
            ho_distill: true,
            supercompile: true,
            ..Default::default()
        },
    );
    set_ast_runtime_computation_guard(previous);
    let mut mir = lower_program(&typed);
    mir.enforce_runtime_computation = true;
    supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    mir
}

#[derive(Debug, Default, PartialEq, Eq)]
struct FusedProfile {
    loop_back_edges: usize,
    switches: usize,
    branch_ifs: usize,
    arithmetic: usize,
    heap_ops: usize,
    arrays: usize,
}

fn fused_profile(src: &str) -> FusedProfile {
    let mir = optimized_mir(src);
    let fused: Vec<_> = mir
        .functions
        .iter()
        .filter(|f| f.name.starts_with("__sdf_fused_"))
        .collect();
    assert_eq!(fused.len(), 1, "expected exactly one fused loop function");
    let func = fused[0];
    let mut p = FusedProfile::default();
    let ids: HashSet<BasicBlockId> = func.blocks.iter().map(|b| b.id.clone()).collect();
    for block in &func.blocks {
        match &block.terminator {
            Terminator::Switch { .. } => p.switches += 1,
            Terminator::BranchIf { .. } => p.branch_ifs += 1,
            _ => {}
        }
        for succ in block.terminator.successors() {
            if ids.contains(&succ) && succ.0 <= block.id.0 {
                p.loop_back_edges += 1;
            }
        }
        for Statement::Assign(_, rv) in &block.statements {
            match rv {
                Rvalue::BinaryOp(..) | Rvalue::UnaryOp(..) => p.arithmetic += 1,
                Rvalue::Alloc(_) | Rvalue::Load(_) => p.heap_ops += 1,
                Rvalue::Array(_) => p.arrays += 1,
                _ => {}
            }
        }
    }
    p
}

fn compile_and_run(src: &str) -> (i32, String) {
    let mir = optimized_mir(src);
    let obj = compile_mir_to_obj(&mir).expect("codegen");
    let stem = format!(
        "nl_sdf_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let obj_path = std::env::temp_dir().join(format!("{}.obj", stem));
    let exe_path = std::env::temp_dir().join(format!("{}.exe", stem));
    fs::write(&obj_path, obj).expect("write object");
    assert!(link_executable(&obj_path, &exe_path).is_ok());
    let out = Command::new(&exe_path).output().expect("run binary");
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    )
}

#[test]
fn stream_dfa_fuses_into_a_runnable_loop() {
    let p = fused_profile(CHAIN_DFA);
    assert!(p.loop_back_edges > 0, "fused DFA must retain a loop: {:?}", p);
    assert!(p.switches > 0, "fused DFA must retain state switches: {:?}", p);
    assert!(p.branch_ifs > 0, "fused DFA must retain a branch: {:?}", p);
    assert!(p.arithmetic > 0, "fused DFA must retain arithmetic: {:?}", p);
    assert_eq!(p.heap_ops, 0, "fused DFA must not allocate or load boxes");
    assert_eq!(p.arrays, 0, "fused DFA must not materialise a table");
}

#[test]
fn stream_dfa_returns_the_independently_computed_answer() {
    let expected = reference_answer(15, 100);
    for src in [CHAIN_DFA, CHAIN_DFA_RENAMED] {
        let (code, stdout) = compile_and_run(src);
        assert_eq!(stdout, expected.to_string(), "wrong stdout");
        assert_eq!(
            code,
            expected.rem_euclid(256) as i32,
            "wrong exit code"
        );
    }
}

#[test]
fn stream_dfa_fusion_is_alpha_rename_invariant() {
    assert_eq!(
        fused_profile(CHAIN_DFA),
        fused_profile(CHAIN_DFA_RENAMED),
        "fusion must not depend on function/local/enum names"
    );
}

#[test]
fn stream_dfa_does_not_emit_the_answer_as_a_literal() {
    let expected = reference_answer(15, 100);
    let mir = optimized_mir(CHAIN_DFA);
    for func in &mir.functions {
        for block in &func.blocks {
            for Statement::Assign(_, rv) in &block.statements {
                if let Rvalue::Constant(numlang::typecheck::typed_ast::TypedLiteral::Int(v, _)) = rv {
                    assert_ne!(*v, expected, "the answer must be computed, not emitted");
                }
            }
        }
    }
}
