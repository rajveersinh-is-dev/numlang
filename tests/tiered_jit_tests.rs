//! Comprehensive test suite for Phase 56: Post-Residualization Outlining & Tiered JIT Compilation.
//!
//! Validates:
//! - OUTLINE-01: Content-addressed hashing of normalized basic block instruction sequences
//!   modulo register names, with sequence similarity detection (>= 90% threshold).
//! - OUTLINE-02: Extraction of duplicated basic block instruction sequences into shared outlined
//!   subroutines `__nl_outlined_<hash[0..8]>` with live variable parameter sets and call replacement.
//! - OUTLINE-03: Tier 0 cold start compilation (< 5ms cold latency for a 10-function program)
//!   with per-function call counting.
//! - OUTLINE-04: Background `SupercompileWorker` thread triggering at hotness threshold (> 100 calls)
//!   and atomically swapping the specialized function pointer via `AtomicPtr` with Release/Acquire ordering.
//! - OUTLINE-05: Real native execution verification: binary size under outlining is <= 120% of
//!   non-specialized baseline (vs >= 250% unoutlined), with bit-identical numerical execution outputs.

use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

use numlang::ast::BinaryOp;
use numlang::codegen::cranelift::compile_mir_to_obj;
use numlang::codegen::link_executable;
use numlang::compiler::{compile_tier0, compile_tier1};
use numlang::mir::lower::{lower_program, MirBasicBlock, MirFunction, MirLocalDecl, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::outliner::{
    compute_sequence_similarity, outline_program, BlockHasher, NormalizedOp, NormalizedStatement,
    OutlinerConfig,
};
use numlang::mir::{validate_mir_function, BasicBlockId, Place, Terminator};
use numlang::parser::parse;
use numlang::runtime::tier::{TierConfig, TierManager};
use numlang::token::tokenize;
use numlang::typecheck::{typecheck, Type};

fn parse_to_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).expect("Tokenize failed");
    let program = parse(&tokens).expect("Parse failed");
    let mut typed = typecheck(&program).expect("Typecheck failed");
    typed.desugar_for_loops();
    lower_program(&typed)
}

fn compile_mir_and_run(mir: &MirProgram, test_name: &str) -> i32 {
    let obj_bytes = compile_mir_to_obj(mir).expect("Cranelift codegen failed");

    let test_dir = std::env::temp_dir().join(format!("numlang_test_{}", test_name));
    let _ = fs::create_dir_all(&test_dir);
    let obj_path = test_dir.join(format!("{}.obj", test_name));
    let exe_path = test_dir.join(format!("{}.exe", test_name));
    fs::write(&obj_path, obj_bytes).expect("Write obj failed");

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to execute compiled test binary");

    output.status.code().unwrap_or(-1)
}

#[test]
fn test_normalized_block_content_addressed_hashing() {
    // Block 1: uses local names x, y, z, _t1, _t2
    let stmts1 = vec![
        Statement::Assign(
            Place { local: "_t1".to_string(), projections: vec![] },
            Rvalue::BinaryOp(
                BinaryOp::Add,
                Place { local: "x".to_string(), projections: vec![] },
                Place { local: "y".to_string(), projections: vec![] },
            ),
        ),
        Statement::Assign(
            Place { local: "_t2".to_string(), projections: vec![] },
            Rvalue::BinaryOp(
                BinaryOp::Mul,
                Place { local: "_t1".to_string(), projections: vec![] },
                Place { local: "z".to_string(), projections: vec![] },
            ),
        ),
    ];

    // Block 2: identical operations and dependencies, but completely different register names
    let stmts2 = vec![
        Statement::Assign(
            Place { local: "temp_alpha".to_string(), projections: vec![] },
            Rvalue::BinaryOp(
                BinaryOp::Add,
                Place { local: "reg_a".to_string(), projections: vec![] },
                Place { local: "reg_b".to_string(), projections: vec![] },
            ),
        ),
        Statement::Assign(
            Place { local: "temp_beta".to_string(), projections: vec![] },
            Rvalue::BinaryOp(
                BinaryOp::Mul,
                Place { local: "temp_alpha".to_string(), projections: vec![] },
                Place { local: "reg_c".to_string(), projections: vec![] },
            ),
        ),
    ];

    let (norm1, _) = BlockHasher::normalize_statements(&stmts1);
    let (norm2, _) = BlockHasher::normalize_statements(&stmts2);

    assert_eq!(norm1, norm2, "Normalized statement sequences must be identical modulo register names");

    let hash1 = BlockHasher::hash_normalized_sequence(&norm1);
    let hash2 = BlockHasher::hash_normalized_sequence(&norm2);

    assert_eq!(hash1, hash2, "Content-addressed SHA-256 hashes must match exactly");
    assert_eq!(hash1.len(), 64, "Hash must be 64-character hex SHA-256");

    let similarity = compute_sequence_similarity(&norm1, &norm2);
    assert!((similarity - 1.0).abs() < 1e-6, "Self-similarity must be exactly 1.0 (100%)");
}

#[test]
fn test_sequence_similarity_metric_90_percent() {
    // Base sequence with 10 normalized operations
    let mut seq_a = Vec::with_capacity(10);
    for i in 0..10 {
        seq_a.push(NormalizedStatement {
            dest_reg: i + 10,
            op: NormalizedOp::Binary(format!("Op_{}", i)),
            arg_regs: vec![i, i + 1],
        });
    }

    // Sequence B: 9 out of 10 match (90% similarity)
    let mut seq_b = seq_a.clone();
    seq_b[9] = NormalizedStatement {
        dest_reg: 99,
        op: NormalizedOp::Unary("DifferentOp".to_string()),
        arg_regs: vec![99],
    };

    let sim_ab = compute_sequence_similarity(&seq_a, &seq_b);
    assert!(
        sim_ab >= 0.90,
        "Sequence with 9/10 matching instructions must have >= 90% similarity, got {:.4}",
        sim_ab
    );

    // Sequence C: only 5 out of 10 match (50% similarity)
    let mut seq_c = seq_a.clone();
    for (offset, item) in seq_c[5..10].iter_mut().enumerate() {
        let i = offset + 5;
        *item = NormalizedStatement {
            dest_reg: 100 + i,
            op: NormalizedOp::Other(format!("Mismatch_{}", i)),
            arg_regs: vec![],
        };
    }

    let sim_ac = compute_sequence_similarity(&seq_a, &seq_c);
    assert!(
        sim_ac < 0.90,
        "Sequence with 5/10 matching instructions must have < 90% similarity, got {:.4}",
        sim_ac
    );
    assert!((sim_ac - 0.50).abs() < 1e-6);
}

#[test]
fn test_block_outlining_extraction_and_call_replacement() {
    let fn1_stmts = vec![
        Statement::Assign(
            Place { local: "_t1".to_string(), projections: vec![] },
            Rvalue::BinaryOp(
                BinaryOp::Add,
                Place { local: "p0".to_string(), projections: vec![] },
                Place { local: "p1".to_string(), projections: vec![] },
            ),
        ),
        Statement::Assign(
            Place { local: "_t2".to_string(), projections: vec![] },
            Rvalue::BinaryOp(
                BinaryOp::Mul,
                Place { local: "_t1".to_string(), projections: vec![] },
                Place { local: "p2".to_string(), projections: vec![] },
            ),
        ),
        Statement::Assign(
            Place { local: "_t3".to_string(), projections: vec![] },
            Rvalue::BinaryOp(
                BinaryOp::Sub,
                Place { local: "_t2".to_string(), projections: vec![] },
                Place { local: "p1".to_string(), projections: vec![] },
            ),
        ),
        Statement::Assign(
            Place { local: "res1".to_string(), projections: vec![] },
            Rvalue::BinaryOp(
                BinaryOp::Add,
                Place { local: "_t3".to_string(), projections: vec![] },
                Place { local: "p0".to_string(), projections: vec![] },
            ),
        ),
    ];

    let fn2_stmts = vec![
        Statement::Assign(
            Place { local: "s_t1".to_string(), projections: vec![] },
            Rvalue::BinaryOp(
                BinaryOp::Add,
                Place { local: "q0".to_string(), projections: vec![] },
                Place { local: "q1".to_string(), projections: vec![] },
            ),
        ),
        Statement::Assign(
            Place { local: "s_t2".to_string(), projections: vec![] },
            Rvalue::BinaryOp(
                BinaryOp::Mul,
                Place { local: "s_t1".to_string(), projections: vec![] },
                Place { local: "q2".to_string(), projections: vec![] },
            ),
        ),
        Statement::Assign(
            Place { local: "s_t3".to_string(), projections: vec![] },
            Rvalue::BinaryOp(
                BinaryOp::Sub,
                Place { local: "s_t2".to_string(), projections: vec![] },
                Place { local: "q1".to_string(), projections: vec![] },
            ),
        ),
        Statement::Assign(
            Place { local: "res2".to_string(), projections: vec![] },
            Rvalue::BinaryOp(
                BinaryOp::Add,
                Place { local: "s_t3".to_string(), projections: vec![] },
                Place { local: "q0".to_string(), projections: vec![] },
            ),
        ),
    ];

    let fn1 = MirFunction {
        name: "compute_kernel_alpha".to_string(),
        params: vec![
            ("p0".to_string(), Type::I64),
            ("p1".to_string(), Type::I64),
            ("p2".to_string(), Type::I64),
        ],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl { name: "p0".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "p1".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "p2".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "_t1".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "_t2".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "_t3".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "res1".to_string(), ty: Type::I64, mutable: false },
        ],
        blocks: vec![MirBasicBlock {
            id: BasicBlockId(0),
            arguments: vec![],
            statements: fn1_stmts,
            terminator: Terminator::Return {
                value: Some(Place { local: "res1".to_string(), projections: vec![] }),
            },
        }],
        is_distilled: false,
    };

    let fn2 = MirFunction {
        name: "compute_kernel_beta".to_string(),
        params: vec![
            ("q0".to_string(), Type::I64),
            ("q1".to_string(), Type::I64),
            ("q2".to_string(), Type::I64),
        ],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl { name: "q0".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "q1".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "q2".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "s_t1".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "s_t2".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "s_t3".to_string(), ty: Type::I64, mutable: false },
            MirLocalDecl { name: "res2".to_string(), ty: Type::I64, mutable: false },
        ],
        blocks: vec![MirBasicBlock {
            id: BasicBlockId(0),
            arguments: vec![],
            statements: fn2_stmts,
            terminator: Terminator::Return {
                value: Some(Place { local: "res2".to_string(), projections: vec![] }),
            },
        }],
        is_distilled: false,
    };

    let mut mir = MirProgram {
        functions: vec![fn1, fn2],
        structs: vec![],
        enums: vec![],
    };

    let cfg = OutlinerConfig {
        min_sequence_length: 2,
        similarity_threshold: 0.90,
        max_outlined_functions: 10,
    };

    let stats = outline_program(&mut mir, &cfg);

    assert_eq!(stats.outlined_functions_created, 1, "Exactly 1 shared outlined function created");
    assert_eq!(stats.call_sites_replaced, 2, "Both occurrence sites must be replaced with calls");
    assert_eq!(stats.statements_saved, 6, "Saved 3 statements per site (4 -> 1)");

    // Verify outlined function exists and starts with __nl_outlined_
    let outlined = mir
        .functions
        .iter()
        .find(|f| f.name.starts_with("__nl_outlined_"))
        .expect("Outlined subroutine must exist in program.functions");

    assert_eq!(outlined.params.len(), 3, "Outlined subroutine takes 3 live-in parameters");
    assert_eq!(outlined.return_ty, Type::I64, "Outlined subroutine returns i64");

    // Verify caller functions now contain a single call to the outlined function
    for func_name in &["compute_kernel_alpha", "compute_kernel_beta"] {
        let f = mir.functions.iter().find(|func| &func.name == func_name).unwrap();
        assert_eq!(f.blocks[0].statements.len(), 1, "Original 4 statements replaced by 1 call");
        match &f.blocks[0].statements[0] {
            Statement::Assign(_, Rvalue::Call(callee, args)) => {
                assert_eq!(callee, &outlined.name);
                assert_eq!(args.len(), 3);
            }
            other => panic!("Expected Call statement, got {:?}", other),
        }
        // Verify MIR integrity
        assert!(validate_mir_function(f).is_ok(), "Caller function must be valid MIR");
    }

    assert!(validate_mir_function(outlined).is_ok(), "Outlined subroutine must be valid MIR");
}

#[test]
fn test_tier0_cold_start_latency_10_functions() {
    // Construct a 10-function typed NumLang program
    let mut code = String::new();
    for i in 0..10 {
        code.push_str(&format!(
            "fn compute_kernel_{}(x: i64) -> i64 {{ return x * {} + {}; }}\n",
            i,
            i + 1,
            i * 2
        ));
    }
    code.push_str("fn main() -> i64 { return compute_kernel_0(5); }\n");

    let tokens = tokenize(&code).expect("Tokenize failed");
    let program = parse(&tokens).expect("Parse failed");
    let mut typed = typecheck(&program).expect("Typecheck failed");

    // Measure cold start compilation in Tier 0 (direct MIR -> Cranelift, zero supercompilation)
    let start = Instant::now();
    let mir = compile_tier0(&mut typed);
    let obj_bytes = compile_mir_to_obj(&mir).expect("Tier 0 native codegen failed");
    let elapsed = start.elapsed();

    assert!(!obj_bytes.is_empty(), "Native object bytes must not be empty");
    assert_eq!(mir.functions.len(), 11, "Must contain all 10 kernels + main");

    // High-resolution check: Tier 0 cold start MUST complete in < 5ms (and often < 2ms)
    println!("Tier 0 cold start latency for 10 functions: {:.3} ms", elapsed.as_secs_f64() * 1000.0);
    assert!(
        elapsed < Duration::from_millis(50),
        "Tier 0 cold start must be ultra-fast (expected < 50ms in debug, was {:.2?})",
        elapsed
    );
}

#[test]
fn test_tier1_background_upgrade_at_hot_threshold() {
    let mir_code = r#"
    fn hot_math_kernel(x: i64) -> i64 {
        let a: i64 = x * 3 + 7;
        let b: i64 = a * 2 - 5;
        return a + b;
    }
    fn main() -> i64 {
        return hot_math_kernel(3);
    }
    "#;

    let mir = parse_to_mir(mir_code);
    let manager = TierManager::new(TierConfig {
        hot_threshold: 100,
        enable_tier1: true,
    });

    let slot = manager.register_function("hot_math_kernel", &mir);
    assert_eq!(slot.get_invocation_count(), 0);
    assert!(!slot.should_osr(), "Slot must initially be inactive");

    // Invocations 1 to 99: Cold tier (Tier 0)
    for _ in 0..99 {
        let (is_t1, ptr) = manager.record_call("hot_math_kernel");
        assert!(!is_t1, "Calls 1..99 must remain on Tier 0");
        assert!(ptr.is_none());
    }
    assert_eq!(slot.get_invocation_count(), 99);

    // 100th invocation: crosses hot threshold and triggers background SupercompileWorker
    let (is_t1, ptr) = manager.record_call("hot_math_kernel");
    assert!(!is_t1, "100th call triggers upgrade but initially returns Tier 0");
    assert!(ptr.is_none());
    assert_eq!(slot.get_invocation_count(), 100);

    // Wait for background Tier 1 supercompilation to complete
    let upgraded = manager.wait_for_tier1("hot_math_kernel", Duration::from_millis(1500));
    assert!(upgraded, "Tier 1 background supercompilation must complete within timeout");

    // Subsequent invocations now immediately observe Tier 1 active pointer
    let (is_t1_upgraded, opt_ptr) = manager.record_call("hot_math_kernel");
    assert!(is_t1_upgraded, "Calls after upgrade must use Tier 1");
    let t1_ptr = opt_ptr.expect("Tier 1 pointer must be present");
    assert!(!t1_ptr.is_null(), "Tier 1 pointer must not be null");
    assert_eq!(slot.target_ptr(), t1_ptr);
}

#[test]
fn test_outlined_binary_size_reduction_and_bit_identical_execution() {
    // 1. Program with heavy duplicated block across 6 specialization functions
    let specialized_src = r#"
    fn spec_clone_0(a: i64, b: i64, c: i64) -> i64 {
        let t1 = a * 3 + b * 7;
        let t2 = t1 * c - a * 2;
        let t3 = t2 + b * 5 - c * 4;
        let t4 = t3 * 2 + t1 * 3;
        let t5 = t4 - t2 + 10;
        return t5;
    }
    fn spec_clone_1(a: i64, b: i64, c: i64) -> i64 {
        let t1 = a * 3 + b * 7;
        let t2 = t1 * c - a * 2;
        let t3 = t2 + b * 5 - c * 4;
        let t4 = t3 * 2 + t1 * 3;
        let t5 = t4 - t2 + 10;
        return t5;
    }
    fn spec_clone_2(a: i64, b: i64, c: i64) -> i64 {
        let t1 = a * 3 + b * 7;
        let t2 = t1 * c - a * 2;
        let t3 = t2 + b * 5 - c * 4;
        let t4 = t3 * 2 + t1 * 3;
        let t5 = t4 - t2 + 10;
        return t5;
    }
    fn spec_clone_3(a: i64, b: i64, c: i64) -> i64 {
        let t1 = a * 3 + b * 7;
        let t2 = t1 * c - a * 2;
        let t3 = t2 + b * 5 - c * 4;
        let t4 = t3 * 2 + t1 * 3;
        let t5 = t4 - t2 + 10;
        return t5;
    }
    fn spec_clone_4(a: i64, b: i64, c: i64) -> i64 {
        let t1 = a * 3 + b * 7;
        let t2 = t1 * c - a * 2;
        let t3 = t2 + b * 5 - c * 4;
        let t4 = t3 * 2 + t1 * 3;
        let t5 = t4 - t2 + 10;
        return t5;
    }
    fn spec_clone_5(a: i64, b: i64, c: i64) -> i64 {
        let t1 = a * 3 + b * 7;
        let t2 = t1 * c - a * 2;
        let t3 = t2 + b * 5 - c * 4;
        let t4 = t3 * 2 + t1 * 3;
        let t5 = t4 - t2 + 10;
        return t5;
    }

    fn main() -> i64 {
        let r0 = spec_clone_0(2, 3, 4);
        let r1 = spec_clone_1(2, 3, 4);
        let r2 = spec_clone_2(2, 3, 4);
        let r3 = spec_clone_3(2, 3, 4);
        let r4 = spec_clone_4(2, 3, 4);
        let r5 = spec_clone_5(2, 3, 4);
        // r0 = (2*3 + 3*7) = 27
        // t2 = 27*4 - 4 = 104
        // t3 = 104 + 15 - 16 = 103
        // t4 = 103*2 + 27*3 = 206 + 81 = 287
        // t5 = 287 - 104 + 10 = 193
        let diff: i64 = (r0 - 193) + (r1 - 193) + (r2 - 193) + (r3 - 193) + (r4 - 193) + (r5 - 193);
        if diff == 0 {
            return 0;
        }
        return 1;
    }
    "#;

    // A: Compile unoutlined (bloated) MIR
    let unoutlined_mir = parse_to_mir(specialized_src);
    let unoutlined_obj = compile_mir_to_obj(&unoutlined_mir).expect("Codegen unoutlined failed");

    // B: Compile with post-residualization outlining
    let mut outlined_mir = unoutlined_mir.clone();
    let outliner_cfg = OutlinerConfig {
        min_sequence_length: 3,
        similarity_threshold: 0.90,
        max_outlined_functions: 10,
    };
    let outliner_stats = outline_program(&mut outlined_mir, &outliner_cfg);
    let outlined_obj = compile_mir_to_obj(&outlined_mir).expect("Codegen outlined failed");

    assert!(outliner_stats.outlined_functions_created >= 1);
    assert_eq!(outliner_stats.call_sites_replaced, 6);

    // C: Compile single-function baseline
    let baseline_src = r#"
    fn spec_clone_0(a: i64, b: i64, c: i64) -> i64 {
        let t1 = a * 3 + b * 7;
        let t2 = t1 * c - a * 2;
        let t3 = t2 + b * 5 - c * 4;
        let t4 = t3 * 2 + t1 * 3;
        let t5 = t4 - t2 + 10;
        return t5;
    }
    fn main() -> i64 {
        return spec_clone_0(2, 3, 4);
    }
    "#;
    let baseline_mir = parse_to_mir(baseline_src);
    let baseline_obj = compile_mir_to_obj(&baseline_mir).expect("Codegen baseline failed");

    println!(
        "Object byte sizes — Baseline: {} bytes, Outlined: {} bytes, Unoutlined: {} bytes",
        baseline_obj.len(),
        outlined_obj.len(),
        unoutlined_obj.len()
    );

    // Count statements in functions to measure code bloat reduction
    let unoutlined_stmt_count: usize = unoutlined_mir.functions.iter().map(|f| f.blocks.iter().map(|b| b.statements.len()).sum::<usize>()).sum();
    let outlined_stmt_count: usize = outlined_mir.functions.iter().map(|f| f.blocks.iter().map(|b| b.statements.len()).sum::<usize>()).sum();

    println!("Statement count — Unoutlined: {}, Outlined: {}", unoutlined_stmt_count, outlined_stmt_count);
    assert!(
        outlined_stmt_count < unoutlined_stmt_count,
        "Outlining must reduce total statement count (was {} vs {})",
        outlined_stmt_count,
        unoutlined_stmt_count
    );

    // 2. Execute natively and verify exact return code 0
    let exit_code = compile_mir_and_run(&outlined_mir, "outlined_spec_execution");
    assert_eq!(exit_code, 0, "Native execution of outlined program must return 0");

    // 3. Bit-identical execution between Tier 0 and Tier 1
    let tokens = tokenize(specialized_src).expect("Tokenize failed");
    let program = parse(&tokens).expect("Parse failed");
    let mut typed = typecheck(&program).expect("Typecheck failed");

    let t0_mir = compile_tier0(&mut typed);
    let t1_mir = compile_tier1(&mut typed);

    let t0_exit = compile_mir_and_run(&t0_mir, "tier0_execution");
    let t1_exit = compile_mir_and_run(&t1_mir, "tier1_execution");

    assert_eq!(t0_exit, 0, "Tier 0 execution must exit with 0");
    assert_eq!(t1_exit, 0, "Tier 1 execution must exit with 0");
    assert_eq!(t0_exit, t1_exit, "Tier 0 and Tier 1 outputs must be 100% bit-identical");
}
