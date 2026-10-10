//! General SSA Process-Tree Supercompiler for NumLang MIR.
//!
//! Evaluates functions with symbolic parameters, propagates positive branch conditions,
//! detects growth via fast homeomorphic embedding, solves recurrence closed forms ($O(N) \to O(1)$),
//! and residualizes back into optimized SSA Control Flow Graphs.

pub mod cache;
pub mod compact;
pub mod distill;
pub mod drive;
pub mod fusion;
pub mod futamura2;
pub mod generalize;
pub mod independence;
pub mod mrsc;
pub mod mrsc_oracle;
pub mod outliner;
pub mod parallel;
pub mod polyhedral;
pub mod polyhedral_ilp;
pub mod recurrence;
pub mod residualize;
pub mod state;
pub mod strength_reduce;
pub mod term;
pub mod validate;
pub mod whistle;

pub use futamura2::*;
pub use strength_reduce::*;

pub use cache::{
    compute_composite_hash, extract_callees, sha256_str, CacheKey, CachedSpecialization,
    DependencyGraph, SpecializationCache,
};
pub use compact::{compact_mir_function, compact_process_tree};
pub use distill::DistillationEngine;
pub use drive::{
    ProcessEdge, ProcessNode, ProcessNodeId, ProcessTree, RecurrenceResult, SupercompilerDriver,
    SupercompilerStats, TerminationWitness, WhistleFiring, WhistleKind,
};
pub use fusion::{find_fusion_candidates, fuse_loops, fuse_map_filter, FusionCandidate};
pub use independence::{
    collect_subtree_rw_set, find_parallel_knot_pairs, sets_are_independent, ReadWriteSet,
};
pub use mrsc::{
    MinCodeSizeObjective, MinDynamicBranchObjective, MrscCostModel, MrscCostVector, MrscObjective,
    MultiResultEngine, ParetoObjective, ResidualObjective,
};
pub use mrsc_oracle::{IddfsOracleConfig, MrscOracleEngine, OracleCandidate, OracleParetoFrontier};
pub use outliner::{
    compute_sequence_similarity, outline_program, BlockHasher, NormalizedBlock, NormalizedOp,
    NormalizedStatement, OutlinerConfig, OutlinerStats,
};
pub use parallel::supercompile_mir_program_parallel;
pub use polyhedral::fuse_polyhedral_stencils;
pub use polyhedral_ilp::{
    BareissSimplex, ConstraintOp, PlutoSchedule, PlutoScheduler, ScheduleVector, SimplexResult,
};
pub use recurrence::{
    detect_nonlinear_recurrence, detect_nway_linear_system, extract_linear_coeffs,
    simulate_mir_call, solve_cross_function_cycle, solve_nonlinear_recurrence,
    solve_nway_recurrence, NWayLinearSystem, NonlinearRecurrence,
};
pub use residualize::{residualize_process_tree, residualize_process_tree_parallel};
pub use state::Interval;
pub use validate::{
    check_satisfiability, verify_formula_validity, verify_program_equivalence, BoolFormula, BvExpr,
    KInductionCertificate, KInductionValidator, LoopInductionCandidate, SmtLib2Printer, SmtResult,
    TranslationValidator, ValidationCertificate, ValidationError,
};
pub use whistle::{is_embedded, is_instance_of, state_embeds};

use crate::mir::lower::{MirFunction, MirProgram};

/// Operating mode for the supercompiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SupercompileMode {
    #[default]
    Classic,
    Distill,
    Mrsc,
    MrscExhaustive,
}

/// Configuration level for supercompilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SupercompileLevel {
    Off,
    Standard,
    #[default]
    Full,
    Extreme,
}

/// Supercompiles an entire MIR program across all functions and returns aggregated stats.
pub fn supercompile_mir_program(program: &mut MirProgram) -> SupercompilerStats {
    supercompile_mir_program_with_mode(program, SupercompileMode::Classic, "size")
}

/// Supercompiles an entire MIR program with explicit mode (Classic, Distill, MRSC) and objective.
pub fn supercompile_mir_program_with_mode(
    program: &mut MirProgram,
    mode: SupercompileMode,
    objective: &str,
) -> SupercompilerStats {
    supercompile_mir_program_with_mode_options(program, mode, objective, false)
}

/// Supercompiles an entire MIR program with explicit mode, objective, and parallel residualization toggle.
pub fn supercompile_mir_program_with_mode_options(
    program: &mut MirProgram,
    mode: SupercompileMode,
    objective: &str,
    parallel_residualize: bool,
) -> SupercompilerStats {
    supercompile_mir_program_with_cache(program, mode, objective, parallel_residualize, None)
}

/// Supercompiles an entire MIR program with explicit mode, objective, parallel residualization, and optional specialization cache.
pub fn supercompile_mir_program_with_cache(
    program: &mut MirProgram,
    mode: SupercompileMode,
    objective: &str,
    parallel_residualize: bool,
    opt_cache: Option<&SpecializationCache>,
) -> SupercompilerStats {
    let mut total_stats = SupercompilerStats::default();

    // Pass 1: Phase 6 Higher-Order Deforestation & Stream Fusion
    for func in &mut program.functions {
        let candidates = find_fusion_candidates(func);
        for candidate in candidates {
            fuse_loops(func, &candidate);
        }
        fuse_map_filter(func);
        fuse_polyhedral_stencils(func);
    }

    // Pass 1.5: Whole-Program Hamilton Global Process-Tree Distillation
    if mode == SupercompileMode::Distill {
        let distill_stats = DistillationEngine::distill_program(program);
        total_stats.nodes_explored += distill_stats.nodes_explored;
        total_stats.branches_pruned += distill_stats.branches_pruned;
        total_stats.loops_collapsed += distill_stats.loops_collapsed;
        total_stats.knots_tied += distill_stats.knots_tied;
    }

    // Pass 2: Symbolic driving, recurrence solving, and SSA supercompilation
    let funcs_snapshot = program.functions.clone();
    let funcs_map: std::collections::HashMap<String, &MirFunction> =
        funcs_snapshot.iter().map(|f| (f.name.clone(), f)).collect();

    if let Some(cache) = opt_cache {
        for func in &funcs_snapshot {
            let callees = cache::extract_callees(func);
            cache.record_dependencies(&func.name, callees);
        }
    }

    for func in &mut program.functions {
        if func_is_impure(func) {
            total_stats.residual_block_count += func.blocks.len();
            total_stats.residual_stmt_count += func
                .blocks
                .iter()
                .map(|b| b.statements.len())
                .sum::<usize>();
            continue;
        }

        // Functions synthesized or transformed by global distillation are already in optimal single-pass form
        if mode == SupercompileMode::Distill && func.is_distilled {
            total_stats.residual_block_count += func.blocks.len();
            total_stats.residual_stmt_count += func
                .blocks
                .iter()
                .map(|b| b.statements.len())
                .sum::<usize>();
            continue;
        }

        match mode {
            SupercompileMode::Classic => {
                // 1. Compute the CacheKey for this function with composite hashing
                let source_hash = if opt_cache.is_some() {
                    cache::compute_composite_hash(&func.name, &funcs_map)
                } else {
                    sha256_str(&format!("{:?}", func))
                };
                let cache_key =
                    CacheKey::new(&func.name, source_hash, "generic").with_flags("mode=classic");

                // 2. Cache lookup
                if let Some(cache) = opt_cache {
                    if let Some(cached) = cache.lookup(&cache_key) {
                        if let Ok(residual) =
                            serde_json::from_str::<MirFunction>(&cached.residual_json)
                        {
                            *func = residual;
                            total_stats.nodes_explored += cached.stats_nodes_explored;
                            total_stats.branches_pruned += cached.stats_branches_pruned;
                            total_stats.loops_collapsed += cached.stats_loops_collapsed;
                            total_stats.knots_tied += cached.stats_knots_tied;
                            total_stats.calls_inlined += cached.stats_calls_inlined;
                            total_stats.sc_bce_eliminated += cached.stats_sc_bce_eliminated;
                            total_stats.residual_block_count += cached.stats_residual_block_count;
                            total_stats.residual_stmt_count += cached.stats_residual_stmt_count;
                            continue; // skip driving entirely
                        }
                    }
                }

                let (new_func, stats) = supercompile_mir_function_with_program_options(
                    func,
                    &funcs_snapshot,
                    parallel_residualize,
                );
                let baseline_blocks = func.blocks.len();
                let residual_blocks = new_func.blocks.len();
                let has_uncollapsed_array_loops =
                    func_has_array_writes(func) && stats.loops_collapsed == 0;
                // Revert if residual is more than 3× the baseline size AND no loops were collapsed
                // (if loops were collapsed the size is expected to shrink, not grow)
                let code_size_bloat = stats.loops_collapsed == 0
                    && residual_blocks > 3 * baseline_blocks
                    && baseline_blocks > 4; // only guard non-trivial functions

                let uncollapsed_knot = stats.knots_tied > 0 && stats.loops_collapsed == 0;
                let mut stats = stats;
                if !has_uncollapsed_array_loops && !code_size_bloat && !uncollapsed_knot {
                    *func = new_func;
                } else {
                    stats.residual_block_count = func.blocks.len();
                    stats.residual_stmt_count =
                        func.blocks.iter().map(|b| b.statements.len()).sum();
                }
                total_stats.nodes_explored += stats.nodes_explored;
                total_stats.branches_pruned += stats.branches_pruned;
                total_stats.loops_collapsed += stats.loops_collapsed;
                total_stats.knots_tied += stats.knots_tied;
                total_stats.calls_inlined += stats.calls_inlined;
                total_stats.sc_bce_eliminated += stats.sc_bce_eliminated;
                total_stats.residual_block_count += stats.residual_block_count;
                total_stats.residual_stmt_count += stats.residual_stmt_count;

                // 4. Cache store
                if let Some(cache) = opt_cache {
                    let entry = CachedSpecialization {
                        key: cache_key,
                        residual_json: serde_json::to_string(&func).unwrap_or_default(),
                        stats_nodes_explored: stats.nodes_explored,
                        stats_branches_pruned: stats.branches_pruned,
                        stats_loops_collapsed: stats.loops_collapsed,
                        stats_knots_tied: stats.knots_tied,
                        stats_calls_inlined: stats.calls_inlined,
                        stats_sc_bce_eliminated: stats.sc_bce_eliminated,
                        stats_residual_block_count: stats.residual_block_count,
                        stats_residual_stmt_count: stats.residual_stmt_count,
                    };
                    let _ = cache.store(&entry);
                }
            }
            SupercompileMode::Distill => {
                let driver = SupercompilerDriver::new(func).with_program_functions(&funcs_snapshot);
                let budget = driver.config.max_inline_nodes;
                let mut tree = driver.run();
                let mut interner_clone = tree.interner.clone();
                let mut distill = DistillationEngine::new(func, &mut interner_clone);
                let folds = distill.distill_process_tree(&mut tree);
                let mut stats = tree.stats.clone();
                let has_uncollapsed_array_loops =
                    func_has_array_writes(func) && stats.loops_collapsed == 0;
                let uncollapsed_knot = stats.knots_tied > 0 && stats.loops_collapsed == 0;
                if !has_uncollapsed_array_loops
                    && !uncollapsed_knot
                    && (is_profitable(&stats, budget, &tree)
                        || (tree.nodes.len() < budget && folds > 0))
                {
                    let (_dead, _deduped) = compact_process_tree(&mut tree);
                    let mut new_func =
                        residualize_process_tree_parallel(&tree, func, parallel_residualize);
                    let _stmts_removed = compact_mir_function(&mut new_func);
                    let _sr = strength_reduce_mir_function(&mut new_func);
                    if _sr > 0 {
                        compact_mir_function(&mut new_func);
                    }
                    stats.residual_block_count = new_func.blocks.len();
                    stats.residual_stmt_count =
                        new_func.blocks.iter().map(|b| b.statements.len()).sum();
                    *func = new_func;
                } else {
                    stats.residual_block_count = func.blocks.len();
                    stats.residual_stmt_count =
                        func.blocks.iter().map(|b| b.statements.len()).sum();
                }
                total_stats.nodes_explored += stats.nodes_explored;
                total_stats.branches_pruned += stats.branches_pruned;
                total_stats.loops_collapsed += stats.loops_collapsed + folds;
                total_stats.knots_tied += stats.knots_tied;
                total_stats.calls_inlined += stats.calls_inlined;
                total_stats.sc_bce_eliminated += stats.sc_bce_eliminated;
                total_stats.residual_block_count += stats.residual_block_count;
                total_stats.residual_stmt_count += stats.residual_stmt_count;
            }
            SupercompileMode::Mrsc => {
                let mrsc_engine = MultiResultEngine::new(func, &funcs_snapshot);
                let (best_res, best_tree, _) = match objective {
                    "branch" => mrsc_engine.explore_and_select(&MinDynamicBranchObjective),
                    "pareto" => mrsc_engine.explore_and_select(&ParetoObjective),
                    _ => mrsc_engine.explore_and_select(&MinCodeSizeObjective),
                };
                let mut stats = best_tree.stats.clone();
                let budget = SupercompilerDriver::new(func).config.max_inline_nodes;
                let has_uncollapsed_array_loops =
                    func_has_array_writes(func) && stats.loops_collapsed == 0;
                let uncollapsed_knot = stats.knots_tied > 0 && stats.loops_collapsed == 0;
                if !has_uncollapsed_array_loops
                    && !uncollapsed_knot
                    && is_profitable(&stats, budget, &best_tree)
                {
                    let mut new_func = best_res;
                    let _stmts_removed = compact_mir_function(&mut new_func);
                    let _sr = strength_reduce_mir_function(&mut new_func);
                    if _sr > 0 {
                        compact_mir_function(&mut new_func);
                    }
                    stats.residual_block_count = new_func.blocks.len();
                    stats.residual_stmt_count =
                        new_func.blocks.iter().map(|b| b.statements.len()).sum();
                    *func = new_func;
                } else {
                    stats.residual_block_count = func.blocks.len();
                    stats.residual_stmt_count =
                        func.blocks.iter().map(|b| b.statements.len()).sum();
                }
                total_stats.nodes_explored += stats.nodes_explored;
                total_stats.branches_pruned += stats.branches_pruned;
                total_stats.loops_collapsed += stats.loops_collapsed;
                total_stats.knots_tied += stats.knots_tied;
                total_stats.calls_inlined += stats.calls_inlined;
                total_stats.sc_bce_eliminated += stats.sc_bce_eliminated;
                total_stats.residual_block_count += stats.residual_block_count;
                total_stats.residual_stmt_count += stats.residual_stmt_count;
            }
            SupercompileMode::MrscExhaustive => {
                let mrsc_obj = match objective {
                    "speed" => MrscObjective::Speed,
                    "size" => MrscObjective::Size,
                    _ => MrscObjective::Balanced,
                };
                let config = IddfsOracleConfig {
                    min_depth: 2,
                    max_depth: 24,
                    step_depth: 2,
                    objective: mrsc_obj,
                    exhaustive: true,
                };
                let oracle = MrscOracleEngine::new(func, &funcs_snapshot, config);
                let (best_res, mut stats) = if let Some(cache) = opt_cache {
                    if let Ok((res, _cost, from_cache)) = oracle.run_with_cache(cache) {
                        let stats = SupercompilerStats {
                            residual_block_count: res.blocks.len(),
                            residual_stmt_count: res
                                .blocks
                                .iter()
                                .map(|b| b.statements.len())
                                .sum(),
                            loops_collapsed: if from_cache { 0 } else { 1 },
                            ..Default::default()
                        };
                        (res, stats)
                    } else {
                        let (_frontier, winner) = oracle.explore_iddfs();
                        let stats = winner.tree.stats.clone();
                        (winner.residual, stats)
                    }
                } else {
                    let (_frontier, winner) = oracle.explore_iddfs();
                    let stats = winner.tree.stats.clone();
                    (winner.residual, stats)
                };
                let mut new_func = best_res;
                let _stmts_removed = compact_mir_function(&mut new_func);
                let _sr = strength_reduce_mir_function(&mut new_func);
                if _sr > 0 {
                    compact_mir_function(&mut new_func);
                }
                stats.residual_block_count = new_func.blocks.len();
                stats.residual_stmt_count =
                    new_func.blocks.iter().map(|b| b.statements.len()).sum();
                *func = new_func;

                total_stats.nodes_explored += stats.nodes_explored;
                total_stats.branches_pruned += stats.branches_pruned;
                total_stats.loops_collapsed += stats.loops_collapsed;
                total_stats.knots_tied += stats.knots_tied;
                total_stats.calls_inlined += stats.calls_inlined;
                total_stats.sc_bce_eliminated += stats.sc_bce_eliminated;
                total_stats.residual_block_count += stats.residual_block_count;
                total_stats.residual_stmt_count += stats.residual_stmt_count;
            }
        }
    }

    if let Some(cache) = opt_cache {
        let _ = cache.save_dependency_graph();
    }

    let _prog_sr = strength_reduce_mir_program(program);

    total_stats
}

fn is_profitable(stats: &SupercompilerStats, budget: usize, tree: &ProcessTree) -> bool {
    tree.nodes.len() < budget
        && (stats.branches_pruned > 0
            || stats.loops_collapsed > 0
            || stats.knots_tied > 0
            || stats.calls_inlined > 0
            || stats.sc_bce_eliminated > 0)
}

fn func_is_impure(func: &MirFunction) -> bool {
    for b in &func.blocks {
        for stmt in &b.statements {
            if let crate::mir::lower::Statement::Assign(
                _,
                crate::mir::lower::Rvalue::Call(callee, _),
            ) = stmt
            {
                if callee == "print" || callee == "println" || callee == "exit" {
                    return true;
                }
            }
        }
    }
    false
}

fn func_has_array_writes(func: &MirFunction) -> bool {
    for b in &func.blocks {
        for stmt in &b.statements {
            let crate::mir::lower::Statement::Assign(dest, _) = stmt;
            if dest
                .projections
                .iter()
                .any(|p| matches!(p, crate::mir::Projection::Index(_)))
            {
                return true;
            }
        }
    }
    false
}

/// Supercompiles a single MIR function using the SSA Process-Tree engine.
pub fn supercompile_mir_function(func: &MirFunction) -> MirFunction {
    supercompile_mir_function_with_stats(func).0
}

/// Supercompiles a single MIR function and returns its performance metrics.
pub fn supercompile_mir_function_with_stats(
    func: &MirFunction,
) -> (MirFunction, SupercompilerStats) {
    let driver = SupercompilerDriver::new(func);
    let budget = driver.config.max_inline_nodes;
    let mut tree = driver.run();
    let mut stats = tree.stats.clone();
    let uncollapsed_knot = stats.knots_tied > 0 && stats.loops_collapsed == 0;
    // Profitability gate: only residualize if we achieved real reductions and did not hit budget explosion
    if !is_profitable(&stats, budget, &tree) || uncollapsed_knot {
        stats.residual_block_count = func.blocks.len();
        stats.residual_stmt_count = func.blocks.iter().map(|b| b.statements.len()).sum();
        return (func.clone(), stats);
    }
    let (_dead, _deduped) = compact_process_tree(&mut tree);
    let mut new_func = residualize_process_tree(&tree, func);
    let _stmts_removed = compact_mir_function(&mut new_func);
    let _sr = strength_reduce_mir_function(&mut new_func);
    if _sr > 0 {
        compact_mir_function(&mut new_func);
    }
    stats.residual_block_count = new_func.blocks.len();
    stats.residual_stmt_count = new_func.blocks.iter().map(|b| b.statements.len()).sum();
    (new_func, stats)
}

/// Supercompiles a single MIR function with context of all other functions in the program.
pub fn supercompile_mir_function_with_program(
    func: &MirFunction,
    program_funcs: &[MirFunction],
) -> (MirFunction, SupercompilerStats) {
    supercompile_mir_function_with_program_options(func, program_funcs, false)
}

/// Supercompiles a single MIR function with program context and optional parallel residualization.
pub fn supercompile_mir_function_with_program_options(
    func: &MirFunction,
    program_funcs: &[MirFunction],
    parallel_residualize: bool,
) -> (MirFunction, SupercompilerStats) {
    let driver = SupercompilerDriver::new(func).with_program_functions(program_funcs);
    let budget = driver.config.max_inline_nodes;
    let mut tree = driver.run();
    let mut stats = tree.stats.clone();
    let uncollapsed_knot = stats.knots_tied > 0 && stats.loops_collapsed == 0;
    // Profitability gate: only residualize if we achieved real reductions and did not hit budget explosion
    if !is_profitable(&stats, budget, &tree) || uncollapsed_knot {
        stats.residual_block_count = func.blocks.len();
        stats.residual_stmt_count = func.blocks.iter().map(|b| b.statements.len()).sum();
        return (func.clone(), stats);
    }
    let (_dead, _deduped) = compact_process_tree(&mut tree);
    let mut new_func = residualize_process_tree_parallel(&tree, func, parallel_residualize);
    let _stmts_removed = compact_mir_function(&mut new_func);
    let _sr = strength_reduce_mir_function(&mut new_func);
    if _sr > 0 {
        compact_mir_function(&mut new_func);
    }
    stats.residual_block_count = new_func.blocks.len();
    stats.residual_stmt_count = new_func.blocks.iter().map(|b| b.statements.len()).sum();
    (new_func, stats)
}

/// Generates the process tree for a function for inspection.
pub fn build_process_tree(func: &MirFunction) -> ProcessTree {
    let driver = SupercompilerDriver::new(func);
    driver.run()
}
