//! Regression tests for empty branch-only block folding in `compact_mir_function`.

use numlang::codegen::compile_mir_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{MirBasicBlock, MirFunction, MirLocalDecl, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::compact::compact_mir_function;
use numlang::mir::{BasicBlockId, Place, Terminator};
use numlang::typecheck::typed_ast::TypedLiteral;
use numlang::typecheck::types::Type;
use std::fs;
use std::process::Command;

fn id(n: usize) -> BasicBlockId {
    BasicBlockId(n)
}

fn place(name: &str) -> Place {
    Place {
        local: name.to_string(),
        projections: vec![],
    }
}

fn branch(target: usize) -> Terminator {
    Terminator::Branch { target: id(target) }
}

fn ret_const(value: i64) -> Vec<Statement> {
    vec![Statement::Assign(
        place("ret"),
        Rvalue::Constant(TypedLiteral::Int(value, Type::I64)),
    )]
}

fn ret() -> Terminator {
    Terminator::Return {
        value: Some(place("ret")),
    }
}

fn block(n: usize, statements: Vec<Statement>, terminator: Terminator) -> MirBasicBlock {
    MirBasicBlock {
        id: id(n),
        arguments: vec![],
        statements,
        terminator,
    }
}

fn function(blocks: Vec<MirBasicBlock>) -> MirFunction {
    MirFunction {
        name: "main".to_string(),
        params: vec![],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl {
                name: "ret".to_string(),
                ty: Type::I64,
                mutable: true,
            },
            MirLocalDecl {
                name: "c".to_string(),
                ty: Type::Bool,
                mutable: true,
            },
            MirLocalDecl {
                name: "x".to_string(),
                ty: Type::I64,
                mutable: true,
            },
        ],
        blocks,
        is_distilled: false,
    }
}

fn block_ids(func: &MirFunction) -> Vec<usize> {
    func.blocks.iter().map(|b| b.id.0).collect()
}

#[test]
fn folds_empty_chain_and_redirects_branch_and_switch() {
    // entry -> bb1 -> bb2(Switch) ; case targets bb3->bb5, bb4, default bb5.
    // bb1 and bb3 are pure forwarders and must disappear; surviving
    // predecessors (including the Switch) must point at their final targets.
    let mut func = function(vec![
        block(0, vec![], branch(1)),
        block(1, vec![], branch(2)),
        block(
            2,
            vec![],
            Terminator::Switch {
                value: place("x"),
                targets: vec![(1, id(3)), (2, id(4))],
                default: id(5),
            },
        ),
        block(3, vec![], branch(5)),
        block(4, ret_const(20), ret()),
        block(5, ret_const(30), ret()),
    ]);

    let removed = compact_mir_function(&mut func);
    assert!(removed >= 2, "expected >= 2 blocks removed, got {removed}");
    assert_eq!(block_ids(&func), vec![0, 2, 4, 5]);
    assert_eq!(func.blocks[0].terminator, branch(2));

    match &func.blocks[1].terminator {
        Terminator::Switch {
            targets, default, ..
        } => {
            assert_eq!(*targets, vec![(1, id(5)), (2, id(4))]);
            assert_eq!(*default, id(5));
        }
        other => panic!("expected Switch, got {other:?}"),
    }

    // Idempotence: nothing left to fold on a second invocation.
    assert_eq!(compact_mir_function(&mut func), 0);
}

#[test]
fn redirects_every_terminator_kind_that_carries_block_ids() {
    let mut func = function(vec![
        block(
            0,
            vec![],
            Terminator::BranchIf {
                condition: place("c"),
                then_target: id(8),
                else_target: id(7),
            },
        ),
        block(
            2,
            vec![],
            Terminator::Switch {
                value: place("x"),
                targets: vec![(1, id(8))],
                default: id(7),
            },
        ),
        block(
            3,
            vec![],
            Terminator::IndirectCall {
                callee: place("x"),
                args: vec![],
                dest: place("ret"),
                next: id(8),
            },
        ),
        block(
            4,
            vec![],
            Terminator::Fork {
                left: id(8),
                right: id(8),
                join: id(8),
            },
        ),
        block(
            5,
            vec![],
            Terminator::Force {
                thunk: "x".to_string(),
                result: "ret".to_string(),
                cont: id(8),
            },
        ),
        block(
            6,
            vec![],
            Terminator::TypeGuard {
                local: place("x"),
                expected_tag: 1,
                fast_path: id(8),
                deopt_stub: id(8),
            },
        ),
        block(7, vec![], Terminator::Return { value: None }),
        block(8, vec![], branch(9)),
        block(9, ret_const(7), ret()),
    ]);

    assert!(compact_mir_function(&mut func) >= 1);
    assert_eq!(block_ids(&func), vec![0, 2, 3, 4, 5, 6, 7, 9]);

    assert_eq!(
        func.blocks[0].terminator,
        Terminator::BranchIf {
            condition: place("c"),
            then_target: id(9),
            else_target: id(7),
        }
    );
    match &func.blocks[1].terminator {
        Terminator::Switch {
            targets, default, ..
        } => {
            assert_eq!(*targets, vec![(1, id(9))]);
            assert_eq!(*default, id(7));
        }
        other => panic!("expected Switch, got {other:?}"),
    }
    match &func.blocks[2].terminator {
        Terminator::IndirectCall { next, .. } => assert_eq!(*next, id(9)),
        other => panic!("expected IndirectCall, got {other:?}"),
    }
    assert_eq!(
        func.blocks[3].terminator,
        Terminator::Fork {
            left: id(9),
            right: id(9),
            join: id(9),
        }
    );
    match &func.blocks[4].terminator {
        Terminator::Force { cont, .. } => assert_eq!(*cont, id(9)),
        other => panic!("expected Force, got {other:?}"),
    }
    assert_eq!(
        func.blocks[5].terminator,
        Terminator::TypeGuard {
            local: place("x"),
            expected_tag: 1,
            fast_path: id(9),
            deopt_stub: id(9),
        }
    );
}

#[test]
fn folds_backedge_forwarder_but_keeps_self_loops_and_empty_cycles() {
    let mut func = function(vec![
        block(0, vec![], branch(1)),
        block(
            1,
            vec![],
            Terminator::BranchIf {
                condition: place("c"),
                then_target: id(2),
                else_target: id(3),
            },
        ),
        // Pure forwarder on the loop back-edge: folds into a self back-edge.
        block(2, vec![], branch(1)),
        // Self-loop and a two-block empty cycle: both must survive untouched.
        block(3, vec![], branch(3)),
        block(4, vec![], branch(5)),
        block(5, vec![], branch(4)),
        block(6, ret_const(11), ret()),
    ]);

    assert!(compact_mir_function(&mut func) >= 1);
    assert_eq!(block_ids(&func), vec![0, 1, 3, 4, 5, 6]);
    assert_eq!(
        func.blocks[1].terminator,
        Terminator::BranchIf {
            condition: place("c"),
            then_target: id(1),
            else_target: id(3),
        }
    );
    assert_eq!(func.blocks[2].terminator, branch(3));
    assert_eq!(func.blocks[3].terminator, branch(5));
    assert_eq!(func.blocks[4].terminator, branch(4));
    assert_eq!(compact_mir_function(&mut func), 0);
}

#[test]
fn keeps_blocks_used_as_phi_labels() {
    let mut func = function(vec![
        block(0, vec![], branch(1)),
        block(1, vec![], branch(2)),
        block(2, vec![], branch(3)),
        block(
            3,
            vec![Statement::Assign(
                place("ret"),
                Rvalue::Phi(vec![(id(1), place("ret"))]),
            )],
            ret(),
        ),
    ]);

    // bb1 is a Phi label, so it must not be folded; bb2 is an ordinary
    // forwarder and is folded into bb3.
    assert!(compact_mir_function(&mut func) >= 1);
    assert_eq!(block_ids(&func), vec![0, 1, 3]);
    assert_eq!(func.blocks[1].terminator, branch(3));
    match &func.blocks[2].statements[0] {
        Statement::Assign(_, Rvalue::Phi(incoming)) => {
            assert_eq!(*incoming, vec![(id(1), place("ret"))]);
        }
        other => panic!("expected Phi, got {other:?}"),
    }
}

fn compile_and_run(func: MirFunction, tag: &str) -> i32 {
    let program = MirProgram {
        functions: vec![func],
        structs: vec![],
        enums: vec![],
    };
    let obj_bytes = compile_mir_to_obj(&program).expect("MIR codegen failed");
    let dir = std::env::temp_dir().join(format!(
        "numlang_empty_fold_{}_{}_{}",
        std::process::id(),
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let obj_path = dir.join("program.obj");
    let exe_path = dir.join("program.exe");
    fs::write(&obj_path, obj_bytes).unwrap();
    link_executable(&obj_path, &exe_path).expect("link failed");
    let output = Command::new(&exe_path).output().expect("run failed");
    let code = output.status.code().unwrap_or(-1);
    let _ = fs::remove_dir_all(&dir);
    code
}

#[test]
fn compiled_execution_is_preserved_by_folding() {
    let func = function(vec![
        block(0, vec![], branch(1)),
        block(1, vec![], branch(2)),
        block(2, vec![], branch(3)),
        block(3, ret_const(42), ret()),
    ]);

    let before = compile_and_run(func.clone(), "before");
    let mut folded = func;
    assert!(compact_mir_function(&mut folded) >= 2);
    assert_eq!(block_ids(&folded), vec![0, 3]);
    let after = compile_and_run(folded, "after");

    assert_eq!(before, 42, "uncompacted MIR returned {before}");
    assert_eq!(after, 42, "folded MIR returned {after}");
}
