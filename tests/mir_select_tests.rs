//! Regression tests for branchless MIR Select lowering.

use numlang::ast::BinaryOp;
use numlang::mir::lower::{MirBasicBlock, MirFunction, MirLocalDecl, Rvalue, Statement};
use numlang::mir::supercompiler::strength_reduce::strength_reduce_mir_function;
use numlang::mir::{BasicBlockId, Place, Terminator};
use numlang::typecheck::typed_ast::TypedLiteral;
use numlang::typecheck::types::Type;

fn place(name: &str) -> Place {
    Place { local: name.to_string(), projections: vec![] }
}

fn clamp_mir() -> MirFunction {
    let zero = place("zero");
    let cond = place("cond");
    let ret = place("ret");
    let x = place("x");
    MirFunction {
        name: "clamp".to_string(),
        params: vec![("x".to_string(), Type::I64)],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl { name: "x".into(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "zero".into(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "cond".into(), ty: Type::Bool, mutable: true },
            MirLocalDecl { name: "ret".into(), ty: Type::I64, mutable: true },
        ],
        blocks: vec![
            MirBasicBlock {
                id: BasicBlockId(0),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(zero.clone(), Rvalue::Constant(TypedLiteral::Int(0, Type::I64))),
                    Statement::Assign(cond.clone(), Rvalue::BinaryOp(BinaryOp::Lt, x.clone(), zero)),
                ],
                terminator: Terminator::BranchIf {
                    condition: cond,
                    then_target: BasicBlockId(1),
                    else_target: BasicBlockId(2),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(1),
                arguments: vec![],
                statements: vec![Statement::Assign(ret.clone(), Rvalue::Constant(TypedLiteral::Int(0, Type::I64)))],
                terminator: Terminator::Branch { target: BasicBlockId(3) },
            },
            MirBasicBlock {
                id: BasicBlockId(2),
                arguments: vec![],
                statements: vec![Statement::Assign(ret.clone(), Rvalue::Use(x))],
                terminator: Terminator::Branch { target: BasicBlockId(3) },
            },
            MirBasicBlock {
                id: BasicBlockId(3),
                arguments: vec![],
                statements: vec![],
                terminator: Terminator::Return { value: Some(ret) },
            },
        ],
        is_distilled: false,
    }
}

#[test]
fn diamond_becomes_select() {
    let mut f = clamp_mir();
    let reduced = strength_reduce_mir_function(&mut f);
    assert!(reduced > 0, "strength reduction must report the diamond rewrite");
    let has_select = f.blocks.iter().any(|b| {
        b.statements.iter().any(|s| matches!(s, Statement::Assign(_, Rvalue::Select(_, _, _))))
    });
    assert!(has_select, "diamond must be rewritten to Rvalue::Select");
    let has_branchif = f.blocks.iter().any(|b| matches!(b.terminator, Terminator::BranchIf { .. }));
    assert!(!has_branchif, "the diamond BranchIf must be removed");
}
