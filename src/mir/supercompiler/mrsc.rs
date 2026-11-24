//! Multi-Result Supercompilation (MRSC - Mitchell & Klyuchnikov 2012).
//!
//! Replaces greedy whistle decisions with an explicit configuration hypergraph
//! exploration over alternative supercompilation choices (unfolding, knot-tying,
//! loop unrolling, recurrence solving, and distillation). Candidate residual
//! programs are evaluated along multiple competing objective dimensions, forming
//! a Pareto frontier from which the optimal residual program is extracted.

use std::collections::HashMap;
use std::sync::Mutex;

use super::drive::{DriverConfig, ProcessEdge, ProcessNodeId, ProcessTree, SupercompilerDriver};
use super::residualize::residualize_process_tree;
use super::term::SymTermId;
use crate::mir::lower::{MirFunction, Rvalue, Statement};
use crate::mir::{BasicBlockId, Place, Terminator};

// ============================================================================
// 1. Residual Objective Trait & Standard Implementations
// ============================================================================

pub trait ResidualObjective {
    /// Score a process tree and its residual program (lower score is better).
    fn score(&self, tree: &ProcessTree, residual: &MirFunction) -> f64;
    /// Descriptive name of the objective.
    fn name(&self) -> &'static str {
        "CustomObjective"
    }
}

/// Minimizes the total number of basic blocks and instructions in the residual program.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinCodeSizeObjective;

impl ResidualObjective for MinCodeSizeObjective {
    fn score(&self, _tree: &ProcessTree, residual: &MirFunction) -> f64 {
        let mut count = 0;
        for b in &residual.blocks {
            count += 1 + b.statements.len();
        }
        count as f64
    }

    fn name(&self) -> &'static str {
        "MinCodeSize"
    }
}

/// Minimizes dynamic branch points and loop headers.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinDynamicBranchObjective;

impl ResidualObjective for MinDynamicBranchObjective {
    fn score(&self, _tree: &ProcessTree, residual: &MirFunction) -> f64 {
        let mut branch_score = 0.0;
        for b in &residual.blocks {
            match &b.terminator {
                Terminator::BranchIf { .. } => branch_score += 10.0,
                Terminator::Switch { targets, .. } => branch_score += (targets.len() * 5) as f64,
                _ => branch_score += 1.0,
            }
        }
        branch_score
    }

    fn name(&self) -> &'static str {
        "MinDynamicBranch"
    }
}

/// Multi-objective Pareto score balancing binary footprint and execution simplicity.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParetoObjective;

impl ResidualObjective for ParetoObjective {
    fn score(&self, tree: &ProcessTree, residual: &MirFunction) -> f64 {
        let size_score = MinCodeSizeObjective.score(tree, residual);
        let branch_score = MinDynamicBranchObjective.score(tree, residual);
        size_score + (2.0 * branch_score)
    }

    fn name(&self) -> &'static str {
        "ParetoBalanced"
    }
}

/// User-parameterized multi-objective cost metric over code size, branches, and loop complexity.
#[derive(Debug, Clone, Copy)]
pub struct CustomWeightedObjective {
    pub weight_code_size: f64,
    pub weight_branches: f64,
    pub weight_loop_steps: f64,
}

impl ResidualObjective for CustomWeightedObjective {
    fn score(&self, tree: &ProcessTree, residual: &MirFunction) -> f64 {
        let metrics = CandidateMetrics::new(residual, tree);
        (self.weight_code_size * metrics.code_size)
            + (self.weight_branches * metrics.dynamic_branches)
            + (self.weight_loop_steps * metrics.dynamic_steps)
    }

    fn name(&self) -> &'static str {
        "CustomWeighted"
    }
}

// ============================================================================
// 2. Candidate Metrics & Pareto Dominance
// ============================================================================

/// Concrete measurements of a candidate residual program along competing dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CandidateMetrics {
    /// Number of basic blocks and instructions in the residual MIR
    pub code_size: f64,
    /// Penalty for control flow branch complexity (BranchIf, Switch)
    pub dynamic_branches: f64,
    /// Estimated dynamic iteration steps / recurrence evaluation complexity
    pub dynamic_steps: f64,
}

impl CandidateMetrics {
    pub fn new(residual: &MirFunction, tree: &ProcessTree) -> Self {
        let mut code_size = 0.0;
        let mut dynamic_branches = 0.0;

        for b in &residual.blocks {
            code_size += 1.0 + b.statements.len() as f64;
            match &b.terminator {
                Terminator::BranchIf { .. } => dynamic_branches += 10.0,
                Terminator::Switch { targets, .. } => {
                    dynamic_branches += (targets.len() * 5) as f64
                }
                _ => dynamic_branches += 1.0,
            }
        }

        // Loop steps: knots in tree imply loops; collapsed loops are O(1)
        let mut dynamic_steps = (tree.stats.knots_tied * 10) as f64;
        if tree.stats.loops_collapsed > 0 {
            dynamic_steps += 1.0;
        }

        CandidateMetrics {
            code_size,
            dynamic_branches,
            dynamic_steps,
        }
    }

    /// Checks Pareto dominance: self dominates other iff self is no worse on all
    /// dimensions and strictly better on at least one.
    pub fn dominates(&self, other: &CandidateMetrics) -> bool {
        let no_worse = self.code_size <= other.code_size
            && self.dynamic_branches <= other.dynamic_branches
            && self.dynamic_steps <= other.dynamic_steps;
        let strictly_better = self.code_size < other.code_size
            || self.dynamic_branches < other.dynamic_branches
            || self.dynamic_steps < other.dynamic_steps;
        no_worse && strictly_better
    }
}

/// A candidate residual program and its corresponding process tree derivation.
#[derive(Debug, Clone)]
pub struct ParetoCandidate {
    pub name: String,
    pub residual: MirFunction,
    pub tree: ProcessTree,
    pub metrics: CandidateMetrics,
}

/// The set of non-dominated candidates forming the Pareto frontier.
#[derive(Debug, Clone, Default)]
pub struct ParetoFrontier {
    pub candidates: Vec<ParetoCandidate>,
    /// Telemetry: the deterministic profitability decision from the last gated selection.
    pub selection: Option<MrscSelectionDecision>,
}

impl ParetoFrontier {
    pub fn new() -> Self {
        ParetoFrontier {
            candidates: Vec::new(),
            selection: None,
        }
    }

    /// Inserts a candidate into the frontier using Pareto dominance filtering.
    /// Rejects the candidate if dominated by any existing member.
    /// Removes any existing members dominated by the new candidate.
    pub fn insert(&mut self, candidate: ParetoCandidate) -> bool {
        if self
            .candidates
            .iter()
            .any(|c| c.metrics.dominates(&candidate.metrics))
        {
            return false;
        }
        self.candidates
            .retain(|c| !candidate.metrics.dominates(&c.metrics));
        self.candidates.push(candidate);
        true
    }

    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Selects the candidate in the frontier that minimizes the given objective.
    pub fn select_optimal<O: ResidualObjective>(
        &self,
        objective: &O,
    ) -> Option<(&ParetoCandidate, f64)> {
        if self.candidates.is_empty() {
            return None;
        }

        let mut best = &self.candidates[0];
        let mut best_score = objective.score(&best.tree, &best.residual);

        for cand in &self.candidates[1..] {
            let score = objective.score(&cand.tree, &cand.residual);
            // Deterministic even when two objectives produce the same floating-point score.
            let better = score < best_score
                || (score == best_score && cmp_candidate_for_tiebreak(cand, best).is_lt());
            if better {
                best_score = score;
                best = cand;
            }
        }

        Some((best, best_score))
    }
}

/// Stable tie-break for equal objective scores: smaller footprint, then lexical name.
fn cmp_candidate_for_tiebreak(a: &ParetoCandidate, b: &ParetoCandidate) -> std::cmp::Ordering {
    a.metrics
        .code_size
        .total_cmp(&b.metrics.code_size)
        .then_with(|| a.metrics.dynamic_steps.total_cmp(&b.metrics.dynamic_steps))
        .then_with(|| a.metrics.dynamic_branches.total_cmp(&b.metrics.dynamic_branches))
        .then_with(|| a.name.cmp(&b.name))
}

// ============================================================================
// 3. Configuration Hypergraph Representation
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HyperNodeId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HyperEdgeId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HyperAction {
    DriveStep {
        target_block: BasicBlockId,
    },
    BranchSplit {
        cond: SymTermId,
        then_block: BasicBlockId,
        else_block: BasicBlockId,
    },
    FoldKnot {
        ancestor_node: ProcessNodeId,
        target_block: BasicBlockId,
    },
    GeneralizeFold {
        ancestor_node: ProcessNodeId,
        target_block: BasicBlockId,
    },
    LoopRecurrenceCollapse {
        exit_block: BasicBlockId,
    },
    UnrollIteration {
        iteration: usize,
        next_block: BasicBlockId,
    },
    CallInline {
        callee: String,
    },
    TerminalReturn {
        return_term: Option<SymTermId>,
    },
    DistillationDeforestation,
    IdentityBaseline,
}

#[derive(Debug, Clone)]
pub struct HyperNode {
    pub id: HyperNodeId,
    pub block: BasicBlockId,
    pub depth: usize,
    pub outgoing_edges: Vec<HyperEdgeId>,
}

#[derive(Debug, Clone)]
pub struct HyperEdge {
    pub id: HyperEdgeId,
    pub source: HyperNodeId,
    pub action: HyperAction,
    pub targets: Vec<HyperNodeId>,
}

#[derive(Debug, Clone)]
pub struct ConfigurationHypergraph {
    pub nodes: Vec<HyperNode>,
    pub edges: Vec<HyperEdge>,
    pub root: HyperNodeId,
}

impl ConfigurationHypergraph {
    pub fn new(root_block: BasicBlockId) -> Self {
        let root = HyperNode {
            id: HyperNodeId(0),
            block: root_block,
            depth: 0,
            outgoing_edges: Vec::new(),
        };
        ConfigurationHypergraph {
            nodes: vec![root],
            edges: Vec::new(),
            root: HyperNodeId(0),
        }
    }

    pub fn add_node(&mut self, block: BasicBlockId, depth: usize) -> HyperNodeId {
        let id = HyperNodeId(self.nodes.len());
        self.nodes.push(HyperNode {
            id,
            block,
            depth,
            outgoing_edges: Vec::new(),
        });
        id
    }

    pub fn add_edge(
        &mut self,
        source: HyperNodeId,
        action: HyperAction,
        targets: Vec<HyperNodeId>,
    ) -> HyperEdgeId {
        let id = HyperEdgeId(self.edges.len());
        let edge = HyperEdge {
            id,
            source,
            action,
            targets,
        };
        self.edges.push(edge);
        self.nodes[source.0].outgoing_edges.push(id);
        id
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn get_node(&self, id: HyperNodeId) -> Option<&HyperNode> {
        self.nodes.get(id.0)
    }

    pub fn get_edge(&self, id: HyperEdgeId) -> Option<&HyperEdge> {
        self.edges.get(id.0)
    }

    pub fn outgoing_edges(&self, node: HyperNodeId) -> Vec<&HyperEdge> {
        if let Some(n) = self.nodes.get(node.0) {
            n.outgoing_edges
                .iter()
                .filter_map(|&e_id| self.edges.get(e_id.0))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Checks whether the hypergraph contains alternative choices (decision nodes with >1 outgoing action).
    pub fn has_alternative_paths(&self) -> bool {
        self.nodes.iter().any(|n| n.outgoing_edges.len() > 1)
    }
}

// ============================================================================
// 4. Multi-Result Supercompilation Engine
// ============================================================================

pub struct MultiResultEngine<'a> {
    func: &'a MirFunction,
    program_funcs: &'a [MirFunction],
    contract: MrscProfitabilityContract,
    budget: ExplorationBudget,
    /// Whether interprocedural constant evaluation is allowed.  Cleared by the
    /// pipeline for programs with no runtime input.
    fold_constants: bool,
    /// Observable telemetry for the most recent `explore_and_select*` call.
    last_decision: Mutex<Option<MrscSelectionDecision>>,
}

impl<'a> MultiResultEngine<'a> {
    pub fn new(func: &'a MirFunction, program_funcs: &'a [MirFunction]) -> Self {
        MultiResultEngine {
            func,
            program_funcs,
            contract: MrscProfitabilityContract::default(),
            budget: ExplorationBudget::default(),
            fold_constants: true,
            last_decision: Mutex::new(None),
        }
    }

    /// General runtime-input guard: set to false when the program is closed.
    pub fn with_const_eval(mut self, fold_constants: bool) -> Self {
        self.fold_constants = fold_constants;
        self
    }

    /// Creates an engine with an explicit "never slower than baseline" contract.
    pub fn new_with_contract(
        func: &'a MirFunction,
        program_funcs: &'a [MirFunction],
        contract: MrscProfitabilityContract,
    ) -> Self {
        let mut engine = Self::new(func, program_funcs);
        engine.contract = contract;
        engine
    }

    /// Overrides the profitability contract (used by deterministic fallback tests).
    pub fn with_contract(mut self, contract: MrscProfitabilityContract) -> Self {
        self.contract = contract;
        self
    }

    /// Overrides the bounded exploration budget.
    pub fn with_budget(mut self, budget: ExplorationBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Telemetry: the decision made by the most recent `explore_and_select*` call.
    pub fn last_decision(&self) -> Option<MrscSelectionDecision> {
        self.last_decision
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or(None)
    }

    /// Systematically explores the configuration hypergraph across multiple supercompilation
    /// strategies (unrolling, knot-tying, recurrence collapse, distillation, conservative baseline)
    /// and populates the Pareto frontier.
    pub fn explore_hypergraph(&self) -> (ConfigurationHypergraph, ParetoFrontier) {
        let options = ExplorationOptions {
            budget: self.budget.clone(),
        };
        self.explore_hypergraph_with_options(&options)
    }

    /// Same as `explore_hypergraph`, with an explicit bounded exploration budget.
    pub fn explore_hypergraph_with_options(
        &self,
        options: &ExplorationOptions,
    ) -> (ConfigurationHypergraph, ParetoFrontier) {
        self.explore_hypergraph_with_hook(options, &|_baseline, _frontier| Vec::new())
    }

    /// Exploration hook for external candidate providers (e.g. L2 cache replay or parallel
    /// workers). Supplied candidates pass through the same Pareto filter and profitability
    /// gate as built-in derivations. The hook is called once, after the bounded built-ins.
    pub fn explore_hypergraph_with_hook(
        &self,
        options: &ExplorationOptions,
        hook: &dyn Fn(&MirFunction, &ParetoFrontier) -> Vec<ParetoCandidate>,
    ) -> (ConfigurationHypergraph, ParetoFrontier) {
        let entry_block = if !self.func.blocks.is_empty() {
            self.func.blocks[0].id.clone()
        } else {
            BasicBlockId(0)
        };

        let mut hypergraph = ConfigurationHypergraph::new(entry_block.clone());
        let mut frontier = ParetoFrontier::new();

        // Node mapping across derivations: (block_id, depth) -> HyperNodeId
        let mut node_map: HashMap<(BasicBlockId, usize), HyperNodeId> = HashMap::new();
        node_map.insert((entry_block.clone(), 0), hypergraph.root);

        // --------------------------------------------------------------------
        // Derivation 1: Standard Supercompilation (Knot-Tying + Recurrence)
        // --------------------------------------------------------------------
        let tree_std = SupercompilerDriver::new(self.func)
            .with_program_functions(self.program_funcs)
            .with_config(DriverConfig {
                max_depth: 128,
                max_unroll_depth: 0,
                solve_recurrences: self.fold_constants,
                inline_calls: self.fold_constants,
                ..DriverConfig::default()
            })
            .run();

        self.record_tree_into_hypergraph(&tree_std, &mut hypergraph, &mut node_map);
        let res_std = residualize_process_tree(&tree_std, self.func);
        let metrics_std = CandidateMetrics::new(&res_std, &tree_std);
        frontier.insert(ParetoCandidate {
            name: "Standard_Supercompiled".to_string(),
            residual: res_std,
            tree: tree_std.clone(),
            metrics: metrics_std,
        });

        // --------------------------------------------------------------------
        // Derivation 2: Aggressive Loop Unrolling (Zero Branch / Loop Peeling)
        // --------------------------------------------------------------------
        let tree_unroll = SupercompilerDriver::new(self.func)
            .with_program_functions(self.program_funcs)
            .with_config(DriverConfig {
                max_depth: 128,
                max_unroll_depth: 4,
                solve_recurrences: self.fold_constants,
                inline_calls: self.fold_constants,
                ..DriverConfig::default()
            })
            .run();

        self.record_tree_into_hypergraph(&tree_unroll, &mut hypergraph, &mut node_map);
        let res_unroll = residualize_process_tree(&tree_unroll, self.func);
        let metrics_unroll = CandidateMetrics::new(&res_unroll, &tree_unroll);
        frontier.insert(ParetoCandidate {
            name: "Unrolled_Supercompiled".to_string(),
            residual: res_unroll,
            tree: tree_unroll,
            metrics: metrics_unroll,
        });

        // --------------------------------------------------------------------
        // Derivation 3: Structured Loops (No Recurrence Collapse)
        // --------------------------------------------------------------------
        let tree_no_rec = SupercompilerDriver::new(self.func)
            .with_program_functions(self.program_funcs)
            .with_config(DriverConfig {
                max_depth: 128,
                max_unroll_depth: 0,
                solve_recurrences: false,
                inline_calls: self.fold_constants,
                ..DriverConfig::default()
            })
            .run();

        self.record_tree_into_hypergraph(&tree_no_rec, &mut hypergraph, &mut node_map);
        let res_no_rec = residualize_process_tree(&tree_no_rec, self.func);
        let metrics_no_rec = CandidateMetrics::new(&res_no_rec, &tree_no_rec);
        frontier.insert(ParetoCandidate {
            name: "Structured_Loop_Supercompiled".to_string(),
            residual: res_no_rec,
            tree: tree_no_rec,
            metrics: metrics_no_rec,
        });

        // --------------------------------------------------------------------
        // Derivation 4: Global Process-Tree Distillation
        // --------------------------------------------------------------------
        let mut tree_distill = tree_std.clone();
        let mut interner_clone = tree_distill.interner.clone();
        let mut distill_engine =
            super::distill::DistillationEngine::new(self.func, &mut interner_clone);
        distill_engine.distill_process_tree(&mut tree_distill);
        let res_distill = residualize_process_tree(&tree_distill, self.func);
        let metrics_distill = CandidateMetrics::new(&res_distill, &tree_distill);

        let root_id = hypergraph.root;
        hypergraph.add_edge(
            root_id,
            HyperAction::DistillationDeforestation,
            vec![root_id],
        );

        frontier.insert(ParetoCandidate {
            name: "Distilled_Supercompiled".to_string(),
            residual: res_distill,
            tree: tree_distill,
            metrics: metrics_distill,
        });

        // --------------------------------------------------------------------
        // Derivation 5: Conservative / Identity Baseline
        // --------------------------------------------------------------------
        let tree_id = SupercompilerDriver::new(self.func)
            .with_program_functions(self.program_funcs)
            .with_inlining(self.fold_constants)
            .with_recurrence_solving(self.fold_constants)
            .run();

        let metrics_baseline = CandidateMetrics::new(self.func, &tree_id);
        hypergraph.add_edge(root_id, HyperAction::IdentityBaseline, vec![root_id]);

        frontier.insert(ParetoCandidate {
            name: "Identity_Baseline".to_string(),
            residual: self.func.clone(),
            tree: tree_id,
            metrics: metrics_baseline,
        });

        // Cheap Pareto expansion: a few additional unroll depths for small functions only.
        // The budget keeps compile time bounded even for hot, large functions.
        let builtin_count = frontier.len();
        if self.func.blocks.len() <= options.budget.max_blocks_for_extra {
            for depth in options
                .budget
                .extra_unroll_depths
                .iter()
                .take(options.budget.max_extra_candidates)
            {
                if *depth == 0 || *depth == 4 {
                    continue; // already covered by the two built-in unroll derivations
                }
                let tree = SupercompilerDriver::new(self.func)
                    .with_program_functions(self.program_funcs)
                    .with_config(DriverConfig {
                        max_depth: 128,
                        max_unroll_depth: *depth,
                        solve_recurrences: self.fold_constants,
                        inline_calls: self.fold_constants,
                        ..DriverConfig::default()
                    })
                    .run();
                self.record_tree_into_hypergraph(&tree, &mut hypergraph, &mut node_map);
                hypergraph.add_edge(
                    root_id,
                    HyperAction::UnrollIteration {
                        iteration: *depth,
                        next_block: entry_block.clone(),
                    },
                    vec![root_id],
                );
                let residual = residualize_process_tree(&tree, self.func);
                let metrics = CandidateMetrics::new(&residual, &tree);
                frontier.insert(ParetoCandidate {
                    name: format!("Unrolled_{}_Supercompiled", depth),
                    residual,
                    tree,
                    metrics,
                });
            }
        }

        // External providers (cache replay / parallel workers) get exactly one bounded call.
        for candidate in hook(self.func, &frontier) {
            frontier.insert(candidate);
        }
        debug_assert!(
            frontier.len() >= builtin_count || builtin_count == 0,
            "Pareto insertion must never discard every built-in candidate"
        );

        (hypergraph, frontier)
    }

    /// Records nodes and transitions from a ProcessTree into the shared ConfigurationHypergraph.
    fn record_tree_into_hypergraph(
        &self,
        tree: &ProcessTree,
        hypergraph: &mut ConfigurationHypergraph,
        node_map: &mut HashMap<(BasicBlockId, usize), HyperNodeId>,
    ) {
        if tree.nodes.is_empty() {
            return;
        }

        let mut tree_to_hyper: HashMap<ProcessNodeId, HyperNodeId> = HashMap::new();

        // 1. Map each process node to a HyperNode
        for (depth, node) in tree.nodes.iter().enumerate() {
            let key = (node.state.block.clone(), depth);
            let h_id = *node_map
                .entry(key)
                .or_insert_with(|| hypergraph.add_node(node.state.block.clone(), depth));
            tree_to_hyper.insert(node.id, h_id);
        }

        // 2. Map edges to HyperEdges
        for node in &tree.nodes {
            let src_h = tree_to_hyper[&node.id];

            if let Some(ret_term) = node.return_term {
                hypergraph.add_edge(
                    src_h,
                    HyperAction::TerminalReturn {
                        return_term: Some(ret_term),
                    },
                    vec![],
                );
            }

            for edge in &node.edges {
                match edge {
                    ProcessEdge::Step(target) => {
                        if let Some(&tgt_h) = tree_to_hyper.get(target) {
                            let target_block = tree.nodes[target.0].state.block.clone();
                            hypergraph.add_edge(
                                src_h,
                                HyperAction::DriveStep { target_block },
                                vec![tgt_h],
                            );
                        }
                    }
                    ProcessEdge::BranchTrue(target, cond) => {
                        if let Some(&tgt_h) = tree_to_hyper.get(target) {
                            let target_block = tree.nodes[target.0].state.block.clone();
                            hypergraph.add_edge(
                                src_h,
                                HyperAction::BranchSplit {
                                    cond: *cond,
                                    then_block: target_block.clone(),
                                    else_block: target_block,
                                },
                                vec![tgt_h],
                            );
                        }
                    }
                    ProcessEdge::BranchFalse(target, cond) => {
                        if let Some(&tgt_h) = tree_to_hyper.get(target) {
                            let target_block = tree.nodes[target.0].state.block.clone();
                            hypergraph.add_edge(
                                src_h,
                                HyperAction::BranchSplit {
                                    cond: *cond,
                                    then_block: target_block.clone(),
                                    else_block: target_block,
                                },
                                vec![tgt_h],
                            );
                        }
                    }
                    ProcessEdge::Knot(anc) => {
                        let target_block = tree.nodes[anc.0].state.block.clone();
                        hypergraph.add_edge(
                            src_h,
                            HyperAction::FoldKnot {
                                ancestor_node: *anc,
                                target_block,
                            },
                            vec![],
                        );
                    }
                }
            }
        }
    }

    /// Explores multiple process-tree generation strategies in the configuration hypergraph
    /// and extracts the optimal residual program according to the given objective.
    /// The result is guarded by the hard "never slower than baseline" contract.
    pub fn explore_and_select<O: ResidualObjective>(
        &self,
        objective: &O,
    ) -> (MirFunction, ProcessTree, f64) {
        let (residual, tree, score, _decision) = self.explore_and_select_with_report(objective);
        (residual, tree, score)
    }

    /// Like `explore_and_select`, but also returns the deterministic profit record. The record
    /// is exposed through the return value, `last_decision()` and `ParetoFrontier::selection`.
    pub fn explore_and_select_with_report<O: ResidualObjective>(
        &self,
        objective: &O,
    ) -> (MirFunction, ProcessTree, f64, MrscSelectionDecision) {
        let model = MrscCostModel::new();
        let baseline_tree = SupercompilerDriver::new(self.func)
            .with_program_functions(self.program_funcs)
            .with_inlining(self.fold_constants)
            .with_recurrence_solving(self.fold_constants)
            .run();
        let baseline_cost = model.evaluate_function(self.func, &baseline_tree.stats);

        let (_hypergraph, mut frontier) = self.explore_hypergraph();
        let selected = frontier
            .select_optimal(objective)
            .map(|(candidate, score)| (candidate.clone(), score));

        let mut decision = match &selected {
            Some((candidate, score)) => {
                let candidate_cost = model.evaluate(&candidate.residual, &candidate.tree);
                let mut decision =
                    model.decide(&baseline_cost, Some(&candidate_cost), self.contract);
                decision.candidate_name = Some(candidate.name.clone());
                decision.candidate_objective_score = *score;
                decision
            }
            None => {
                let mut decision = model.decide(&baseline_cost, None, self.contract);
                decision.candidate_objective_score = objective.score(&baseline_tree, self.func);
                decision
            }
        };
        // The gate always uses the calibrated cost model, but report both scores so callers
        // can see why an objective winner was rolled back.
        decision.candidate_objective_score = selected
            .as_ref()
            .map(|(_, score)| *score)
            .unwrap_or(decision.candidate_objective_score);

        frontier.selection = Some(decision.clone());
        if let Ok(mut guard) = self.last_decision.lock() {
            *guard = Some(decision.clone());
        }

        if decision.accepted {
            if let Some((candidate, score)) = selected {
                return (candidate.residual.clone(), candidate.tree.clone(), score, decision);
            }
        }

        let baseline_score = objective.score(&baseline_tree, self.func);
        (self.func.clone(), baseline_tree, baseline_score, decision)
    }
}

// ============================================================================
// 5. Calibrated MRSC Cost Model, Hard Fallback & Bounded Exploration (Phase 2)
// ============================================================================
//
// The model estimates five independent, non-negative quantities for one call of a
// residual function. Units are *not* nanoseconds: they are simple-MIR-instruction
// equivalents, calibrated against the read-only constants in
// `distill::calculate_function_cost`:
//
//   alloc = 50, call = 20, enum construction = 10, load = 5, block = 2, stmt = 1.
//
// `dynamic_steps` counts executed instruction equivalents: every statement contributes 1,
// a load contributes 5 extra and an enum construction 10 extra (matching distill), branches
// retain the legacy 10/5 control-flow penalty, and every block inside a natural loop is
// multiplied by an estimated trip count (constant when discoverable, otherwise
// `DEFAULT_LOOP_TRIP_COUNT`). We deliberately do **not** collapse all dynamic steps to 1.0
// when `loops_collapsed > 0`; that old shortcut made loop residuals look artificially free.
//
// `allocation_count` counts heap-allocating MIR rvalues (Alloc / ClosureAlloc / Thunk);
// `call_count` counts direct, indirect and forced calls; `residual_blocks` is the static CFG
// size; `register_pressure` is the maximum number of distinct locals referenced in one block
// (a spill costs a load/store, i.e. ~5 instruction equivalents per distill).
//
// Score = w_dyn * dynamic_steps + w_alloc * allocations + w_call * calls
//       + w_code * residual_blocks + w_reg * register_pressure.
//
// Speed weights are the distill constants verbatim (1, 50, 20, 2, 5). Size and Balanced
// rebalance emphasis but never set an execution-cost weight to zero, so a candidate slower
// on the dominant dimension cannot win a tie for free.

/// Legacy control-flow penalty for a conditional branch, in instruction equivalents.
pub const BRANCH_STEP_COST: f64 = 10.0;
/// Legacy control-flow penalty per switch target, in instruction equivalents.
pub const SWITCH_TARGET_STEP_COST: f64 = 5.0;
/// Fallback trip count for loops whose bound is not a compile-time constant.
pub const DEFAULT_LOOP_TRIP_COUNT: f64 = 16.0;
/// Upper bound on a single estimated trip count (keeps scores finite and bounded).
pub const MAX_LOOP_TRIP_COUNT: f64 = 256.0;
/// Upper bound on nested-loop multipliers.
pub const MAX_LOOP_MULTIPLIER: f64 = 4096.0;

/// Five-dimensional cost vector for a residual program (lower is better on every axis).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MrscCostVector {
    /// Estimated *dynamic* instruction count for one invocation (instruction equivalents).
    pub dynamic_steps: f64,
    /// Static count of heap allocation operations in the residual code.
    pub allocation_count: usize,
    /// Static count of direct/indirect calls and thunk forces.
    pub call_count: usize,
    /// Static number of basic blocks in the residual MIR CFG.
    pub residual_blocks: usize,
    /// Maximum distinct locals live/referenced in a single block (spill proxy).
    pub register_pressure: usize,
}

impl MrscCostVector {
    /// Pareto dominance over all five dimensions (strict on at least one).
    pub fn dominates(&self, other: &MrscCostVector) -> bool {
        let no_worse = self.dynamic_steps <= other.dynamic_steps
            && self.allocation_count <= other.allocation_count
            && self.call_count <= other.call_count
            && self.residual_blocks <= other.residual_blocks
            && self.register_pressure <= other.register_pressure;
        let strictly_better = self.dynamic_steps < other.dynamic_steps
            || self.allocation_count < other.allocation_count
            || self.call_count < other.call_count
            || self.residual_blocks < other.residual_blocks
            || self.register_pressure < other.register_pressure;
        no_worse && strictly_better
    }

    /// Deterministic total order used to break score ties (never hash order dependent).
    pub fn compare_for_selection(&self, other: &MrscCostVector) -> std::cmp::Ordering {
        self.dynamic_steps
            .total_cmp(&other.dynamic_steps)
            .then_with(|| self.allocation_count.cmp(&other.allocation_count))
            .then_with(|| self.call_count.cmp(&other.call_count))
            .then_with(|| self.residual_blocks.cmp(&other.residual_blocks))
            .then_with(|| self.register_pressure.cmp(&other.register_pressure))
    }
}

/// Explicit weights for the five cost dimensions (all non-negative; lower score is better).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CostWeights {
    pub dynamic_instruction: f64,
    pub allocation: f64,
    pub call: f64,
    pub code_size: f64,
    pub register_pressure: f64,
}

impl CostWeights {
    /// Distill constants verbatim: 1/50/20/2/5.
    pub const SPEED: Self = Self {
        dynamic_instruction: 1.0,
        allocation: 50.0,
        call: 20.0,
        code_size: 2.0,
        register_pressure: 5.0,
    };
    /// Code-footprint oriented; still charges dynamic instructions and calls.
    pub const SIZE: Self = Self {
        dynamic_instruction: 0.5,
        allocation: 5.0,
        call: 5.0,
        code_size: 20.0,
        register_pressure: 10.0,
    };
    /// Balanced emphasis across execution, allocation, calls and footprint.
    pub const BALANCED: Self = Self {
        dynamic_instruction: 2.0,
        allocation: 15.0,
        call: 15.0,
        code_size: 5.0,
        register_pressure: 2.0,
    };

    pub const fn for_objective(objective: MrscObjective) -> Self {
        match objective {
            MrscObjective::Speed => Self::SPEED,
            MrscObjective::Size => Self::SIZE,
            MrscObjective::Balanced => Self::BALANCED,
        }
    }

    /// Dot product with a cost vector; non-finite inputs are treated as infinitely expensive.
    pub fn dot(&self, cost: &MrscCostVector) -> f64 {
        let raw = self.dynamic_instruction * cost.dynamic_steps
            + self.allocation * cost.allocation_count as f64
            + self.call * cost.call_count as f64
            + self.code_size * cost.residual_blocks as f64
            + self.register_pressure * cost.register_pressure as f64;
        if raw.is_finite() {
            raw.max(0.0)
        } else {
            f64::INFINITY
        }
    }
}

/// Optimization objectives for selecting from the Pareto frontier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum MrscObjective {
    /// Prioritizes minimizing dynamic instructions and heap allocations.
    #[default]
    Speed,
    /// Prioritizes minimizing code size (basic blocks) and register pressure.
    Size,
    /// Harmonious weighted balance across all five dimensions.
    Balanced,
}

impl ResidualObjective for MrscObjective {
    fn score(&self, tree: &ProcessTree, residual: &MirFunction) -> f64 {
        let model = MrscCostModel::new();
        model.score(&model.evaluate(residual, tree), *self)
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Speed => "MrscSpeed",
            Self::Size => "MrscSize",
            Self::Balanced => "MrscBalanced",
        }
    }
}

/// Why a selection was accepted or rolled back (telemetry-friendly, stable strings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MrscFallbackReason {
    AcceptedImprovement,
    NoCandidate,
    NonFiniteCost,
    CostRegression,
    ImprovementBelowMargin,
}

impl MrscFallbackReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AcceptedImprovement => "accepted_improvement",
            Self::NoCandidate => "no_candidate",
            Self::NonFiniteCost => "non_finite_cost",
            Self::CostRegression => "cost_regression",
            Self::ImprovementBelowMargin => "improvement_below_margin",
        }
    }
}

/// Deterministic public record of a profitability decision.
#[derive(Debug, Clone, PartialEq)]
pub struct MrscSelectionDecision {
    pub accepted: bool,
    /// True iff the original baseline MIR/tree was returned.
    pub fallback: bool,
    pub reason: MrscFallbackReason,
    pub objective: MrscObjective,
    pub min_improvement: f64,
    pub baseline: MrscCostVector,
    pub candidate: Option<MrscCostVector>,
    pub baseline_score: f64,
    pub candidate_score: f64,
    /// Score under the caller's selecting objective (may differ from `objective`).
    pub candidate_objective_score: f64,
    pub candidate_name: Option<String>,
}

/// Contract for the hard "never slower than baseline" gate.
///
/// A candidate is accepted only when
/// `candidate_score < baseline_score * (1 - min_improvement)`.
/// The default 5% margin matches the 0.95 regression threshold used by benchmark CI;
/// `min_improvement = 0.0` gives strict improvement only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MrscProfitabilityContract {
    pub objective: MrscObjective,
    pub min_improvement: f64,
}

impl Default for MrscProfitabilityContract {
    fn default() -> Self {
        Self {
            objective: MrscObjective::Speed,
            min_improvement: 0.05,
        }
    }
}

impl MrscProfitabilityContract {
    /// Strict form: only accept a strictly cheaper residual.
    pub const STRICT: Self = Self {
        objective: MrscObjective::Speed,
        min_improvement: 0.0,
    };
    /// Always rolls back; used to exercise and audit the fallback path.
    pub const NEVER_ACCEPT: Self = Self {
        objective: MrscObjective::Speed,
        min_improvement: 2.0,
    };

    pub const fn new(objective: MrscObjective, min_improvement: f64) -> Self {
        Self {
            objective,
            min_improvement,
        }
    }
}

/// Bounded exploration budget: extra unroll depths are attempted only for small functions.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplorationBudget {
    pub extra_unroll_depths: Vec<usize>,
    pub max_extra_candidates: usize,
    pub max_blocks_for_extra: usize,
}

impl Default for ExplorationBudget {
    fn default() -> Self {
        Self {
            extra_unroll_depths: vec![1, 2],
            max_extra_candidates: 2,
            max_blocks_for_extra: 32,
        }
    }
}

/// Options for `explore_hypergraph_with_options` / `explore_hypergraph_with_hook`.
#[derive(Debug, Clone, Default)]
pub struct ExplorationOptions {
    pub budget: ExplorationBudget,
}

/// Calibrated five-dimensional cost model.
#[derive(Debug, Clone, Default)]
pub struct MrscCostModel;

impl MrscCostModel {
    pub fn new() -> Self {
        Self
    }

    /// Cost of a candidate residual together with the stats of its derivation.
    pub fn evaluate(&self, residual: &MirFunction, tree: &ProcessTree) -> MrscCostVector {
        self.evaluate_function(residual, &tree.stats)
    }

    /// Cost of any MIR function. `stats` is accepted for API compatibility; the dynamic
    /// estimate comes from the residual CFG itself so baseline and residual share a yardstick.
    pub fn evaluate_function(
        &self,
        residual: &MirFunction,
        _stats: &super::drive::SupercompilerStats,
    ) -> MrscCostVector {
        let multipliers = estimate_execution_multipliers(residual);
        let mut allocation_count = 0usize;
        let mut call_count = 0usize;
        let mut max_live_registers = 0usize;
        let mut dynamic_steps = 0.0f64;

        for (block_idx, block) in residual.blocks.iter().enumerate() {
            let multiplicity = multipliers
                .get(block_idx)
                .copied()
                .unwrap_or(1.0)
                .min(MAX_LOOP_MULTIPLIER);
            let mut block_locals = std::collections::HashSet::new();

            for stmt in &block.statements {
                let Statement::Assign(dest, rval) = stmt;
                add_place_locals(dest, &mut block_locals);
                dynamic_steps += statement_step_weight(rval) * multiplicity;
                match rval {
                    Rvalue::Alloc(p) => {
                        allocation_count += 1;
                        add_place_locals(p, &mut block_locals);
                    }
                    Rvalue::ClosureAlloc { captured, .. } => {
                        allocation_count += 1;
                        for place in captured {
                            add_place_locals(place, &mut block_locals);
                        }
                    }
                    Rvalue::Thunk { env, .. } => {
                        allocation_count += 1;
                        for local in env {
                            block_locals.insert(local.clone());
                        }
                    }
                    Rvalue::Call(_, args) => {
                        call_count += 1;
                        for arg in args {
                            add_place_locals(arg, &mut block_locals);
                        }
                    }
                    Rvalue::Use(p) | Rvalue::UnaryOp(_, p) | Rvalue::Discriminant(p) => {
                        add_place_locals(p, &mut block_locals);
                    }
                    Rvalue::BinaryOp(_, l, r) => {
                        add_place_locals(l, &mut block_locals);
                        add_place_locals(r, &mut block_locals);
                    }
                    Rvalue::Select(c, t, e) => {
                        add_place_locals(c, &mut block_locals);
                        add_place_locals(t, &mut block_locals);
                        add_place_locals(e, &mut block_locals);
                    }
                    Rvalue::Load(p) => add_place_locals(p, &mut block_locals),
                    Rvalue::Array(places) => {
                        for place in places {
                            add_place_locals(place, &mut block_locals);
                        }
                    }
                    Rvalue::Struct(_, fields) => {
                        for (_, place) in fields {
                            add_place_locals(place, &mut block_locals);
                        }
                    }
                    Rvalue::EnumVariant { fields, .. } => {
                        for field in fields {
                            add_place_locals(field, &mut block_locals);
                        }
                    }
                    Rvalue::Phi(entries) => {
                        for (_, place) in entries {
                            add_place_locals(place, &mut block_locals);
                        }
                    }
                    Rvalue::Constant(_) | Rvalue::FnPtr(_) => {}
                }
            }

            dynamic_steps += terminator_step_weight(&block.terminator) * multiplicity;
            match &block.terminator {
                Terminator::IndirectCall { callee, args, .. } => {
                    call_count += 1;
                    add_place_locals(callee, &mut block_locals);
                    for arg in args {
                        add_place_locals(arg, &mut block_locals);
                    }
                }
                Terminator::Force { .. } => call_count += 1,
                Terminator::BranchIf { condition, .. } => {
                    add_place_locals(condition, &mut block_locals);
                }
                Terminator::Switch { value, .. } => add_place_locals(value, &mut block_locals),
                Terminator::TypeGuard { local, .. } => add_place_locals(local, &mut block_locals),
                Terminator::Return { value: Some(value) } => {
                    add_place_locals(value, &mut block_locals);
                }
                _ => {}
            }

            max_live_registers = max_live_registers.max(block_locals.len());
        }

        MrscCostVector {
            dynamic_steps,
            allocation_count,
            call_count,
            residual_blocks: residual.blocks.len(),
            register_pressure: max_live_registers.max(1),
        }
    }

    /// Scalar fitness under an objective (lower is better).
    pub fn score(&self, cost: &MrscCostVector, objective: MrscObjective) -> f64 {
        self.score_with_weights(cost, &CostWeights::for_objective(objective))
    }

    /// Scalar fitness under explicit weights (useful for calibration/tests).
    pub fn score_with_weights(&self, cost: &MrscCostVector, weights: &CostWeights) -> f64 {
        weights.dot(cost)
    }

    /// The hard fallback decision. No candidate, non-finite cost, regression, or an
    /// improvement smaller than the contract margin all force baseline rollback.
    pub fn decide(
        &self,
        baseline: &MrscCostVector,
        candidate: Option<&MrscCostVector>,
        contract: MrscProfitabilityContract,
    ) -> MrscSelectionDecision {
        let weights = CostWeights::for_objective(contract.objective);
        let baseline_score = weights.dot(baseline).max(0.0);
        let candidate_score = candidate.map_or(f64::INFINITY, |cost| weights.dot(cost));
        let margin = contract.min_improvement.max(0.0);
        let required_score = baseline_score * (1.0 - margin);

        let (accepted, reason) = match candidate {
            None => (false, MrscFallbackReason::NoCandidate),
            Some(_) if !baseline_score.is_finite() || !candidate_score.is_finite() => {
                (false, MrscFallbackReason::NonFiniteCost)
            }
            Some(_) if candidate_score >= baseline_score => {
                (false, MrscFallbackReason::CostRegression)
            }
            Some(_) if candidate_score >= required_score => {
                (false, MrscFallbackReason::ImprovementBelowMargin)
            }
            Some(_) => (true, MrscFallbackReason::AcceptedImprovement),
        };

        MrscSelectionDecision {
            accepted,
            fallback: !accepted,
            reason,
            objective: contract.objective,
            min_improvement: margin,
            baseline: *baseline,
            candidate: candidate.copied(),
            baseline_score,
            candidate_score,
            candidate_objective_score: candidate_score,
            candidate_name: None,
        }
    }
}

/// Collects a place and all nested projection locals for register-pressure estimation.
fn add_place_locals(place: &Place, locals: &mut std::collections::HashSet<String>) {
    locals.insert(place.local.clone());
    for projection in &place.projections {
        if let crate::mir::Projection::Index(index) = projection {
            add_place_locals(index, locals);
        }
    }
}

/// Simple-instruction equivalents for one rvalue (distill calibration).
fn statement_step_weight(rval: &Rvalue) -> f64 {
    match rval {
        Rvalue::Load(_) => 1.0 + 5.0,
        Rvalue::EnumVariant { .. } => 1.0 + 10.0,
        _ => 1.0,
    }
}

fn terminator_step_weight(terminator: &Terminator) -> f64 {
    match terminator {
        Terminator::BranchIf { .. } => BRANCH_STEP_COST,
        Terminator::Switch { targets, .. } => (targets.len() as f64) * SWITCH_TARGET_STEP_COST,
        Terminator::Unreachable => 0.0,
        _ => 1.0,
    }
}

/// Detects natural loops and assigns a per-block execution multiplicity.
fn estimate_execution_multipliers(func: &MirFunction) -> Vec<f64> {
    let block_count = func.blocks.len();
    let mut multipliers = vec![1.0; block_count];
    if block_count == 0 {
        return multipliers;
    }

    let block_index: HashMap<BasicBlockId, usize> = func
        .blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.id.clone(), idx))
        .collect();
    let successors: Vec<Vec<usize>> = func
        .blocks
        .iter()
        .map(|block| {
            block
                .terminator
                .successors()
                .iter()
                .filter_map(|id| block_index.get(id).copied())
                .collect()
        })
        .collect();

    // Iterative DFS: a back edge points to a header still on the DFS stack.
    let mut color = vec![0u8; block_count];
    let mut back_edges = Vec::new();
    color[0] = 1;
    let mut stack: Vec<(usize, usize)> = vec![(0, 0)];
    while let Some(&(node, next)) = stack.last() {
        if next < successors[node].len() {
            let target = successors[node][next];
            if let Some(top) = stack.last_mut() {
                top.1 += 1;
            }
            match color[target] {
                1 => back_edges.push((target, node)),
                0 => {
                    color[target] = 1;
                    stack.push((target, 0));
                }
                _ => {}
            }
        } else {
            color[node] = 2;
            stack.pop();
        }
    }

    for (header, tail) in back_edges {
        let trip_count = estimate_loop_trip_count(func, header)
            .clamp(1.0, MAX_LOOP_TRIP_COUNT)
            .min(MAX_LOOP_MULTIPLIER);
        let mut visited = vec![false; block_count];
        let mut work = vec![tail];
        visited[tail] = true;
        while let Some(node) = work.pop() {
            multipliers[node] = (multipliers[node] * trip_count).min(MAX_LOOP_MULTIPLIER);
            if node == header {
                continue;
            }
            for predecessor in 0..block_count {
                if !visited[predecessor] && successors[predecessor].contains(&node) {
                    visited[predecessor] = true;
                    work.push(predecessor);
                }
            }
        }
    }

    multipliers
}

/// Reads a loop bound from the branch-condition defining statement when it compares
/// against a compile-time integer constant.
fn estimate_loop_trip_count(func: &MirFunction, header: usize) -> f64 {
    let condition_local = match &func.blocks[header].terminator {
        Terminator::BranchIf { condition, .. } => condition.local.clone(),
        _ => return DEFAULT_LOOP_TRIP_COUNT,
    };

    for block in &func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(dest, Rvalue::BinaryOp(op, lhs, rhs)) = stmt else {
                continue;
            };
            if dest.local != condition_local {
                continue;
            }
            for (candidate, constant_on_left) in [(lhs, false), (rhs, true)] {
                let Some(bound) = constant_value(func, candidate) else {
                    continue;
                };
                let increasing = matches!(
                    op,
                    crate::ast::BinaryOp::Lt | crate::ast::BinaryOp::Le
                );
                let decreasing = matches!(
                    op,
                    crate::ast::BinaryOp::Gt | crate::ast::BinaryOp::Ge
                );
                if (increasing && !constant_on_left) || (decreasing && constant_on_left) {
                    return (bound.max(1) as f64).min(MAX_LOOP_TRIP_COUNT);
                }
            }
        }
    }
    DEFAULT_LOOP_TRIP_COUNT
}

fn constant_value(func: &MirFunction, place: &Place) -> Option<i64> {
    if !place.projections.is_empty() {
        return None;
    }
    for block in &func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(dest, Rvalue::Constant(value)) = stmt else {
                continue;
            };
            if dest.local == place.local {
                if let crate::typecheck::typed_ast::TypedLiteral::Int(value, _) = value {
                    return Some(*value);
                }
                return None;
            }
        }
    }
    None
}
