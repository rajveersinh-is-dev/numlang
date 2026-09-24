//! General SSA Process-Tree Supercompiler for NumLang MIR.
//!
//! Evaluates functions with symbolic parameters, propagates positive branch conditions,
//! detects growth via fast homeomorphic embedding, solves recurrence closed forms ($O(N) \to O(1)$),
//! and residualizes back into optimized SSA Control Flow Graphs.

pub mod drive;
pub mod generalize;
pub mod residualize;
pub mod state;
pub mod term;
pub mod whistle;

pub use drive::{
    ProcessEdge, ProcessNode, ProcessNodeId, ProcessTree, SupercompilerDriver, SupercompilerStats,
};
use residualize::residualize_process_tree;

use crate::mir::lower::{MirFunction, MirProgram};

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
    let mut total_stats = SupercompilerStats::default();
    for func in &mut program.functions {
        let (new_func, stats) = supercompile_mir_function_with_stats(func);
        *func = new_func;
        total_stats.nodes_explored += stats.nodes_explored;
        total_stats.branches_pruned += stats.branches_pruned;
        total_stats.loops_collapsed += stats.loops_collapsed;
        total_stats.knots_tied += stats.knots_tied;
    }
    total_stats
}

/// Supercompiles a single MIR function using the SSA Process-Tree engine.
pub fn supercompile_mir_function(func: &MirFunction) -> MirFunction {
    supercompile_mir_function_with_stats(func).0
}

/// Supercompiles a single MIR function and returns its performance metrics.
pub fn supercompile_mir_function_with_stats(func: &MirFunction) -> (MirFunction, SupercompilerStats) {
    let driver = SupercompilerDriver::new(func);
    let tree = driver.run();
    let stats = tree.stats.clone();
    (residualize_process_tree(&tree, func), stats)
}

/// Generates the process tree for a function for inspection.
pub fn build_process_tree(func: &MirFunction) -> ProcessTree {
    let driver = SupercompilerDriver::new(func);
    driver.run()
}
