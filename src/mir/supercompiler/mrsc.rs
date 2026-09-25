//! Multi-Result Supercompilation (MRSC - Mitchell & Klyuchnikov 2012).
//!
//! Replaces greedy whistle decisions with a non-deterministic search over the
//! process tree configuration lattice, scoring candidate residual programs
//! against multi-objective cost functions (e.g. minimal code size or branches).

use super::drive::{ProcessTree, SupercompilerDriver};
use super::residualize::residualize_process_tree;
use crate::mir::lower::MirFunction;

pub trait ResidualObjective {
    /// Score a process tree and its residual program (lower score is better).
    fn score(&self, tree: &ProcessTree, residual: &MirFunction) -> f64;
}

/// Minimizes the total number of basic blocks and instructions in the residual program.
pub struct MinCodeSizeObjective;

impl ResidualObjective for MinCodeSizeObjective {
    fn score(&self, _tree: &ProcessTree, residual: &MirFunction) -> f64 {
        let mut count = 0;
        for b in &residual.blocks {
            count += 1 + b.statements.len();
        }
        count as f64
    }
}

/// Minimizes dynamic branch points and loop headers.
pub struct MinDynamicBranchObjective;

impl ResidualObjective for MinDynamicBranchObjective {
    fn score(&self, _tree: &ProcessTree, residual: &MirFunction) -> f64 {
        let mut branch_score = 0.0;
        for b in &residual.blocks {
            match &b.terminator {
                crate::mir::Terminator::BranchIf { .. } => branch_score += 10.0,
                crate::mir::Terminator::Switch { targets, .. } => branch_score += (targets.len() * 5) as f64,
                _ => branch_score += 1.0,
            }
        }
        branch_score
    }
}

/// Multi-objective Pareto score balancing binary footprint and execution simplicity.
pub struct ParetoObjective;

impl ResidualObjective for ParetoObjective {
    fn score(&self, tree: &ProcessTree, residual: &MirFunction) -> f64 {
        let size_score = MinCodeSizeObjective.score(tree, residual);
        let branch_score = MinDynamicBranchObjective.score(tree, residual);
        size_score + (2.0 * branch_score)
    }
}

pub struct MultiResultEngine<'a> {
    func: &'a MirFunction,
    program_funcs: &'a [MirFunction],
}

impl<'a> MultiResultEngine<'a> {
    pub fn new(func: &'a MirFunction, program_funcs: &'a [MirFunction]) -> Self {
        MultiResultEngine { func, program_funcs }
    }

    /// Explores multiple process-tree generation strategies and returns the candidate
    /// minimizing the provided objective score.
    pub fn explore_and_select<O: ResidualObjective>(
        &self,
        objective: &O,
    ) -> (MirFunction, ProcessTree, f64) {
        let mut candidates: Vec<(MirFunction, ProcessTree)> = Vec::new();

        // Candidate Strategy 1: Standard SSA Process-Tree Driver
        let tree_std = SupercompilerDriver::new(self.func)
            .with_program_functions(self.program_funcs)
            .run();
        let res_std = residualize_process_tree(&tree_std, self.func);
        candidates.push((res_std, tree_std));

        // Candidate Strategy 2: Distilled Process Tree
        let mut tree_distill = SupercompilerDriver::new(self.func)
            .with_program_functions(self.program_funcs)
            .run();
        let mut interner_clone = tree_distill.interner.clone();
        let mut distill_engine = super::distill::DistillationEngine::new(self.func, &mut interner_clone);
        distill_engine.distill_process_tree(&mut tree_distill);
        let res_distill = residualize_process_tree(&tree_distill, self.func);
        candidates.push((res_distill, tree_distill));

        // Candidate Strategy 3: Identity / Unmodified Baseline
        let tree_id = SupercompilerDriver::new(self.func)
            .with_program_functions(self.program_funcs)
            .run();
        candidates.push((self.func.clone(), tree_id));

        // Score candidates and select minimum
        let mut best_idx = 0;
        let mut best_score = f64::MAX;

        for (idx, (res, tree)) in candidates.iter().enumerate() {
            let s = objective.score(tree, res);
            if s < best_score {
                best_score = s;
                best_idx = idx;
            }
        }

        let (best_res, best_tree) = candidates.remove(best_idx);
        (best_res, best_tree, best_score)
    }
}
