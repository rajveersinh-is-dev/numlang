//! Tests verifying algorithmic generality of order-2 linear recurrence supercompilation and lowering.
//! Ensures arbitrary coefficients (Fibonacci, Lucas, Pell, Jacobsthal) compile and evaluate cleanly
//! without hardcoded function name or constant matches.

use std::fs;
use std::process::Command;

use numlang::codegen::cranelift::compile_mir_to_obj;
use numlang::codegen::link_executable;
use numlang::mir::lower::{
    MirBasicBlock, MirFunction, MirLocalDecl, MirProgram, Rvalue, Statement,
};
use numlang::mir::supercompiler::generalize::solve_order2_recurrence;
use numlang::mir::supercompiler::term::{SymTerm, TermInterner};
use numlang::mir::{BasicBlockId, Place, Terminator};
use numlang::typecheck::typed_ast::TypedLiteral;
use numlang::typecheck::types::Type;

fn make_place(name: &str) -> Place {
    Place {
        local: name.to_string(),
        projections: Vec::new(),
    }
}

#[test]
fn test_order2_recurrence_solver_symbolic_emission() {
    let mut interner = TermInterner::new();
    let num_iters = interner.intern_var(make_place("n"), Type::I64);

    // 1. Fibonacci: s_{k+1} = 1*s_k + 1*s_{k-1}, s0=0, s1=1
    let fib_samples = vec![0, 1, 1, 2, 3, 5, 8, 13];
    let fib_res = solve_order2_recurrence(&fib_samples, num_iters, &mut interner);
    assert!(fib_res.is_some(), "Fibonacci recurrence must be detected");
    let term = interner.get(fib_res.unwrap()).clone();
    match term {
        SymTerm::Call(callee, args, _) => {
            assert_eq!(callee, "__numlang_linear_rec2");
            assert_eq!(args.len(), 5);
        }
        other => panic!("Expected Call(__numlang_linear_rec2), got {:?}", other),
    }

    // 2. Lucas: s_{k+1} = 1*s_k + 1*s_{k-1}, s0=2, s1=1
    let lucas_samples = vec![2, 1, 3, 4, 7, 11, 18, 29];
    let lucas_res = solve_order2_recurrence(&lucas_samples, num_iters, &mut interner);
    assert!(lucas_res.is_some(), "Lucas recurrence must be detected");
    let term = interner.get(lucas_res.unwrap()).clone();
    match term {
        SymTerm::Call(callee, args, _) => {
            assert_eq!(callee, "__numlang_linear_rec2");
            assert_eq!(args.len(), 5);
        }
        other => panic!("Expected Call(__numlang_linear_rec2), got {:?}", other),
    }

    // 3. Pell: s_{k+1} = 2*s_k + 1*s_{k-1}, s0=0, s1=1
    let pell_samples = vec![0, 1, 2, 5, 12, 29, 70, 169];
    let pell_res = solve_order2_recurrence(&pell_samples, num_iters, &mut interner);
    assert!(pell_res.is_some(), "Pell recurrence must be detected");
    let term = interner.get(pell_res.unwrap()).clone();
    match term {
        SymTerm::Call(callee, args, _) => {
            assert_eq!(callee, "__numlang_linear_rec2");
            assert_eq!(args.len(), 5);
        }
        other => panic!("Expected Call(__numlang_linear_rec2), got {:?}", other),
    }

    // 4. Jacobsthal: s_{k+1} = 1*s_k + 2*s_{k-1}, s0=0, s1=1
    let jacob_samples = vec![0, 1, 1, 3, 5, 11, 21, 43];
    let jacob_res = solve_order2_recurrence(&jacob_samples, num_iters, &mut interner);
    assert!(
        jacob_res.is_some(),
        "Jacobsthal recurrence must be detected"
    );
}

#[test]
fn test_order2_recurrence_constant_evaluation() {
    let mut interner = TermInterner::new();
    let n10 = interner.intern_int(10);

    // Fibonacci F(10) = 55
    let fib_samples = vec![0, 1, 1, 2, 3, 5, 8, 13];
    let fib_res = solve_order2_recurrence(&fib_samples, n10, &mut interner).expect("fib");
    match interner.get(fib_res) {
        SymTerm::ConstInt(val, _) => assert_eq!(*val, 55),
        other => panic!("Expected ConstInt(55), got {:?}", other),
    }

    // Lucas L(10) = 123
    let lucas_samples = vec![2, 1, 3, 4, 7, 11, 18, 29];
    let lucas_res = solve_order2_recurrence(&lucas_samples, n10, &mut interner).expect("lucas");
    match interner.get(lucas_res) {
        SymTerm::ConstInt(val, _) => assert_eq!(*val, 123),
        other => panic!("Expected ConstInt(123), got {:?}", other),
    }
}

#[test]
fn test_order2_recurrence_cranelift_execution() {
    let c1_decl = MirLocalDecl {
        name: "c1".to_string(),
        ty: Type::I64,
        mutable: false,
    };
    let c2_decl = MirLocalDecl {
        name: "c2".to_string(),
        ty: Type::I64,
        mutable: false,
    };
    let s0_decl = MirLocalDecl {
        name: "s0".to_string(),
        ty: Type::I64,
        mutable: false,
    };
    let s1_decl = MirLocalDecl {
        name: "s1".to_string(),
        ty: Type::I64,
        mutable: false,
    };
    let n_decl = MirLocalDecl {
        name: "n".to_string(),
        ty: Type::I64,
        mutable: false,
    };
    let res_decl = MirLocalDecl {
        name: "res".to_string(),
        ty: Type::I64,
        mutable: false,
    };

    let bb0 = MirBasicBlock {
        id: BasicBlockId(0),
        arguments: vec![],
        statements: vec![
            Statement::Assign(
                make_place("c1"),
                Rvalue::Constant(TypedLiteral::Int(1, Type::I64)),
            ),
            Statement::Assign(
                make_place("c2"),
                Rvalue::Constant(TypedLiteral::Int(1, Type::I64)),
            ),
            Statement::Assign(
                make_place("s0"),
                Rvalue::Constant(TypedLiteral::Int(0, Type::I64)),
            ),
            Statement::Assign(
                make_place("s1"),
                Rvalue::Constant(TypedLiteral::Int(1, Type::I64)),
            ),
            Statement::Assign(
                make_place("n"),
                Rvalue::Constant(TypedLiteral::Int(10, Type::I64)),
            ),
            Statement::Assign(
                make_place("res"),
                Rvalue::Call(
                    "__numlang_linear_rec2".to_string(),
                    vec![
                        make_place("c1"),
                        make_place("c2"),
                        make_place("s0"),
                        make_place("s1"),
                        make_place("n"),
                    ],
                ),
            ),
        ],
        terminator: Terminator::Return {
            value: Some(make_place("res")),
        },
    };

    let main_fn = MirFunction {
        name: "main".to_string(),
        params: vec![],
        return_ty: Type::I64,
        locals: vec![c1_decl, c2_decl, s0_decl, s1_decl, n_decl, res_decl],
        blocks: vec![bb0],
        is_distilled: false,
    };

    let program = MirProgram {
        functions: vec![main_fn],
        structs: vec![],
        enums: vec![],
    };

    let obj_bytes = compile_mir_to_obj(&program).expect("compile_mir_to_obj");
    let test_dir = std::env::temp_dir().join("numlang_test_rec2");
    let _ = fs::create_dir_all(&test_dir);
    let obj_path = test_dir.join("rec2.obj");
    let exe_path = test_dir.join("rec2.exe");
    fs::write(&obj_path, obj_bytes).expect("write obj");

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "linking: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to execute compiled test binary");

    assert_eq!(
        output.status.code().unwrap_or(-1),
        55,
        "Fibonacci F(10) computed via Cranelift __numlang_linear_rec2 must return exit code 55"
    );
}
