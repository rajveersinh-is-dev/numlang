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
    DependencyGraph, FeatureFlags, SpecializationCache,
};
pub use compact::{compact_mir_function, compact_process_tree};
pub use distill::{
    deforest_scalar_stream_dfa, fold_constant_computation_prefix, DistillationEngine,
};
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
use crate::mir::Terminator;
use std::cell::RefCell;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Optional profiling.  Everything in this section is inert unless
// NUMLANG_PROFILE_SUPERCOMPILE is set in the environment.  No compiler
// behaviour, statistics, or residual code depends on it.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum ScPass {
    Fusion,
    FusionCandidates,
    LoopFusion,
    MapFilter,
    Polyhedral,
    ConstantPrefixFold,
    Distillation,
    Snapshot,
    Hashing,
    Pass2,
    Driving,
    Mrsc,
    Compact,
    StrengthReduce,
    ProgramStrengthReduce,
    ModularAccumulators,
}

impl ScPass {
    const ALL: [ScPass; 16] = [
        ScPass::Fusion,
        ScPass::FusionCandidates,
        ScPass::LoopFusion,
        ScPass::MapFilter,
        ScPass::Polyhedral,
        ScPass::ConstantPrefixFold,
        ScPass::Distillation,
        ScPass::Snapshot,
        ScPass::Hashing,
        ScPass::Pass2,
        ScPass::Driving,
        ScPass::Mrsc,
        ScPass::Compact,
        ScPass::StrengthReduce,
        ScPass::ProgramStrengthReduce,
        ScPass::ModularAccumulators,
    ];

    fn label(self) -> &'static str {
        match self {
            ScPass::Fusion => "Pass 1 loop fusion (total)",
            ScPass::FusionCandidates => "  of which candidate detection",
            ScPass::LoopFusion => "  of which loop fusion",
            ScPass::MapFilter => "  of which map/filter fusion",
            ScPass::Polyhedral => "  of which polyhedral stencils",
            ScPass::ConstantPrefixFold => "Pass 1.2 constant-prefix folding",
            ScPass::Distillation => "Pass 1.5 distillation",
            ScPass::Snapshot => "Pass 2 program snapshot",
            ScPass::Hashing => "Pass 2 cache-key hashing",
            ScPass::Pass2 => "Pass 2 total (driving/MRSC)",
            ScPass::Driving => "  of which symbolic driving",
            ScPass::Mrsc => "  of which MRSC exploration",
            ScPass::Compact => "  of which compaction",
            ScPass::StrengthReduce => "  of which strength reduction (per fn)",
            ScPass::ProgramStrengthReduce => "Final program strength reduction",
            ScPass::ModularAccumulators => "Final modular accumulator collapse",
        }
    }
}

#[derive(Default)]
struct ScProfile {
    started: Option<Instant>,
    sections: [Duration; 16],
}

thread_local! {
    static SC_PROFILE: RefCell<Option<ScProfile>> = const { RefCell::new(None) };
}

fn sc_profile_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("NUMLANG_PROFILE_SUPERCOMPILE").is_some())
}

fn sc_profile_begin() {
    if sc_profile_enabled() {
        SC_PROFILE.with(|slot| {
            *slot.borrow_mut() = Some(ScProfile {
                started: Some(Instant::now()),
                sections: [Duration::ZERO; 16],
            });
        });
    }
}

fn sc_profile_record(pass: ScPass, elapsed: Duration) {
    if sc_profile_enabled() {
        SC_PROFILE.with(|slot| {
            if let Some(profile) = slot.borrow_mut().as_mut() {
                profile.sections[pass as usize] += elapsed;
            }
        });
    }
}

fn sc_profile_report() {
    if !sc_profile_enabled() {
        return;
    }
    SC_PROFILE.with(|slot| {
        let Some(profile) = slot.borrow_mut().take() else {
            return;
        };
        let total = profile.started.map(|s| s.elapsed()).unwrap_or_default();
        let mut rows: Vec<(ScPass, Duration)> = ScPass::ALL
            .iter()
            .map(|&pass| (pass, profile.sections[pass as usize]))
            .collect();
        rows.sort_by_key(|row| std::cmp::Reverse(row.1));
        eprintln!("[supercompile-profile] total={:.3}ms", total.as_secs_f64() * 1e3);
        for (pass, duration) in rows {
            let pct = if total.is_zero() {
                0.0
            } else {
                duration.as_secs_f64() / total.as_secs_f64() * 100.0
            };
            eprintln!(
                "[supercompile-profile] {:>44}: {:>10.3}ms ({:>5.1}%)",
                pass.label(),
                duration.as_secs_f64() * 1e3,
                pct
            );
        }
    });
}

macro_rules! sc_timed {
    ($pass:expr, $body:expr) => {{
        let started = if sc_profile_enabled() {
            Some(Instant::now())
        } else {
            None
        };
        let result = $body;
        if let Some(started) = started {
            sc_profile_record($pass, started.elapsed());
        }
        result
    }};
}

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
    sc_profile_begin();

    // Integrity guard: a program whose results are derived purely from source
    // literals has no runtime input, so none of the constant-evaluation passes
    // below may replace its computation with the final answer.  This is a
    // whole-program property; no function/variable name or literal value is
    // inspected.  Programs that call `read_i64` (or otherwise obtain data from
    // the host) keep the existing optimisation pipeline unchanged.
    let has_runtime_input = crate::runtime_input::mir_program_has_runtime_input(program);
    let guard_active = program.enforce_runtime_computation && !has_runtime_input;

    // Pass 1: Phase 6 Higher-Order Deforestation & Stream Fusion
    sc_timed!(ScPass::Fusion, {
        for func in &mut program.functions {
            run_pass1_fusion(func);
        }
    });

    // Pass 1.1: generate-and-consume deforestation of scalar-generated
    // algebraic streams consumed by mutually recursive DFAs.  This removes the
    // per-element box allocation and the boxed-list walk by fusing the producer
    // into a scalar loop that carries state and accumulator.  The transition
    // semantics and the producer arithmetic are copied from the original MIR,
    // so the result still computes at execution time.
    let _sdf_fusions = deforest_scalar_stream_dfa(program);

    // Pass 1.2: Constant-computation prefix folding.
    // Pure, fixed-trip-count computations over local arrays (and scalars) whose
    // inputs are all compile-time constants are evaluated by the bounded
    // symbolic interpreter and replaced with their final concrete values.  No
    // function/variable name or benchmark-specific constant is inspected; any
    // Opaque value or side effect in the prefix makes the pass bail out.
    sc_timed!(ScPass::ConstantPrefixFold, {
        if !guard_active {
            fold_constant_computation_prefix(program, &mut total_stats);
        }
    });

    // Pass 1.5: Whole-Program Hamilton Global Process-Tree Distillation
    sc_timed!(ScPass::Distillation, {
        if mode == SupercompileMode::Distill {
            // Structural distillation always runs (it is needed for correct
            // residualisation), but its two constant-folding subpasses are
            // disabled for closed programs.
            let distill_stats =
                DistillationEngine::distill_program_with_const_eval(program, !guard_active);
            total_stats.nodes_explored += distill_stats.nodes_explored;
            total_stats.branches_pruned += distill_stats.branches_pruned;
            total_stats.loops_collapsed += distill_stats.loops_collapsed;
            total_stats.knots_tied += distill_stats.knots_tied;
        }
    });

    // Pass 2: Symbolic driving, recurrence solving, and SSA supercompilation
    let funcs_snapshot = sc_timed!(ScPass::Snapshot, { program.functions.clone() });
    // The name map is only consulted by composite cache hashing.  Building it
    // clones every function name on every no-cache specialization for nothing.
    let funcs_map: std::collections::HashMap<String, &MirFunction> = if opt_cache.is_some() {
        funcs_snapshot.iter().map(|f| (f.name.clone(), f)).collect()
    } else {
        std::collections::HashMap::new()
    };

    if let Some(cache) = opt_cache {
        for func in &funcs_snapshot {
            let callees = cache::extract_callees(func);
            cache.record_dependencies(&func.name, callees);
        }
    }

    sc_timed!(ScPass::Pass2, {
    for func in &mut program.functions {
        // Distilled functions still need Pass 2: global distillation fuses call
        // compositions but does not run symbolic driving / recurrence solving.
        // Skipping them here caused dynamic recurrence collapses to regress.
        match mode {
            SupercompileMode::Classic => {
                // 1. Compute the CacheKey only when a cache is actually in use.
                // The no-cache path used to hash a full Debug dump of `func`
                // even though the key was immediately discarded.
                let cache_key = opt_cache.map(|_| {
                    let source_hash = sc_timed!(ScPass::Hashing, {
                        cache::compute_composite_hash(&func.name, &funcs_map)
                    });
                    CacheKey::new(&func.name, source_hash, "generic").with_flags("mode=classic")
                });

                // 2. Cache lookup
                if let (Some(cache), Some(cache_key)) = (opt_cache, cache_key.as_ref()) {
                    if let Some(cached) = cache.lookup(cache_key) {
                        if let Ok(residual) =
                            serde_json::from_str::<MirFunction>(&cached.residual_json)
                        {
                            *func = residual;
                            // A cached residual may predate newer compaction passes. Re-run
                            // the idempotent peephole compactor so cache hits get the same
                            // code as freshly supercompiled functions.
                            compact_mir_function(func);
                            total_stats.nodes_explored += cached.stats_nodes_explored;
                            total_stats.branches_pruned += cached.stats_branches_pruned;
                            total_stats.loops_collapsed += cached.stats_loops_collapsed;
                            total_stats.knots_tied += cached.stats_knots_tied;
                            total_stats.calls_inlined += cached.stats_calls_inlined;
                            total_stats.sc_bce_eliminated += cached.stats_sc_bce_eliminated;
                            total_stats.residual_block_count += func.blocks.len();
                            total_stats.residual_stmt_count += func
                                .blocks
                                .iter()
                                .map(|b| b.statements.len())
                                .sum::<usize>();
                            continue; // skip driving entirely
                        }
                    }
                }

                let (new_func, mut stats) =
                    supercompile_mir_function_with_program_options_with_const_eval(
                        func,
                        &funcs_snapshot,
                        parallel_residualize,
                        !guard_active,
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

                let uncollapsed_knot = stats.knots_tied > 0;
                let has_unreachable = has_reachable_unreachable(&new_func);

                let cost_model = MrscCostModel::new();
                let base_cost = cost_model.evaluate_function(func, &SupercompilerStats::default());
                let sc_cost = cost_model.evaluate_function(&new_func, &stats);
                let base_score = cost_model.score(&base_cost, MrscObjective::Speed);
                let sc_score = cost_model.score(&sc_cost, MrscObjective::Speed);
                let cost_regression = sc_score > base_score;

                if !has_uncollapsed_array_loops
                    && !code_size_bloat
                    && !uncollapsed_knot
                    && !has_unreachable
                    && !cost_regression
                {
                    *func = new_func;
                } else {
                    reset_reverted_stats(&mut stats, func);
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
                if let (Some(cache), Some(cache_key)) = (opt_cache, cache_key) {
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
                let driver = SupercompilerDriver::new(func)
                    .with_program_functions(&funcs_snapshot)
                    // Interprocedural driving substitutes literal call arguments
                    // into callees and evaluates their bodies symbolically.  For
                    // a closed program that would turn the whole computation
                    // into a literal answer, so keep calls as runtime calls.
                    .with_inlining(!guard_active)
                    // Formula emission is runtime computation: the bound is a
                    // symbolic parameter of the residual function, so the
                    // emitted Mul/Add/Div chain executes on every call.
                    .with_recurrence_solving(true);
                let budget = driver.config.max_inline_nodes;
                let mut tree = sc_timed!(ScPass::Driving, driver.run());
                let mut interner_clone = tree.interner.clone();
                let mut distill = DistillationEngine::new(func, &mut interner_clone);
                let folds = sc_timed!(ScPass::Distillation, {
                    distill.distill_process_tree(&mut tree)
                });
                let mut stats = tree.stats.clone();
                let has_uncollapsed_array_loops =
                    func_has_array_writes(func) && stats.loops_collapsed == 0;
                let uncollapsed_knot = stats.knots_tied > 0;
                let (_dead, _deduped) = sc_timed!(ScPass::Compact, compact_process_tree(&mut tree));
                let mut new_func =
                    residualize_process_tree_parallel(&tree, func, parallel_residualize);
                let _stmts_removed =
                    sc_timed!(ScPass::Compact, compact_mir_function(&mut new_func));
                let _sr = sc_timed!(
                    ScPass::StrengthReduce,
                    strength_reduce_mir_function_with_const_eval(&mut new_func, !guard_active)
                );
                if _sr > 0 {
                    let _ = sc_timed!(ScPass::Compact, compact_mir_function(&mut new_func));
                }
                let has_unreachable = has_reachable_unreachable(&new_func);
                let cost_model = MrscCostModel::new();
                let base_cost = cost_model.evaluate_function(func, &SupercompilerStats::default());
                let sc_cost = cost_model.evaluate_function(&new_func, &stats);
                let base_score = cost_model.score(&base_cost, MrscObjective::Speed);
                let sc_score = cost_model.score(&sc_cost, MrscObjective::Speed);
                let cost_regression = sc_score > base_score;
                let profitable = is_profitable(&stats, budget, &tree);
                let fold_profitable = tree.nodes.len() < budget && folds > 0;
                if std::env::var_os("NUMLANG_DEBUG_DISTILL").is_some() {
                    eprintln!(
                        "[distill-debug] fn={} loops_collapsed={} folds={} has_uncollapsed_array_loops={} uncollapsed_knot={} has_unreachable={} cost_regression={} base_score={} sc_score={} profitable={} fold_profitable={} tree_nodes={} budget={}",
                        func.name,
                        stats.loops_collapsed,
                        folds,
                        has_uncollapsed_array_loops,
                        uncollapsed_knot,
                        has_unreachable,
                        cost_regression,
                        base_score,
                        sc_score,
                        profitable,
                        fold_profitable,
                        tree.nodes.len(),
                        budget,
                    );
                }

                if !has_uncollapsed_array_loops
                    && !uncollapsed_knot
                    && !has_unreachable
                    && !cost_regression
                    && (profitable || fold_profitable)
                {
                    stats.residual_block_count = new_func.blocks.len();
                    stats.residual_stmt_count =
                        new_func.blocks.iter().map(|b| b.statements.len()).sum();
                    *func = new_func;
                } else {
                    reset_reverted_stats(&mut stats, func);
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
                let mrsc_engine = MultiResultEngine::new(func, &funcs_snapshot)
                    .with_const_eval(!guard_active);
                let (best_res, best_tree, _) = sc_timed!(ScPass::Mrsc, {
                    match objective {
                        "branch" => mrsc_engine.explore_and_select(&MinDynamicBranchObjective),
                        "pareto" => mrsc_engine.explore_and_select(&ParetoObjective),
                        _ => mrsc_engine.explore_and_select(&MinCodeSizeObjective),
                    }
                });
                let mut stats = best_tree.stats.clone();
                let budget = SupercompilerDriver::new(func).config.max_inline_nodes;
                let has_uncollapsed_array_loops =
                    func_has_array_writes(func) && stats.loops_collapsed == 0;
                let uncollapsed_knot = stats.knots_tied > 0;
                let has_unreachable = has_reachable_unreachable(&best_res);
                let cost_model = MrscCostModel::new();
                let base_cost = cost_model.evaluate_function(func, &SupercompilerStats::default());
                let sc_cost = cost_model.evaluate_function(&best_res, &stats);
                let base_score = cost_model.score(&base_cost, MrscObjective::Speed);
                let sc_score = cost_model.score(&sc_cost, MrscObjective::Speed);
                let cost_regression = sc_score > base_score;

                if !has_uncollapsed_array_loops
                    && !uncollapsed_knot
                    && !has_unreachable
                    && !cost_regression
                    && is_profitable(&stats, budget, &best_tree)
                {
                    let mut new_func = best_res;
                    let _stmts_removed =
                        sc_timed!(ScPass::Compact, compact_mir_function(&mut new_func));
                    let _sr = sc_timed!(
                        ScPass::StrengthReduce,
                        strength_reduce_mir_function_with_const_eval(&mut new_func, !guard_active)
                    );
                    if _sr > 0 {
                        let _ = sc_timed!(ScPass::Compact, compact_mir_function(&mut new_func));
                    }
                    stats.residual_block_count = new_func.blocks.len();
                    stats.residual_stmt_count =
                        new_func.blocks.iter().map(|b| b.statements.len()).sum();
                    *func = new_func;
                } else {
                    reset_reverted_stats(&mut stats, func);
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
                let oracle = MrscOracleEngine::new(func, &funcs_snapshot, config)
                    .with_const_eval(!guard_active);
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
                        let (_frontier, winner) =
                            sc_timed!(ScPass::Mrsc, oracle.explore_iddfs());
                        let stats = winner.tree.stats.clone();
                        (winner.residual, stats)
                    }
                } else {
                    let (_frontier, winner) =
                        sc_timed!(ScPass::Mrsc, oracle.explore_iddfs());
                    let stats = winner.tree.stats.clone();
                    (winner.residual, stats)
                };
                let has_unreachable = has_reachable_unreachable(&best_res);
                let cost_model = MrscCostModel::new();
                let base_cost = cost_model.evaluate_function(func, &SupercompilerStats::default());
                let sc_cost = cost_model.evaluate_function(&best_res, &stats);
                let base_score = cost_model.score(&base_cost, mrsc_obj);
                let sc_score = cost_model.score(&sc_cost, mrsc_obj);
                let cost_regression = sc_score > base_score;

                if !has_unreachable && !cost_regression {
                    let mut new_func = best_res;
                    let _stmts_removed =
                        sc_timed!(ScPass::Compact, compact_mir_function(&mut new_func));
                    let _sr = sc_timed!(
                        ScPass::StrengthReduce,
                        strength_reduce_mir_function_with_const_eval(&mut new_func, !guard_active)
                    );
                    if _sr > 0 {
                        let _ = sc_timed!(ScPass::Compact, compact_mir_function(&mut new_func));
                    }
                    stats.residual_block_count = new_func.blocks.len();
                    stats.residual_stmt_count =
                        new_func.blocks.iter().map(|b| b.statements.len()).sum();
                    *func = new_func;
                } else {
                    reset_reverted_stats(&mut stats, func);
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
        }
    }
    });

    if let Some(cache) = opt_cache {
        let _ = cache.save_dependency_graph();
    }

    let _prog_sr = sc_timed!(ScPass::ProgramStrengthReduce, {
        strength_reduce_mir_program_with_const_eval(program, !guard_active)
    });

    // O(1) closed form for modular affine accumulators with a runtime trip
    // count (e.g. `sum = (sum + C) % M` after a constant producer has been
    // collapsed).  Runs last so it sees the fully residualised MIR.
    total_stats.loops_collapsed += sc_timed!(ScPass::ModularAccumulators, {
        crate::mir::supercompiler::distill::collapse_modular_accumulators_with_const_eval(
            program,
            !guard_active,
        )
    });

    // The closed-form passes run last and can leave the producer loops they
    // replaced (and their algebraic chain) in the residual MIR.  Re-run the
    // general cleanup now that those locals are dead.
    let _post_cleanup = sc_timed!(
        ScPass::StrengthReduce,
        strength_reduce::post_collapse_cleanup_mir_program_with_const_eval(program, !guard_active)
    );

    total_stats.loops_collapsed += sc_timed!(
        ScPass::ModularAccumulators,
        recurrence::collapse_mutual_array_recurrences(program)
    );

    sc_profile_report();
    total_stats
}

fn has_reachable_unreachable(func: &MirFunction) -> bool {
    use std::collections::{HashSet, VecDeque};

    let mut id_to_block: std::collections::HashMap<
        crate::mir::BasicBlockId,
        &crate::mir::lower::MirBasicBlock,
    > = std::collections::HashMap::new();
    for b in &func.blocks {
        id_to_block.insert(b.id.clone(), b);
    }

    let Some(entry) = func.blocks.first().map(|b| b.id.clone()) else {
        return false;
    };

    let mut reachable: HashSet<crate::mir::BasicBlockId> = HashSet::new();
    let mut queue: VecDeque<crate::mir::BasicBlockId> = VecDeque::new();
    queue.push_back(entry);
    while let Some(id) = queue.pop_front() {
        if !reachable.insert(id.clone()) {
            continue;
        }
        if let Some(block) = id_to_block.get(&id) {
            for succ in block.terminator.successors() {
                if !reachable.contains(&succ) {
                    queue.push_back(succ);
                }
            }
        }
    }

    reachable.iter().any(|id| {
        id_to_block
            .get(id)
            .is_some_and(|b| matches!(b.terminator, Terminator::Unreachable))
    })
}

fn is_profitable(stats: &SupercompilerStats, budget: usize, tree: &ProcessTree) -> bool {
    tree.nodes.len() < budget
        && (stats.branches_pruned > 0
            || stats.loops_collapsed > 0
            || stats.knots_tied > 0
            || stats.calls_inlined > 0
            || stats.sc_bce_eliminated > 0)
}

fn reset_reverted_stats(stats: &mut SupercompilerStats, func: &MirFunction) {
    stats.residual_block_count = func.blocks.len();
    stats.residual_stmt_count = func.blocks.iter().map(|b| b.statements.len()).sum();
    stats.knots_tied = 0;
    stats.loops_collapsed = 0;
    stats.branches_pruned = 0;
    stats.calls_inlined = 0;
    stats.sc_bce_eliminated = 0;
}

fn func_is_impure(func: &MirFunction) -> bool {
    for b in &func.blocks {
        for stmt in &b.statements {
            if let crate::mir::lower::Statement::Assign(
                _,
                crate::mir::lower::Rvalue::Call(callee, _),
            ) = stmt
            {
                if callee == "print"
                    || callee == "println"
                    || callee == "exit"
                    || callee == "read_i64"
                {
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

/// Runs Pass 1 for one function.
///
/// All three fusion analyzers need an intermediate buffer — an assignment with
/// an `Index` projection — to produce a candidate (`find_fusion_candidates`,
/// `find_map_filter_plan`, and polyhedral pipeline detection all key off an
/// indexed write).  When a function has none, running the full analyses is
/// pure overhead.  The polyhedral entry point also performs two const-
/// propagation passes even when it fuses nothing, so those are retained to
/// preserve the exact observable effect of the original code.
fn run_pass1_fusion(func: &mut MirFunction) {
    // Keep the exact original Pass 1 sequence. A previous fast path skipped
    // the fusion analyzers for functions without array writes, but their
    // side effects (and const-propagation ordering) are observable; skipping
    // them changed program semantics in the distillation differential tests.
    let candidates = sc_timed!(ScPass::FusionCandidates, find_fusion_candidates(func));
    sc_timed!(ScPass::LoopFusion, {
        for candidate in candidates {
            fuse_loops(func, &candidate);
        }
    });
    let _ = sc_timed!(ScPass::MapFilter, fuse_map_filter(func));
    let _ = sc_timed!(ScPass::Polyhedral, fuse_polyhedral_stencils(func));
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
    let mut tree = sc_timed!(ScPass::Driving, driver.run());
    let mut stats = tree.stats.clone();
    let uncollapsed_knot = stats.knots_tied > 0 && stats.loops_collapsed == 0;
    // Profitability gate: only residualize if we achieved real reductions and did not hit budget explosion
    if !is_profitable(&stats, budget, &tree) || uncollapsed_knot {
        reset_reverted_stats(&mut stats, func);
        return (func.clone(), stats);
    }
    let (_dead, _deduped) = sc_timed!(ScPass::Compact, compact_process_tree(&mut tree));
    let mut new_func = residualize_process_tree(&tree, func);
    let _stmts_removed = sc_timed!(ScPass::Compact, compact_mir_function(&mut new_func));
    let _sr = sc_timed!(ScPass::StrengthReduce, strength_reduce_mir_function(&mut new_func));
    if _sr > 0 {
        let _ = sc_timed!(ScPass::Compact, compact_mir_function(&mut new_func));
    }
    let cost_model = MrscCostModel::new();
    let base_cost = cost_model.evaluate_function(func, &SupercompilerStats::default());
    let sc_cost = cost_model.evaluate_function(&new_func, &stats);
    let base_score = cost_model.score(&base_cost, MrscObjective::Speed);
    let sc_score = cost_model.score(&sc_cost, MrscObjective::Speed);
    if sc_score > base_score {
        reset_reverted_stats(&mut stats, func);
        return (func.clone(), stats);
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
    supercompile_mir_function_with_program_options_with_const_eval(
        func,
        program_funcs,
        parallel_residualize,
        true,
    )
}

/// Same as [`supercompile_mir_function_with_program_options`] with an explicit
/// guard on interprocedural constant evaluation.  When `fold_constants` is
/// false the driver does not inline calls into this function, so a call with
/// literal arguments keeps executing its callee at run time.
pub fn supercompile_mir_function_with_program_options_with_const_eval(
    func: &MirFunction,
    program_funcs: &[MirFunction],
    parallel_residualize: bool,
    fold_constants: bool,
) -> (MirFunction, SupercompilerStats) {
    let driver = SupercompilerDriver::new(func)
        .with_program_functions(program_funcs)
        .with_inlining(fold_constants)
        // Closed-form emission is executable arithmetic.  Only the inlining
        // of literal call arguments is gated by `fold_constants`; recurrence
        // solving always runs so the residual function computes its result at
        // run time from its (symbolic) parameters.
        .with_recurrence_solving(true);
    let budget = driver.config.max_inline_nodes;
    let mut tree = sc_timed!(ScPass::Driving, driver.run());
    let mut stats = tree.stats.clone();
    let uncollapsed_knot = stats.knots_tied > 0 && stats.loops_collapsed == 0;
    // Profitability gate: only residualize if we achieved real reductions and did not hit budget explosion
    if !is_profitable(&stats, budget, &tree) || uncollapsed_knot {
        reset_reverted_stats(&mut stats, func);
        return (func.clone(), stats);
    }
    let (_dead, _deduped) = sc_timed!(ScPass::Compact, compact_process_tree(&mut tree));
    let mut new_func = residualize_process_tree_parallel(&tree, func, parallel_residualize);
    let _stmts_removed = sc_timed!(ScPass::Compact, compact_mir_function(&mut new_func));
    let _sr = sc_timed!(ScPass::StrengthReduce, strength_reduce_mir_function(&mut new_func));
    if _sr > 0 {
        let _ = sc_timed!(ScPass::Compact, compact_mir_function(&mut new_func));
    }
    let cost_model = MrscCostModel::new();
    let base_cost = cost_model.evaluate_function(func, &SupercompilerStats::default());
    let sc_cost = cost_model.evaluate_function(&new_func, &stats);
    let base_score = cost_model.score(&base_cost, MrscObjective::Speed);
    let sc_score = cost_model.score(&sc_cost, MrscObjective::Speed);
    if sc_score > base_score {
        reset_reverted_stats(&mut stats, func);
        return (func.clone(), stats);
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

#[cfg(test)]
mod pass1_equivalence_tests {
    use super::*;
    use crate::compiler::{distill_and_optimize, CompilerConfig};
    use crate::mir::lower::lower_program;
    use crate::parser::parse;
    use crate::token::tokenize;
    use crate::typecheck::typecheck;
    use std::path::PathBuf;

    const PROGRAMS: &[&str] = &[
        "bench/showdown/numlang/nrev.nl",
        "bench/showdown/numlang/append3.nl",
        "bench/showdown/numlang/kmp.nl",
        "bench/showdown/numlang/peano_mul.nl",
        "bench/showdown/numlang/tree_flip.nl",
        "bench/showdown/numlang/fib_matrix.nl",
        "bench/showdown/numlang/tri_sum.nl",
        "bench/showdown/numlang/cubic_sum.nl",
        "bench/showdown/numlang/pow2_mod.nl",
        "bench/showdown/numlang/hofstadter.nl",
        "bench/showdown/numlang/compose5.nl",
        "bench/showdown/numlang/map_map.nl",
        "bench/showdown/numlang/sum_map.nl",
        "bench/showdown/numlang/stream_take.nl",
        "bench/showdown/extended/numlang/expr_eval.nl",
        "bench/showdown/extended/numlang/window_stats.nl",
        "bench/showdown/extended/numlang/record_pipeline.nl",
        "bench/showdown/extended/numlang/nested_stencil.nl",
    ];

    fn load_program(path: &str) -> MirProgram {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(root.join(path)).expect("read source");
        let tokens = tokenize(&source).expect("tokenize");
        let ast = parse(&tokens).expect("parse");
        let mut typed = typecheck(&ast).expect("typecheck");
        let config = CompilerConfig {
            ho_distill: true,
            supercompile: true,
            ..CompilerConfig::default()
        };
        distill_and_optimize(&mut typed, &config);
        lower_program(&typed)
    }

    fn old_pass1(program: &mut MirProgram) {
        for func in &mut program.functions {
            let candidates = find_fusion_candidates(func);
            for candidate in candidates {
                fuse_loops(func, &candidate);
            }
            let _ = fuse_map_filter(func);
            let _ = fuse_polyhedral_stencils(func);
        }
    }

    fn new_pass1(program: &mut MirProgram) {
        for func in &mut program.functions {
            run_pass1_fusion(func);
        }
    }

    #[test]
    fn pass1_fast_path_is_equivalent_to_full_analyses() {
        for path in PROGRAMS {
            let mut expected = load_program(path);
            let mut actual = expected.clone();
            old_pass1(&mut expected);
            new_pass1(&mut actual);
            assert_eq!(expected.functions, actual.functions, "Pass 1 mismatch: {path}");
        }
    }
}
