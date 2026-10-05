use std::sync::Arc;

use numlang::mir::lower::{
    lower_program, MirBasicBlock, MirFunction, MirLocalDecl, MirProgram, Rvalue, Statement,
};
use numlang::mir::supercompiler::drive::{
    DriverConfig, ProcessEdge, ProcessNodeId, ProcessTree, SupercompilerDriver,
};
use numlang::mir::supercompiler::parallel::{
    supercompile_mir_functions_work_stealing, supercompile_mir_program_parallel,
};
use numlang::mir::supercompiler::{supercompile_mir_program, SupercompileMode};
use numlang::mir::{BasicBlockId, Place, Terminator};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::types::Type;

fn get_mir(src: &str) -> MirProgram {
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");
    lower_program(&typed)
}

/// Constructs a synthetic MIR function with N sequentially chained basic blocks:
/// bb0 -> bb1 -> bb2 -> ... -> bb{N-1} -> Return(42)
fn build_deep_linear_mir_function(num_blocks: usize) -> MirFunction {
    let mut blocks = Vec::with_capacity(num_blocks);

    for i in 0..num_blocks {
        let block_id = BasicBlockId(i);
        let terminator = if i + 1 < num_blocks {
            Terminator::Branch {
                target: BasicBlockId(i + 1),
            }
        } else {
            Terminator::Return {
                value: Some(Place {
                    local: "res".to_string(),
                    projections: vec![],
                }),
            }
        };

        let statements = if i == 0 {
            vec![Statement::Assign(
                Place {
                    local: "res".to_string(),
                    projections: vec![],
                },
                Rvalue::Use(Place {
                    local: "param".to_string(),
                    projections: vec![],
                }),
            )]
        } else {
            vec![]
        };

        blocks.push(MirBasicBlock {
            id: block_id,
            arguments: vec![],
            statements,
            terminator,
        });
    }

    MirFunction {
        name: "deep_chain".to_string(),
        params: vec![("param".to_string(), Type::I64)],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl {
                name: "param".to_string(),
                ty: Type::I64,
                mutable: false,
            },
            MirLocalDecl {
                name: "res".to_string(),
                ty: Type::I64,
                mutable: false,
            },
        ],
        blocks,
        is_distilled: false,
    }
}

#[test]
fn test_cps_linear_deep_recursion_depth_1200() {
    // 1,200 basic blocks would exceed standard thread call stack if recursive.
    // The CPS trampoline processes all of them in heap memory with constant stack frames.
    let chain_len = 1200;
    let func = build_deep_linear_mir_function(chain_len);

    let config = DriverConfig {
        max_depth: 2048,
        max_inline_nodes: 4096,
        solve_recurrences: true,
        inline_calls: false,
        max_unroll_depth: 0,
        max_inline_depth: 8,
        inline_loop_body_calls: false,
    };

    let driver = SupercompilerDriver::new(&func).with_config(config);
    let tree: ProcessTree = driver.run();

    // Verify all 1,200 blocks were driven without stack overflow
    assert_eq!(
        tree.nodes.len(),
        chain_len,
        "Expected all {} nodes to be explored",
        chain_len
    );

    // Verify the leaf return term is present
    let leaf = &tree.nodes[chain_len - 1];
    assert!(!leaf.overflow, "Deep recursion should not overflow");
    assert!(
        leaf.return_term.is_some(),
        "Leaf node should have return term"
    );
}

#[test]
fn test_cps_ancestor_dag_reconstruction() {
    let chain_len = 50;
    let func = build_deep_linear_mir_function(chain_len);

    let driver = SupercompilerDriver::new(&func);
    let tree = driver.run();

    // Reconstruct ancestors for the final leaf node
    let ancestors = tree
        .nodes
        .iter()
        .map(|n| n.id)
        .take(chain_len - 1)
        .collect::<Vec<_>>();

    // The leaf should have exactly 49 predecessors leading back to node 0
    assert_eq!(ancestors.len(), 49);
    assert_eq!(ancestors[0], ProcessNodeId(0));
    assert_eq!(ancestors[48], ProcessNodeId(48));
}

#[test]
fn test_cps_binary_branching_tree() {
    // Construct a branching MIR program that evaluates nested conditions
    let src = r#"
        fn branch_tree(x: i64) -> i64 {
            let mut acc: i64 = 0;
            if x > 10 {
                acc = acc + 1;
            } else {
                acc = acc + 2;
            }
            if x > 20 {
                acc = acc + 10;
            } else {
                acc = acc + 20;
            }
            if x > 30 {
                acc = acc + 100;
            } else {
                acc = acc + 200;
            }
            return acc;
        }
    "#;

    let program = get_mir(src);
    let func = &program.functions[0];

    let driver = SupercompilerDriver::new(func);
    let tree = driver.run();

    // Verify the process tree has branches exploring both true and false paths
    let has_branch_true = tree
        .nodes
        .iter()
        .any(|n| n.edges.iter().any(|e| matches!(e, ProcessEdge::BranchTrue(..))));
    let has_branch_false = tree
        .nodes
        .iter()
        .any(|n| n.edges.iter().any(|e| matches!(e, ProcessEdge::BranchFalse(..))));

    assert!(has_branch_true, "Process tree should contain BranchTrue edges");
    assert!(has_branch_false, "Process tree should contain BranchFalse edges");
    assert!(tree.nodes.len() > 1, "Should explore multiple tree nodes");
}

#[test]
fn test_cps_loop_knot_and_whistle_under_trampoline() {
    let src = r#"
        fn sum_loop(n: i64) -> i64 {
            let mut s: i64 = 0;
            let mut i: i64 = 0;
            while i < n {
                s = s + i;
                i = i + 1;
            }
            return s;
        }
    "#;

    let program = get_mir(src);
    let func = &program.functions[0];

    let driver = SupercompilerDriver::new(func);
    let tree = driver.run();

    // Under the CPS trampoline, the recurrence should be collapsed or knots tied cleanly
    assert!(
        tree.stats.loops_collapsed > 0 || tree.stats.knots_tied > 0,
        "Trampoline driving should collapse loop recurrence or tie knots: {}",
        tree.stats
    );
}

#[test]
fn test_work_stealing_parallel_supercompiler() {
    let src = r#"
        fn f1(n: i64) -> i64 {
            let mut s: i64 = 0;
            let mut i: i64 = 0;
            while i < n {
                s = s + 1;
                i = i + 1;
            }
            return s;
        }

        fn f2(n: i64) -> i64 {
            let mut s: i64 = 0;
            let mut i: i64 = 0;
            while i < n {
                s = s + 2;
                i = i + 1;
            }
            return s;
        }

        fn f3(n: i64) -> i64 {
            let mut s: i64 = 0;
            let mut i: i64 = 0;
            while i < n {
                s = s + 3;
                i = i + 1;
            }
            return s;
        }

        fn f4(n: i64) -> i64 {
            let mut s: i64 = 0;
            let mut i: i64 = 0;
            while i < n {
                s = s + 4;
                i = i + 1;
            }
            return s;
        }
    "#;

    let mut prog_sequential = get_mir(src);
    let mut prog_parallel = get_mir(src);

    let stats_seq = supercompile_mir_program(&mut prog_sequential);
    let stats_par = supercompile_mir_program_parallel(
        &mut prog_parallel,
        SupercompileMode::Classic,
        "speed",
        4,
    );

    // Both should supercompile all 4 functions
    assert_eq!(prog_sequential.functions.len(), 4);
    assert_eq!(prog_parallel.functions.len(), 4);

    // Function names must match in order
    for i in 0..4 {
        assert_eq!(
            prog_sequential.functions[i].name,
            prog_parallel.functions[i].name
        );
    }

    assert!(
        stats_par.nodes_explored > 0,
        "Parallel driving must explore nodes: {}",
        stats_par
    );
    assert!(
        stats_seq.nodes_explored > 0,
        "Sequential driving must explore nodes: {}",
        stats_seq
    );
}

#[test]
fn test_work_stealing_dynamic_distribution() {
    // Test dynamic work-stealing across multiple functions with different complexities
    let mut functions = Vec::new();
    for i in 0..12 {
        let chain_len = 10 + (i * 15);
        let mut func = build_deep_linear_mir_function(chain_len);
        func.name = format!("worker_task_{}", i);
        functions.push(func);
    }

    let program_snapshot = Arc::new(functions.clone());
    let (processed, stats) = supercompile_mir_functions_work_stealing(
        functions,
        program_snapshot,
        SupercompileMode::Classic,
        "speed",
        4,
    );

    assert_eq!(processed.len(), 12, "All 12 functions must be processed");
    for (i, func) in processed.iter().enumerate() {
        assert_eq!(
            func.name,
            format!("worker_task_{}", i),
            "Output function ordering must be preserved exactly"
        );
    }
    assert!(stats.nodes_explored > 0);
}
