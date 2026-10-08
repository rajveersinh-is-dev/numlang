//! Multi-Result Supercompilation (MRSC - Mitchell & Klyuchnikov 2012).
//!
//! Replaces greedy whistle decisions with an explicit configuration hypergraph
//! exploration over alternative supercompilation choices (unfolding, knot-tying,
//! loop unrolling, recurrence solving, and distillation). Candidate residual
//! programs are evaluated along multiple competing objective dimensions, forming
//! a Pareto frontier from which the optimal residual program is extracted.

use std::collections::HashMap;

use super::drive::{DriverConfig, ProcessEdge, ProcessNodeId, ProcessTree, SupercompilerDriver};
use super::residualize::residualize_process_tree;
use super::term::SymTermId;
use crate::mir::lower::MirFunction;
use crate::mir::{BasicBlockId, Terminator};

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
}

impl ParetoFrontier {
    pub fn new() -> Self {
        ParetoFrontier {
            candidates: Vec::new(),
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
            if score < best_score {
                best_score = score;
                best = cand;
            }
        }

        Some((best, best_score))
    }
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
}

impl<'a> MultiResultEngine<'a> {
    pub fn new(func: &'a MirFunction, program_funcs: &'a [MirFunction]) -> Self {
        MultiResultEngine {
            func,
            program_funcs,
        }
    }

    /// Systematically explores the configuration hypergraph across multiple supercompilation
    /// strategies (unrolling, knot-tying, recurrence collapse, distillation, conservative baseline)
    /// and populates the Pareto frontier.
    pub fn explore_hypergraph(&self) -> (ConfigurationHypergraph, ParetoFrontier) {
        let entry_block = if !self.func.blocks.is_empty() {
            self.func.blocks[0].id.clone()
        } else {
            BasicBlockId(0)
        };

        let mut hypergraph = ConfigurationHypergraph::new(entry_block.clone());
        let mut frontier = ParetoFrontier::new();

        // Node mapping across derivations: (block_id, depth) -> HyperNodeId
        let mut node_map: HashMap<(BasicBlockId, usize), HyperNodeId> = HashMap::new();
        node_map.insert((entry_block, 0), hypergraph.root);

        // --------------------------------------------------------------------
        // Derivation 1: Standard Supercompilation (Knot-Tying + Recurrence)
        // --------------------------------------------------------------------
        let tree_std = SupercompilerDriver::new(self.func)
            .with_program_functions(self.program_funcs)
            .with_config(DriverConfig {
                max_depth: 128,
                max_unroll_depth: 0,
                solve_recurrences: true,
                inline_calls: true,
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
                solve_recurrences: true,
                inline_calls: true,
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
                inline_calls: true,
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
            .run();

        let metrics_baseline = CandidateMetrics::new(self.func, &tree_id);
        hypergraph.add_edge(root_id, HyperAction::IdentityBaseline, vec![root_id]);

        frontier.insert(ParetoCandidate {
            name: "Identity_Baseline".to_string(),
            residual: self.func.clone(),
            tree: tree_id,
            metrics: metrics_baseline,
        });

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
    pub fn explore_and_select<O: ResidualObjective>(
        &self,
        objective: &O,
    ) -> (MirFunction, ProcessTree, f64) {
        let (_hypergraph, frontier) = self.explore_hypergraph();

        if let Some((candidate, score)) = frontier.select_optimal(objective) {
            (candidate.residual.clone(), candidate.tree.clone(), score)
        } else {
            let tree = SupercompilerDriver::new(self.func)
                .with_program_functions(self.program_funcs)
                .run();
            let score = objective.score(&tree, self.func);
            (self.func.clone(), tree, score)
        }
    }
}

// ============================================================================
// 5. 4-Dimensional MRSC Cost Model & Objective (Phase 53)
// ============================================================================

/// 4-dimensional cost vector for candidate residual programs:
/// 1. Dynamic steps estimated from symbolic evaluation and unrolling
/// 2. Heap allocation count
/// 3. Residual basic block count
/// 4. Live register pressure
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MrscCostVector {
    /// Dynamic step count estimated from symbolic unrolling and loops.
    pub dynamic_steps: f64,
    /// Count of heap allocation operations in the residual code.
    pub allocation_count: usize,
    /// Number of basic blocks in the residual MIR CFG.
    pub residual_blocks: usize,
    /// Maximum live register pressure in any basic block.
    pub register_pressure: usize,
}

impl MrscCostVector {
    /// Checks Pareto dominance: self dominates other iff self is no worse on all
    /// 4 dimensions and strictly better on at least one dimension.
    pub fn dominates(&self, other: &MrscCostVector) -> bool {
        let no_worse = self.dynamic_steps <= other.dynamic_steps
            && self.allocation_count <= other.allocation_count
            && self.residual_blocks <= other.residual_blocks
            && self.register_pressure <= other.register_pressure;
        let strictly_better = self.dynamic_steps < other.dynamic_steps
            || self.allocation_count < other.allocation_count
            || self.residual_blocks < other.residual_blocks
            || self.register_pressure < other.register_pressure;
        no_worse && strictly_better
    }
}

/// Optimization objectives for selecting from the Pareto frontier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum MrscObjective {
    /// Prioritizes minimizing dynamic steps and memory allocations.
    #[default]
    Speed,
    /// Prioritizes minimizing code size (basic blocks) and register pressure.
    Size,
    /// Harmonious weighted balance across all 4 dimensions.
    Balanced,
}

/// 4-dimensional cost model evaluating candidate residual programs.
#[derive(Debug, Clone, Default)]
pub struct MrscCostModel;

impl MrscCostModel {
    pub fn new() -> Self {
        Self
    }

    /// Compute the 4-dimensional cost vector for a candidate residual and its process tree.
    pub fn evaluate(&self, residual: &MirFunction, tree: &ProcessTree) -> MrscCostVector {
        let mut allocation_count = 0;
        let mut max_live_registers = 0;

        for block in &residual.blocks {
            let mut block_locals = std::collections::HashSet::new();
            for stmt in &block.statements {
                let crate::mir::lower::Statement::Assign(dest, rval) = stmt;
                block_locals.insert(dest.local.clone());
                match rval {
                    crate::mir::lower::Rvalue::Alloc(_)
                    | crate::mir::lower::Rvalue::ClosureAlloc { .. }
                    | crate::mir::lower::Rvalue::Thunk { .. } => {
                        allocation_count += 1;
                    }
                    crate::mir::lower::Rvalue::Use(p)
                    | crate::mir::lower::Rvalue::UnaryOp(_, p)
                    | crate::mir::lower::Rvalue::Discriminant(p) => {
                        block_locals.insert(p.local.clone());
                    }
                    crate::mir::lower::Rvalue::BinaryOp(_, l, r) => {
                        block_locals.insert(l.local.clone());
                        block_locals.insert(r.local.clone());
                    }
                    crate::mir::lower::Rvalue::Call(_, args) => {
                        for a in args {
                            block_locals.insert(a.local.clone());
                        }
                    }
                    crate::mir::lower::Rvalue::EnumVariant { fields, .. } => {
                        for f in fields {
                            block_locals.insert(f.local.clone());
                        }
                    }
                    _ => {}
                }
            }
            if block_locals.len() > max_live_registers {
                max_live_registers = block_locals.len();
            }
        }

        let residual_blocks = residual.blocks.len();
        let mut dynamic_steps = residual
            .blocks
            .iter()
            .map(|b| b.statements.len())
            .sum::<usize>() as f64;
        for block in &residual.blocks {
            match &block.terminator {
                Terminator::BranchIf { .. } => dynamic_steps += 10.0,
                Terminator::Switch { targets, .. } => dynamic_steps += (targets.len() * 5) as f64,
                _ => {}
            }
        }
        dynamic_steps += (tree.stats.knots_tied * 10) as f64;
        if tree.stats.loops_collapsed > 0 {
            dynamic_steps = 1.0;
        }

        MrscCostVector {
            dynamic_steps,
            allocation_count,
            residual_blocks,
            register_pressure: max_live_registers.max(1),
        }
    }

    /// Calculate scalar fitness score for an objective (lower score is better).
    pub fn score(&self, cost: &MrscCostVector, objective: MrscObjective) -> f64 {
        match objective {
            MrscObjective::Speed => {
                cost.dynamic_steps * 10.0
                    + (cost.allocation_count as f64) * 50.0
                    + (cost.residual_blocks as f64) * 1.0
                    + (cost.register_pressure as f64) * 0.5
            }
            MrscObjective::Size => {
                (cost.residual_blocks as f64) * 20.0
                    + (cost.register_pressure as f64) * 10.0
                    + (cost.allocation_count as f64) * 5.0
                    + cost.dynamic_steps * 0.5
            }
            MrscObjective::Balanced => {
                cost.dynamic_steps * 2.0
                    + (cost.allocation_count as f64) * 15.0
                    + (cost.residual_blocks as f64) * 5.0
                    + (cost.register_pressure as f64) * 2.0
            }
        }
    }
}
