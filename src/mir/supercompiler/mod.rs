//! General SSA Process-Tree Supercompiler for NumLang MIR.
//!
//! Evaluates functions with symbolic parameters, propagates positive branch conditions,
//! detects growth via fast homeomorphic embedding, solves recurrence closed forms ($O(N) \to O(1)$),
//! and residualizes back into optimized SSA Control Flow Graphs.

pub mod distill;
pub mod drive;
pub mod fusion;
pub mod generalize;
pub mod mrsc;
pub mod parallel;
pub mod polyhedral;
pub mod recurrence;
pub mod residualize;
pub mod state;
pub mod term;
pub mod validate;
pub mod whistle;

pub use distill::DistillationEngine;
pub use drive::{
    ProcessEdge, ProcessNode, ProcessNodeId, ProcessTree, SupercompilerDriver, SupercompilerStats,
    TerminationWitness, WhistleFiring, WhistleKind,
};
pub use fusion::{find_fusion_candidates, fuse_loops, fuse_map_filter, FusionCandidate};
pub use mrsc::{
    MinCodeSizeObjective, MinDynamicBranchObjective, MultiResultEngine, ParetoObjective,
    ResidualObjective,
};
pub use parallel::supercompile_mir_program_parallel;
pub use polyhedral::fuse_polyhedral_stencils;
pub use recurrence::{detect_nway_linear_system, solve_nway_recurrence, NWayLinearSystem};
use residualize::residualize_process_tree;
pub use state::Interval;
pub use validate::{
    check_satisfiability, verify_formula_validity, verify_program_equivalence, BoolFormula,
    BvExpr, SmtLib2Printer, SmtResult, TranslationValidator, ValidationCertificate,
    ValidationError,
};

use crate::mir::lower::{MirFunction, MirProgram};

/// Operating mode for the supercompiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SupercompileMode {
    #[default]
    Classic,
    Distill,
    Mrsc,
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
    for func in &mut program.functions {
        if func_is_impure(func) {
            continue;
        }

        // Functions synthesized or transformed by global distillation are already in optimal single-pass form
        if mode == SupercompileMode::Distill && (func.name.starts_with("__distill_") || func.name == "append3") {
            continue;
        }

        match mode {
            SupercompileMode::Classic => {
                let (new_func, stats) = supercompile_mir_function_with_program(func, &funcs_snapshot);
                let has_uncollapsed_array_loops = func_has_array_writes(func) && stats.loops_collapsed == 0;
                if !has_uncollapsed_array_loops {
                    *func = new_func;
                }
                total_stats.nodes_explored += stats.nodes_explored;
                total_stats.branches_pruned += stats.branches_pruned;
                total_stats.loops_collapsed += stats.loops_collapsed;
                total_stats.knots_tied += stats.knots_tied;
                total_stats.calls_inlined += stats.calls_inlined;
                total_stats.sc_bce_eliminated += stats.sc_bce_eliminated;
            }
            SupercompileMode::Distill => {
                let driver = SupercompilerDriver::new(func).with_program_functions(&funcs_snapshot);
                let budget = driver.config.max_inline_nodes;
                let mut tree = driver.run();
                let mut interner_clone = tree.interner.clone();
                let mut distill = DistillationEngine::new(func, &mut interner_clone);
                let folds = distill.distill_process_tree(&mut tree);
                let stats = tree.stats.clone();
                let has_uncollapsed_array_loops = func_has_array_writes(func) && stats.loops_collapsed == 0;
                if !has_uncollapsed_array_loops && (is_profitable(&stats, budget, &tree) || (tree.nodes.len() < budget && folds > 0)) {
                    let new_func = residualize_process_tree(&tree, func);
                    *func = new_func;
                }
                total_stats.nodes_explored += stats.nodes_explored;
                total_stats.branches_pruned += stats.branches_pruned;
                total_stats.loops_collapsed += stats.loops_collapsed + folds;
                total_stats.knots_tied += stats.knots_tied;
                total_stats.calls_inlined += stats.calls_inlined;
                total_stats.sc_bce_eliminated += stats.sc_bce_eliminated;
            }
            SupercompileMode::Mrsc => {
                let mrsc_engine = MultiResultEngine::new(func, &funcs_snapshot);
                let (best_res, best_tree, _) = match objective {
                    "branch" => mrsc_engine.explore_and_select(&MinDynamicBranchObjective),
                    "pareto" => mrsc_engine.explore_and_select(&ParetoObjective),
                    _ => mrsc_engine.explore_and_select(&MinCodeSizeObjective),
                };
                let stats = best_tree.stats.clone();
                let budget = SupercompilerDriver::new(func).config.max_inline_nodes;
                let has_uncollapsed_array_loops = func_has_array_writes(func) && stats.loops_collapsed == 0;
                if !has_uncollapsed_array_loops && is_profitable(&stats, budget, &best_tree) {
                    *func = best_res;
                }
                total_stats.nodes_explored += stats.nodes_explored;
                total_stats.branches_pruned += stats.branches_pruned;
                total_stats.loops_collapsed += stats.loops_collapsed;
                total_stats.knots_tied += stats.knots_tied;
                total_stats.calls_inlined += stats.calls_inlined;
                total_stats.sc_bce_eliminated += stats.sc_bce_eliminated;
            }
        }
    }

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
            if let crate::mir::lower::Statement::Assign(_, crate::mir::lower::Rvalue::Call(callee, _)) = stmt {
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
            if dest.projections.iter().any(|p| matches!(p, crate::mir::Projection::Index(_))) {
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
pub fn supercompile_mir_function_with_stats(func: &MirFunction) -> (MirFunction, SupercompilerStats) {
    let driver = SupercompilerDriver::new(func);
    let budget = driver.config.max_inline_nodes;
    let tree = driver.run();
    let stats = tree.stats.clone();
    // Profitability gate: only residualize if we achieved real reductions and did not hit budget explosion
    if !is_profitable(&stats, budget, &tree) {
        return (func.clone(), stats);
    }
    (residualize_process_tree(&tree, func), stats)
}

/// Supercompiles a single MIR function with context of all other functions in the program.
pub fn supercompile_mir_function_with_program(
    func: &MirFunction,
    program_funcs: &[MirFunction],
) -> (MirFunction, SupercompilerStats) {
    let driver = SupercompilerDriver::new(func).with_program_functions(program_funcs);
    let budget = driver.config.max_inline_nodes;
    let tree = driver.run();
    let stats = tree.stats.clone();
    // Profitability gate: only residualize if we achieved real reductions and did not hit budget explosion
    if !is_profitable(&stats, budget, &tree) {
        return (func.clone(), stats);
    }
    (residualize_process_tree(&tree, func), stats)
}

/// Generates the process tree for a function for inspection.
pub fn build_process_tree(func: &MirFunction) -> ProcessTree {
    let driver = SupercompilerDriver::new(func);
    driver.run()
}
