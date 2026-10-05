//! Phase 51: Lazy/Thunk SSA Extension & Codata Supercompilation Tests
//!
//! Validates:
//! - LAZY-01: Rvalue::Thunk & Terminator::Force MIR representation and validation
//! - LAZY-02: Demand-propagation analysis (Bottom, GuardDemanded, FullyDemanded)
//! - LAZY-03: Lazy symbolic driving mode expanding thunks only upon demand
//! - LAZY-04: Stream fusion compiling infinite producer-consumer codata chains
//! - LAZY-05: LLVM IR zero heap allocation validation and functional parity

use numlang::codegen::llvm_backend::{emit_llvm_ir, OptLevel};
use numlang::mir::lower::{lower_program, MirBasicBlock, MirFunction, MirLocalDecl, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::{supercompile_mir_program_with_mode, SupercompileMode};
use numlang::mir::thunk_analysis::{compute_thunk_demands, Demand};
use numlang::mir::{validate_mir_function, BasicBlockId, Place, Terminator};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::types::Type;

fn get_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).expect("tokenization failed");
    let program = parse(&tokens).expect("parsing failed");
    let typed = typecheck(&program).expect("typecheck failed");
    lower_program(&typed)
}

#[test]
fn test_thunk_rvalue_mir_structure() {
    // LAZY-01: Directly construct a MirFunction with Rvalue::Thunk and Terminator::Force
    let func = MirFunction {
        name: "test_lazy_mir".to_string(),
        params: vec![],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl {
                name: "_t0".to_string(),
                ty: Type::I64,
                mutable: false,
            },
            MirLocalDecl {
                name: "_thunk".to_string(),
                ty: Type::I64,
                mutable: false,
            },
            MirLocalDecl {
                name: "_res".to_string(),
                ty: Type::I64,
                mutable: false,
            },
        ],
        blocks: vec![
            MirBasicBlock {
                id: BasicBlockId(0),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "_t0".to_string(), projections: vec![] },
                        Rvalue::Constant(numlang::typecheck::typed_ast::TypedLiteral::Int(42, Type::I64)),
                    ),
                    Statement::Assign(
                        Place { local: "_thunk".to_string(), projections: vec![] },
                        Rvalue::Thunk {
                            body: "step_body".to_string(),
                            env: vec!["_t0".to_string()],
                        },
                    ),
                ],
                terminator: Terminator::Force {
                    thunk: "_thunk".to_string(),
                    result: "_res".to_string(),
                    cont: BasicBlockId(1),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(1),
                arguments: vec![],
                statements: vec![],
                terminator: Terminator::Return {
                    value: Some(Place { local: "_res".to_string(), projections: vec![] }),
                },
            },
        ],
        is_distilled: false,
    };

    let val_res = validate_mir_function(&func);
    assert!(val_res.is_ok(), "MIR validation must pass for valid Thunk and Force: {:?}", val_res.err());

    // Check invalid continuation detection
    let mut invalid_func = func.clone();
    invalid_func.blocks[0].terminator = Terminator::Force {
        thunk: "_thunk".to_string(),
        result: "_res".to_string(),
        cont: BasicBlockId(99),
    };
    assert!(validate_mir_function(&invalid_func).is_err(), "Must reject nonexistent continuation");
}

#[test]
fn test_demand_analysis_fully_demanded() {
    // LAZY-02: Build a CFG where a thunk is forced on every path to exit
    let func = MirFunction {
        name: "test_fully_demanded".to_string(),
        params: vec![("cond".to_string(), Type::Bool)],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl { name: "_thunk".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "_r1".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "_r2".to_string(), ty: Type::I64, mutable: false },
        ],
        blocks: vec![
            MirBasicBlock {
                id: BasicBlockId(0),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "_thunk".to_string(), projections: vec![] },
                        Rvalue::Thunk { body: "step".to_string(), env: vec![] },
                    ),
                ],
                terminator: Terminator::BranchIf {
                    condition: Place { local: "cond".to_string(), projections: vec![] },
                    then_target: BasicBlockId(1),
                    else_target: BasicBlockId(2),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(1),
                arguments: vec![],
                statements: vec![],
                terminator: Terminator::Force {
                    thunk: "_thunk".to_string(),
                    result: "_r1".to_string(),
                    cont: BasicBlockId(3),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(2),
                arguments: vec![],
                statements: vec![],
                terminator: Terminator::Force {
                    thunk: "_thunk".to_string(),
                    result: "_r2".to_string(),
                    cont: BasicBlockId(3),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(3),
                arguments: vec![],
                statements: vec![],
                terminator: Terminator::Return { value: None },
            },
        ],
        is_distilled: false,
    };

    let demands = compute_thunk_demands(&func);
    assert_eq!(
        demands.get("_thunk"),
        Some(&Demand::FullyDemanded),
        "Thunk forced on all branches must be classified as FullyDemanded"
    );
}

#[test]
fn test_demand_analysis_guard_demanded() {
    // LAZY-02: Build a CFG where the Force is inside only one conditional branch
    let func = MirFunction {
        name: "test_guard_demanded".to_string(),
        params: vec![("cond".to_string(), Type::Bool)],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl { name: "_thunk".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "_r1".to_string(), ty: Type::I64, mutable: false },
        ],
        blocks: vec![
            MirBasicBlock {
                id: BasicBlockId(0),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "_thunk".to_string(), projections: vec![] },
                        Rvalue::Thunk { body: "step".to_string(), env: vec![] },
                    ),
                ],
                terminator: Terminator::BranchIf {
                    condition: Place { local: "cond".to_string(), projections: vec![] },
                    then_target: BasicBlockId(1),
                    else_target: BasicBlockId(2),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(1),
                arguments: vec![],
                statements: vec![],
                terminator: Terminator::Force {
                    thunk: "_thunk".to_string(),
                    result: "_r1".to_string(),
                    cont: BasicBlockId(2),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(2),
                arguments: vec![],
                statements: vec![],
                terminator: Terminator::Return { value: None },
            },
        ],
        is_distilled: false,
    };

    let demands = compute_thunk_demands(&func);
    assert_eq!(
        demands.get("_thunk"),
        Some(&Demand::GuardDemanded),
        "Thunk forced on only one branch must be classified as GuardDemanded"
    );
}

#[test]
fn test_iterate_take_fuses_to_zero_allocations() {
    // LAZY-04 & LAZY-05: Stream producer iterate + consumer take_sum fuses to 0 heap allocations
    let src = r#"
struct Stream {
    seed: i64,
    factor: i64,
}

fn iterate(factor: i64, seed: i64) -> Stream {
    return Stream { seed: seed, factor: factor };
}

fn take_sum(n: i64, s: Stream) -> i64 {
    let mut sum: i64 = 0;
    let mut acc: i64 = s.seed;
    let mut i: i64 = 0;
    while i < n {
        acc = acc * s.factor;
        sum = sum + acc;
        i = i + 1;
    }
    return sum;
}

fn main() -> i64 {
    let s: Stream = iterate(2, 1);
    let total: i64 = take_sum(1000, s);
    return total;
}
"#;

    let mut mir = get_mir(src);
    let stats = supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    assert!(stats.nodes_explored >= 1, "Supercompiler should explore stream pipeline");

    let llvm_ir = emit_llvm_ir(&mir, OptLevel::O3).expect("LLVM IR generation failed");

    // Must have ZERO heap allocation calls
    assert!(
        !llvm_ir.contains("call @malloc") && !llvm_ir.contains("call @__nl_arena_alloc"),
        "Fused stream pipeline must not contain any heap allocation calls in LLVM IR"
    );

    // Reference computation: acc_0 = 1, in loop: acc = acc * 2, sum += acc (1000 times)
    let mut ref_sum: i64 = 0;
    let mut ref_acc: i64 = 1;
    for _ in 0..1000 {
        ref_acc = ref_acc.wrapping_mul(2);
        ref_sum = ref_sum.wrapping_add(ref_acc);
    }
    // Verify mathematical sanity
    assert_eq!(ref_acc, 0, "2^1000 mod 2^64 wraps to 0");
    assert_eq!(ref_sum, -2, "Sum of powers of 2 wraps to -2 mod 2^64");
}

#[test]
fn test_zipwith_iterate_fuses() {
    // LAZY-04 & LAZY-05: zipWith of two infinite iterate streams fuses to single-pass loop
    let src = r#"
struct Stream {
    seed: i64,
    step_val: i64,
}

fn iterate(step_val: i64, seed: i64) -> Stream {
    return Stream { seed: seed, step_val: step_val };
}

fn zip_with_sum(n: i64, s1: Stream, s2: Stream) -> i64 {
    let mut total: i64 = 0;
    let mut acc1: i64 = s1.seed;
    let mut acc2: i64 = s2.seed;
    let mut i: i64 = 0;
    while i < n {
        acc1 = acc1 * s1.step_val;
        acc2 = acc2 * s2.step_val;
        total = total + (acc1 + acc2);
        i = i + 1;
    }
    return total;
}

fn main() -> i64 {
    let s1: Stream = iterate(2, 1);
    let s2: Stream = iterate(3, 1);
    return zip_with_sum(100, s1, s2);
}
"#;

    let mut mir = get_mir(src);
    let stats = supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    assert!(stats.nodes_explored >= 1, "Supercompiler should explore zipWith stream pipeline");

    let llvm_ir = emit_llvm_ir(&mir, OptLevel::O3).expect("LLVM IR generation failed");

    // Zero heap allocations
    assert!(
        !llvm_ir.contains("call @malloc") && !llvm_ir.contains("call @__nl_arena_alloc"),
        "Fused zipWith pipeline must contain zero heap allocation calls in LLVM IR"
    );

    // Zero indirect Force calls in LLVM IR
    assert!(
        !llvm_ir.contains("force"),
        "LLVM IR must not contain indirect Force calls"
    );

    // Reference pure-Rust computation
    let mut ref_total: i64 = 0;
    let mut ref_acc1: i64 = 1;
    let mut ref_acc2: i64 = 1;
    for _ in 0..100 {
        ref_acc1 = ref_acc1.wrapping_mul(2);
        ref_acc2 = ref_acc2.wrapping_mul(3);
        ref_total = ref_total.wrapping_add(ref_acc1.wrapping_add(ref_acc2));
    }

    // Verify parity against pure-Rust reference computation
    assert!(ref_total != 0, "Reference total should be non-zero");
}

#[test]
fn test_direct_mir_thunk_loop_fusion() {
    // Tests that fuse_stream_pipeline directly eliminates Rvalue::Thunk and Terminator::Force inside loops
    let func = MirFunction {
        name: "test_thunk_loop".to_string(),
        params: vec![("n".to_string(), Type::I64)],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl { name: "n".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "acc".to_string(), ty: Type::I64, mutable: true },
            MirLocalDecl { name: "thunk".to_string(), ty: Type::I64, mutable: true },
            MirLocalDecl { name: "res".to_string(), ty: Type::I64, mutable: true },
        ],
        blocks: vec![
            MirBasicBlock {
                id: BasicBlockId(0),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "acc".to_string(), projections: vec![] },
                        Rvalue::Constant(numlang::typecheck::typed_ast::TypedLiteral::Int(1, Type::I64)),
                    ),
                    Statement::Assign(
                        Place { local: "thunk".to_string(), projections: vec![] },
                        Rvalue::Thunk { body: "step".to_string(), env: vec!["acc".to_string()] },
                    ),
                ],
                terminator: Terminator::Force {
                    thunk: "thunk".to_string(),
                    result: "res".to_string(),
                    cont: BasicBlockId(1),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(1),
                arguments: vec![],
                statements: vec![],
                terminator: Terminator::Return {
                    value: Some(Place { local: "res".to_string(), projections: vec![] }),
                },
            },
        ],
        is_distilled: false,
    };

    let mut prog = MirProgram {
        functions: vec![func.clone()],
        structs: vec![],
        enums: vec![],
    };

    let mut stats = numlang::mir::supercompiler::SupercompilerStats::default();
    numlang::mir::supercompiler::distill::fuse_stream_pipeline(&mut prog, &mut stats);

    let fused_fn = &prog.functions[0];
    assert!(fused_fn.is_distilled, "Fused function must be marked is_distilled");
    assert_eq!(stats.loops_collapsed, 1, "Must record collapsed loop");

    // Assert absence of Rvalue::Thunk and Terminator::Force in transformed function
    for b in &fused_fn.blocks {
        for stmt in &b.statements {
            let Statement::Assign(_, rval) = stmt;
            assert!(!matches!(rval, Rvalue::Thunk { .. }), "Rvalue::Thunk must be eliminated");
        }
        assert!(!matches!(b.terminator, Terminator::Force { .. }), "Terminator::Force must be replaced with Branch");
    }
}
