use std::fs;
use std::process::Command;

use numlang::codegen::compile_supercompiled_to_obj_with_mode;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, MirProgram};
use numlang::mir::supercompiler::drive::SupercompilerStats;
use numlang::mir::supercompiler::mrsc::{
    CandidateMetrics, CustomWeightedObjective, MinCodeSizeObjective,
    MinDynamicBranchObjective, MultiResultEngine, ParetoCandidate, ParetoFrontier,
    ParetoObjective, ResidualObjective,
};
use numlang::mir::supercompiler::SupercompileMode;
use numlang::mir::Terminator;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn get_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).expect("Tokenize failed");
    let program = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&program).expect("Typecheck failed");
    lower_program(&typed)
}

fn compile_and_run_mode(src: &str, test_name: &str, mode: SupercompileMode, objective: &str) -> i32 {
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");

    let obj_bytes = compile_supercompiled_to_obj_with_mode(&typed, mode, objective)
        .expect("Codegen with mode failed");

    let test_dir = std::env::temp_dir().join(format!("numlang_mrsc_phase22_{}", test_name));
    let _ = fs::create_dir_all(&test_dir);
    let obj_path = test_dir.join(format!("{}.obj", test_name));
    let exe_path = test_dir.join(format!("{}.exe", test_name));
    fs::write(&obj_path, obj_bytes).expect("Write obj failed");

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to run binary");
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_dir(&test_dir);
    output.status.code().unwrap_or(-1)
}

#[test]
fn test_mrsc_hypergraph_construction_and_alternative_paths() {
    let code = r#"
    fn unroll_kernel(x: i64) -> i64 {
        let mut acc: i64 = x;
        let mut i: i64 = 0;
        while i < 3 {
            acc = acc * 2 + 1;
            i = i + 1;
        }
        return acc;
    }

    fn main() -> i64 {
        return unroll_kernel(5);
    }
    "#;

    let mir = get_mir(code);
    let func = mir.functions.iter().find(|f| f.name == "unroll_kernel").unwrap();

    let mrsc = MultiResultEngine::new(func, &mir.functions);
    let (hypergraph, frontier) = mrsc.explore_hypergraph();

    // Verify non-trivial hypergraph properties
    assert!(hypergraph.node_count() > 1, "Hypergraph must contain multiple configuration nodes");
    assert!(hypergraph.edge_count() > 1, "Hypergraph must contain multiple hyperedges");
    assert!(
        hypergraph.has_alternative_paths(),
        "Hypergraph must branch into alternative actions (e.g. unrolling vs knot-tying/recurrence)"
    );

    // Verify Pareto frontier contains multiple valid derivations
    assert!(!frontier.is_empty(), "Frontier must contain non-dominated candidates");
}

#[test]
fn test_mrsc_pareto_dominance_mathematical_properties() {
    let dummy_mir = get_mir("fn dummy() -> i64 { return 0; }");
    let dummy_func = &dummy_mir.functions[0];
    let dummy_tree = numlang::mir::supercompiler::drive::ProcessTree {
        nodes: Vec::new(),
        root: numlang::mir::supercompiler::drive::ProcessNodeId(0),
        interner: numlang::mir::supercompiler::term::TermInterner::new(),
        stats: SupercompilerStats::default(),
        witness: Default::default(),
    };

    // Candidate A: small code, few branches, few steps
    let m_a = CandidateMetrics {
        code_size: 10.0,
        dynamic_branches: 2.0,
        dynamic_steps: 5.0,
    };

    // Candidate B: strictly worse than A on all dimensions
    let m_b = CandidateMetrics {
        code_size: 20.0,
        dynamic_branches: 5.0,
        dynamic_steps: 15.0,
    };

    // Candidate C: trade-off (better size, worse branches)
    let m_c = CandidateMetrics {
        code_size: 5.0,
        dynamic_branches: 10.0,
        dynamic_steps: 5.0,
    };

    // 1. Strict dominance
    assert!(m_a.dominates(&m_b), "A must dominate B");
    assert!(!m_b.dominates(&m_a), "B must NOT dominate A");
    assert!(!m_a.dominates(&m_a), "A must NOT dominate itself (strict inequality required)");

    // 2. Incomparability (Trade-off)
    assert!(!m_a.dominates(&m_c), "A must not dominate C (C has smaller size)");
    assert!(!m_c.dominates(&m_a), "C must not dominate A (A has fewer branches)");

    // 3. Pareto frontier insertion and pruning
    let mut frontier = ParetoFrontier::new();

    let cand_b = ParetoCandidate {
        name: "B".to_string(),
        residual: dummy_func.clone(),
        tree: dummy_tree.clone(),
        metrics: m_b,
    };
    frontier.insert(cand_b);
    assert_eq!(frontier.len(), 1);

    // Inserting A should prune B because A dominates B
    let cand_a = ParetoCandidate {
        name: "A".to_string(),
        residual: dummy_func.clone(),
        tree: dummy_tree.clone(),
        metrics: m_a,
    };
    assert!(frontier.insert(cand_a));
    assert_eq!(frontier.len(), 1);
    assert_eq!(frontier.candidates[0].name, "A");

    // Inserting C should be accepted alongside A (incomparable trade-off)
    let cand_c = ParetoCandidate {
        name: "C".to_string(),
        residual: dummy_func.clone(),
        tree: dummy_tree.clone(),
        metrics: m_c,
    };
    assert!(frontier.insert(cand_c));
    assert_eq!(frontier.len(), 2, "Both A and C must coexist on the Pareto frontier");
}

#[test]
fn test_mrsc_competing_objectives_selection_and_divergence() {
    // A kernel where unrolling eliminates branches completely (dynamic_branches = 0),
    // while structured loop / baseline retains branches but has compact size.
    let code = r#"
    fn kernel(x: i64) -> i64 {
        let mut acc: i64 = x;
        let mut i: i64 = 0;
        while i < 3 {
            acc = acc * 2 + 3;
            i = i + 1;
        }
        return acc;
    }

    fn main() -> i64 {
        return kernel(2);
    }
    "#;

    let mir = get_mir(code);
    let func = mir.functions.iter().find(|f| f.name == "kernel").unwrap();

    let mrsc = MultiResultEngine::new(func, &mir.functions);

    // 1. MinDynamicBranchObjective selects candidate with minimal branch overhead
    let (branch_res, branch_tree, branch_score) =
        mrsc.explore_and_select(&MinDynamicBranchObjective);

    let branch_count = branch_res
        .blocks
        .iter()
        .filter(|b| matches!(b.terminator, Terminator::BranchIf { .. } | Terminator::Switch { .. }))
        .count();
    assert_eq!(branch_count, 0, "Unrolled candidate under MinDynamicBranchObjective must have zero branches");

    // 2. MinCodeSizeObjective selects candidate with minimal instruction/block footprint
    let (size_res, _size_tree, size_score) = mrsc.explore_and_select(&MinCodeSizeObjective);

    let size_count: usize = size_res.blocks.iter().map(|b| 1 + b.statements.len()).sum();

    // Verify objective scores match candidate properties
    assert_eq!(
        size_score, size_count as f64,
        "MinCodeSize score must match exact block + statement count"
    );

    // Verify that branch score incorporates branch weights
    let expected_branch_score = MinDynamicBranchObjective.score(&branch_tree, &branch_res);
    assert_eq!(branch_score, expected_branch_score);

    // Pareto objective combines both
    let (pareto_res, _pareto_tree, pareto_score) = mrsc.explore_and_select(&ParetoObjective);
    assert!(pareto_score >= size_score, "Pareto score combines size and branch penalties");
    assert!(!pareto_res.blocks.is_empty(), "Pareto residual must be valid");
}

#[test]
fn test_mrsc_custom_weighted_objective() {
    let code = r#"
    fn loop_accum(n: i64) -> i64 {
        let mut s: i64 = 0;
        let mut i: i64 = 0;
        while i < 4 {
            s = s + n;
            i = i + 1;
        }
        return s;
    }

    fn main() -> i64 {
        return loop_accum(10);
    }
    "#;

    let mir = get_mir(code);
    let func = mir.functions.iter().find(|f| f.name == "loop_accum").unwrap();

    let mrsc = MultiResultEngine::new(func, &mir.functions);

    // Extreme Objective 1: 100% weight on code size
    let obj_size = CustomWeightedObjective {
        weight_code_size: 100.0,
        weight_branches: 0.0,
        weight_loop_steps: 0.0,
    };
    let (_res1, _tree1, score1) = mrsc.explore_and_select(&obj_size);
    assert!(score1 > 0.0);

    // Extreme Objective 2: 100% weight on branch elimination
    let obj_branch = CustomWeightedObjective {
        weight_code_size: 0.0,
        weight_branches: 100.0,
        weight_loop_steps: 0.0,
    };
    let (_res2, _tree2, score2) = mrsc.explore_and_select(&obj_branch);
    assert!(score2 >= 0.0);
}

#[test]
fn test_mrsc_end_to_end_execution_parity() {
    let code = r#"
    fn compute(x: i64) -> i64 {
        let mut acc: i64 = x;
        let mut i: i64 = 0;
        while i < 4 {
            acc = acc * 3 + 2;
            i = i + 1;
        }
        return acc;
    }

    fn main() -> i64 {
        // compute(1)
        // i=0: 1*3+2 = 5
        // i=1: 5*3+2 = 17
        // i=2: 17*3+2 = 53
        // i=3: 53*3+2 = 161
        return compute(1);
    }
    "#;

    // 1. Run under MRSC with size objective
    let res_size = compile_and_run_mode(code, "test_mrsc_size_exec", SupercompileMode::Mrsc, "size");
    assert_eq!(res_size, 161, "MRSC size-optimized execution must produce 161");

    // 2. Run under MRSC with branch objective
    let res_branch = compile_and_run_mode(code, "test_mrsc_branch_exec", SupercompileMode::Mrsc, "branch");
    assert_eq!(res_branch, 161, "MRSC branch-optimized execution must produce 161");

    // 3. Run under MRSC with pareto objective
    let res_pareto = compile_and_run_mode(code, "test_mrsc_pareto_exec", SupercompileMode::Mrsc, "pareto");
    assert_eq!(res_pareto, 161, "MRSC pareto-optimized execution must produce 161");
}
