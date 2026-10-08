use std::fs;
use std::path::PathBuf;
use std::process::Command;

use numlang::codegen::compile_supercompiled_to_obj_with_mode;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, MirProgram};
use numlang::mir::supercompiler::cache::SpecializationCache;
use numlang::mir::supercompiler::mrsc::{MrscCostModel, MrscCostVector, MrscObjective};
use numlang::mir::supercompiler::mrsc_oracle::{
    IddfsOracleConfig, MrscOracleEngine, OracleCandidate, OracleParetoFrontier,
};
use numlang::mir::supercompiler::{
    supercompile_mir_program_with_cache, SupercompileMode, SupercompilerDriver,
};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn get_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).expect("Tokenize failed");
    let program = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&program).expect("Typecheck failed");
    lower_program(&typed)
}

fn compile_and_run_oracle(
    src: &str,
    test_name: &str,
    mode: SupercompileMode,
    objective: &str,
) -> i32 {
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");

    let obj_bytes = compile_supercompiled_to_obj_with_mode(&typed, mode, objective)
        .expect("Codegen with mode failed");

    let test_dir = std::env::temp_dir().join(format!("numlang_mrsc_oracle_{}", test_name));
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
fn test_4d_cost_model_evaluation() {
    let code = r#"
    fn compute(n: i64) -> i64 {
        let mut sum: i64 = 0;
        let mut i: i64 = 0;
        while i < n {
            sum = sum + i;
            i = i + 1;
        }
        return sum;
    }
    "#;
    let mir = get_mir(code);
    let func = &mir.functions[0];
    let tree = SupercompilerDriver::new(func).run();

    let cost_model = MrscCostModel::new();
    let cost = cost_model.evaluate(func, &tree);

    assert!(cost.dynamic_steps > 0.0, "Dynamic steps should be computed");
    assert!(cost.residual_blocks >= 1, "Residual blocks must be >= 1");
    assert!(
        cost.register_pressure >= 1,
        "Register pressure must be >= 1"
    );
    assert_eq!(
        cost.allocation_count, 0,
        "No heap allocations in primitive loop"
    );

    let score_speed = cost_model.score(&cost, MrscObjective::Speed);
    let score_size = cost_model.score(&cost, MrscObjective::Size);
    let score_balanced = cost_model.score(&cost, MrscObjective::Balanced);

    assert!(score_speed > 0.0);
    assert!(score_size > 0.0);
    assert!(score_balanced > 0.0);
}

#[test]
fn test_pareto_dominance_4d() {
    let vec_a = MrscCostVector {
        dynamic_steps: 10.0,
        allocation_count: 1,
        residual_blocks: 4,
        register_pressure: 3,
    };
    let vec_b = MrscCostVector {
        dynamic_steps: 10.0,
        allocation_count: 1,
        residual_blocks: 5,
        register_pressure: 3,
    };
    let vec_c = MrscCostVector {
        dynamic_steps: 5.0,
        allocation_count: 2,
        residual_blocks: 4,
        register_pressure: 3,
    };
    let vec_d = MrscCostVector {
        dynamic_steps: 15.0,
        allocation_count: 2,
        residual_blocks: 6,
        register_pressure: 4,
    };

    // A dominates B (identical except A has fewer residual blocks)
    assert!(vec_a.dominates(&vec_b), "A should dominate B");
    assert!(!vec_b.dominates(&vec_a), "B should not dominate A");

    // A and C are mutually non-dominating (tradeoff between steps and allocations)
    assert!(!vec_a.dominates(&vec_c), "A should not dominate C");
    assert!(!vec_c.dominates(&vec_a), "C should not dominate A");

    // A and C both dominate D
    assert!(vec_a.dominates(&vec_d), "A should dominate D");
    assert!(vec_c.dominates(&vec_d), "C should dominate D");

    // Test OracleParetoFrontier
    let mut frontier = OracleParetoFrontier::new();
    let mir = get_mir("fn dummy() -> i64 { return 42; }");
    let dummy_func = mir.functions[0].clone();
    let dummy_tree = SupercompilerDriver::new(&dummy_func).run();

    let cand_a = OracleCandidate {
        depth: 2,
        residual: dummy_func.clone(),
        tree: dummy_tree.clone(),
        cost: vec_a,
        fitness_score: 1.0,
    };
    let cand_b = OracleCandidate {
        depth: 2,
        residual: dummy_func.clone(),
        tree: dummy_tree.clone(),
        cost: vec_b,
        fitness_score: 2.0,
    };
    let cand_c = OracleCandidate {
        depth: 4,
        residual: dummy_func.clone(),
        tree: dummy_tree.clone(),
        cost: vec_c,
        fitness_score: 1.5,
    };
    let cand_d = OracleCandidate {
        depth: 1,
        residual: dummy_func.clone(),
        tree: dummy_tree.clone(),
        cost: vec_d,
        fitness_score: 3.0,
    };

    assert!(frontier.insert(cand_a));
    // cand_b is dominated by cand_a, so insertion should return false
    assert!(!frontier.insert(cand_b));
    assert_eq!(frontier.len(), 1);

    // cand_c is non-dominated, insertion should succeed
    assert!(frontier.insert(cand_c));
    assert_eq!(frontier.len(), 2);

    // cand_d is dominated by both, insertion should return false
    assert!(!frontier.insert(cand_d));
    assert_eq!(frontier.len(), 2);
}

#[test]
fn test_iddfs_reaches_depth_20_and_reduces_steps() {
    let code = r#"
    fn power_kernel(base: i64) -> i64 {
        let mut result: i64 = 1;
        let mut count: i64 = 0;
        while count < 8 {
            result = result * base;
            count = count + 1;
        }
        return result;
    }
    "#;
    let mir = get_mir(code);
    let func = &mir.functions[0];

    // Configure IDDFS to explore unbounded depths up to 24 (depth >= 20)
    let config = IddfsOracleConfig {
        min_depth: 2,
        max_depth: 24,
        step_depth: 4,
        objective: MrscObjective::Speed,
        exhaustive: true,
    };

    let oracle = MrscOracleEngine::new(func, &mir.functions, config);
    let (frontier, winner) = oracle.explore_iddfs();

    assert!(
        !frontier.is_empty(),
        "Frontier must contain Pareto candidates"
    );
    assert!(
        frontier.max_depth_evaluated >= 20,
        "IDDFS must explore depths >= 20, reached: {}",
        frontier.max_depth_evaluated
    );
    assert!(
        frontier.total_evaluated >= 5,
        "Must evaluate candidates across deepening iterations"
    );

    // The winning candidate should have low dynamic steps
    assert!(winner.cost.dynamic_steps <= frontier.candidates[0].cost.dynamic_steps);
}

#[test]
fn test_pareto_objective_selection_deterministic() {
    let code = r#"
    fn fib_step(n: i64) -> i64 {
        let mut a: i64 = 0;
        let mut b: i64 = 1;
        let mut i: i64 = 0;
        while i < 6 {
            let temp: i64 = a + b;
            a = b;
            b = temp;
            i = i + 1;
        }
        return a;
    }
    "#;
    let mir = get_mir(code);
    let func = &mir.functions[0];

    let run_oracle_with_obj = |obj: MrscObjective| -> OracleCandidate {
        let config = IddfsOracleConfig {
            min_depth: 2,
            max_depth: 20,
            step_depth: 6,
            objective: obj,
            exhaustive: true,
        };
        let oracle = MrscOracleEngine::new(func, &mir.functions, config);
        let (_frontier, winner) = oracle.explore_iddfs();
        winner
    };

    let winner_speed1 = run_oracle_with_obj(MrscObjective::Speed);
    let winner_speed2 = run_oracle_with_obj(MrscObjective::Speed);
    let winner_size = run_oracle_with_obj(MrscObjective::Size);

    // Deterministic selection across repeated runs
    assert_eq!(
        winner_speed1.cost, winner_speed2.cost,
        "Pareto winner selection must be completely deterministic"
    );
    assert_eq!(
        winner_speed1.residual.blocks.len(),
        winner_speed2.residual.blocks.len()
    );

    // Cost model distinguishes Speed vs Size
    assert!(
        winner_size.cost.residual_blocks <= winner_speed1.cost.residual_blocks
            || winner_size.cost.register_pressure <= winner_speed1.cost.register_pressure
    );
}

#[test]
fn test_l2_cache_integration_bypasses_driving() {
    let code = r#"
    fn cached_kernel(x: i64) -> i64 {
        let mut val: i64 = x;
        let mut i: i64 = 0;
        while i < 4 {
            val = val * 3 + 2;
            i = i + 1;
        }
        return val;
    }
    "#;
    let mir = get_mir(code);
    let func = &mir.functions[0];

    let cache_dir = std::env::temp_dir().join(format!(
        "numlang_oracle_cache_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = fs::create_dir_all(&cache_dir);

    let config = IddfsOracleConfig {
        min_depth: 2,
        max_depth: 20,
        step_depth: 6,
        objective: MrscObjective::Balanced,
        exhaustive: true,
    };
    let oracle = MrscOracleEngine::new(func, &mir.functions, config);

    // First call: Cache miss, writes to L2 disk cache
    let (res1, cost1, from_cache1) = oracle
        .run_and_cache(&cache_dir)
        .expect("Oracle run_and_cache failed");
    assert!(!from_cache1, "First run should be a cache miss");

    // Second call: Cache hit, loads from L2 disk cache with zero driving overhead
    let (res2, cost2, from_cache2) = oracle
        .run_and_cache(&cache_dir)
        .expect("Oracle run_and_cache cached hit failed");
    assert!(from_cache2, "Second run must be a cache hit");
    assert_eq!(
        res1.blocks.len(),
        res2.blocks.len(),
        "Cached residual structure must match original"
    );
    assert_eq!(
        res1.name, res2.name,
        "Cached residual function name must match"
    );
    assert_eq!(cost1.residual_blocks, cost2.residual_blocks);

    // Third call using open cache handle: records in-memory L2 hit telemetry
    let cache = SpecializationCache::open(&cache_dir);
    let (_res3, _cost3, from_cache3) = oracle
        .run_with_cache(&cache)
        .expect("Oracle run_with_cache hit failed");
    assert!(
        from_cache3,
        "Third run with cache instance must be a cache hit"
    );
    assert!(
        cache.metrics().l2_hits >= 1,
        "Cache should record at least 1 L2 hit"
    );

    // Clean up temporary cache dir
    let _ = fs::remove_dir_all(&cache_dir);
}

#[test]
fn test_mrsc_exhaustive_program_supercompilation() {
    let code = r#"
    fn add_scaled(a: i64, b: i64) -> i64 {
        return a + b * 2;
    }

    fn run_algo(n: i64) -> i64 {
        let mut acc: i64 = 0;
        let mut i: i64 = 0;
        while i < 5 {
            acc = add_scaled(acc, i);
            i = i + 1;
        }
        return acc;
    }
    "#;
    let mut mir = get_mir(code);

    let cache_dir = PathBuf::from(".test_numlang_cache_p53");
    let cache = SpecializationCache::open(&cache_dir);

    let stats = supercompile_mir_program_with_cache(
        &mut mir,
        SupercompileMode::MrscExhaustive,
        "balanced",
        false,
        Some(&cache),
    );

    assert!(
        stats.residual_block_count > 0,
        "Residual blocks must be > 0"
    );
    assert!(stats.residual_stmt_count > 0, "Residual stmts must be > 0");

    let _ = fs::remove_dir_all(&cache_dir);
}

#[test]
fn test_e2e_executable_execution_with_mrsc_exhaustive() {
    let code = r#"
    fn compute_kernel() -> i64 {
        let mut x: i64 = 1;
        let mut i: i64 = 0;
        while i < 6 {
            x = x * 2 + 1;
            i = i + 1;
        }
        return x;
    }

    fn main() -> i64 {
        let ans: i64 = compute_kernel();
        if ans == 127 {
            return 0;
        }
        return 1;
    }
    "#;
    let exit_code = compile_and_run_oracle(
        code,
        "e2e_mrsc_exhaustive",
        SupercompileMode::MrscExhaustive,
        "speed",
    );
    assert_eq!(
        exit_code, 0,
        "Binary compiled with --mrsc-exhaustive must execute correctly and return 0"
    );
}
