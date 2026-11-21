//! Exhaustive Multi-Result Supercompilation (MRSC) Oracle with IDDFS & Pareto Cost Model.
//!
//! Replaces shallow, fixed-depth supercompilation with an unbounded Iterative Deepening
//! Depth-First Search (IDDFS) over the configuration hypergraph (`--mrsc-exhaustive`).
//! Evaluates candidates with the calibrated 5-dimensional cost model (dynamic instructions,
//! allocations, calls, residual blocks, register pressure), extracts the Pareto frontier,
//! applies the "never slower than baseline" gate, and commits accepted winners to the L2
//! persistent specialization disk cache.

use super::cache::{sha256_str, CacheKey, CachedSpecialization, SpecializationCache};
use super::distill::DistillationEngine;
use super::drive::{DriverConfig, ProcessTree, SupercompilerDriver};
use super::mrsc::{
    MrscCostModel, MrscCostVector, MrscFallbackReason, MrscObjective, MrscProfitabilityContract,
    MrscSelectionDecision,
};
use super::residualize::residualize_process_tree;
use crate::mir::lower::MirFunction;
use std::path::Path;
use std::sync::Mutex;

/// Configuration for the MRSC Iterative Deepening Depth-First Search (IDDFS) Oracle.
#[derive(Debug, Clone)]
pub struct IddfsOracleConfig {
    /// Minimum search depth bound (default: 2).
    pub min_depth: usize,
    /// Maximum search depth bound (depth >= 20 for exhaustive exploration, default: 24).
    pub max_depth: usize,
    /// Depth increment step per deepening iteration (default: 2).
    pub step_depth: usize,
    /// Objective function for selecting from the Pareto frontier.
    pub objective: MrscObjective,
    /// Whether exhaustive search mode is enabled.
    pub exhaustive: bool,
}

impl Default for IddfsOracleConfig {
    fn default() -> Self {
        Self {
            min_depth: 2,
            max_depth: 24,
            step_depth: 2,
            objective: MrscObjective::Balanced,
            exhaustive: true,
        }
    }
}

/// A candidate residual program discovered during IDDFS exploration.
#[derive(Debug, Clone)]
pub struct OracleCandidate {
    pub depth: usize,
    pub residual: MirFunction,
    pub tree: ProcessTree,
    pub cost: MrscCostVector,
    pub fitness_score: f64,
}

/// Pareto frontier over the 5-dimensional cost model.
#[derive(Debug, Clone, Default)]
pub struct OracleParetoFrontier {
    pub candidates: Vec<OracleCandidate>,
    pub max_depth_evaluated: usize,
    pub total_evaluated: usize,
    /// Telemetry for the last gated selection.
    pub selection: Option<MrscSelectionDecision>,
}

impl OracleParetoFrontier {
    pub fn new() -> Self {
        Self {
            candidates: Vec::new(),
            max_depth_evaluated: 0,
            total_evaluated: 0,
            selection: None,
        }
    }

    /// Inserts a candidate into the 5D Pareto frontier.
    /// Rejects the candidate if dominated by any existing member.
    /// Removes any existing members dominated by the new candidate.
    pub fn insert(&mut self, candidate: OracleCandidate) -> bool {
        if self
            .candidates
            .iter()
            .any(|c| c.cost.dominates(&candidate.cost))
        {
            return false;
        }
        self.candidates
            .retain(|c| !candidate.cost.dominates(&c.cost));
        self.candidates.push(candidate);
        true
    }

    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Selects the optimal candidate according to the specified objective. Ties are broken
    /// deterministically by the calibrated cost vector, depth and residual footprint.
    pub fn select_optimal(&self, objective: MrscObjective) -> Option<&OracleCandidate> {
        let cost_model = MrscCostModel::new();
        self.candidates.iter().min_by(|a, b| {
            let score_a = cost_model.score(&a.cost, objective);
            let score_b = cost_model.score(&b.cost, objective);
            score_a
                .total_cmp(&score_b)
                .then_with(|| a.cost.compare_for_selection(&b.cost))
                .then_with(|| a.depth.cmp(&b.depth))
                .then_with(|| a.residual.name.cmp(&b.residual.name))
        })
    }
}

/// Exhaustive MRSC IDDFS Oracle Engine.
pub struct MrscOracleEngine<'a> {
    func: &'a MirFunction,
    program_funcs: &'a [MirFunction],
    config: IddfsOracleConfig,
    cost_model: MrscCostModel,
    contract: MrscProfitabilityContract,
    last_decision: Mutex<Option<MrscSelectionDecision>>,
}

impl<'a> MrscOracleEngine<'a> {
    pub fn new(
        func: &'a MirFunction,
        program_funcs: &'a [MirFunction],
        config: IddfsOracleConfig,
    ) -> Self {
        let contract = MrscProfitabilityContract::new(config.objective, 0.05);
        Self {
            func,
            program_funcs,
            config,
            cost_model: MrscCostModel::new(),
            contract,
            last_decision: Mutex::new(None),
        }
    }

    /// Creates an oracle with an explicit "never slower than baseline" contract.
    pub fn new_with_contract(
        func: &'a MirFunction,
        program_funcs: &'a [MirFunction],
        config: IddfsOracleConfig,
        contract: MrscProfitabilityContract,
    ) -> Self {
        Self {
            contract,
            ..Self::new(func, program_funcs, config)
        }
    }

    /// Overrides the profitability contract (used by deterministic fallback tests).
    pub fn with_contract(mut self, contract: MrscProfitabilityContract) -> Self {
        self.contract = contract;
        self
    }

    /// Telemetry: decision from the most recent `explore_iddfs` / `run_with_cache` call.
    pub fn last_decision(&self) -> Option<MrscSelectionDecision> {
        self.last_decision
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or(None)
    }

    fn record_decision(&self, frontier: &mut OracleParetoFrontier, decision: MrscSelectionDecision) {
        frontier.selection = Some(decision.clone());
        if let Ok(mut guard) = self.last_decision.lock() {
            *guard = Some(decision);
        }
    }

    /// Run Iterative Deepening Depth-First Search (IDDFS) exploring deepening process trees
    /// from `min_depth` to `max_depth`. The returned candidate is always gated: if no
    /// candidate clears the contract margin, the original baseline MIR/cost is returned.
    pub fn explore_iddfs(&self) -> (OracleParetoFrontier, OracleCandidate) {
        let mut frontier = OracleParetoFrontier::new();

        // Baseline is always available as a deterministic fallback.
        let baseline_tree = SupercompilerDriver::new(self.func)
            .with_program_functions(self.program_funcs)
            .run();
        let baseline_cost = self.cost_model.evaluate(self.func, &baseline_tree);
        let baseline_score = self.cost_model.score(&baseline_cost, self.contract.objective);
        let baseline = OracleCandidate {
            depth: 0,
            residual: self.func.clone(),
            tree: baseline_tree,
            cost: baseline_cost,
            fitness_score: baseline_score,
        };
        frontier.insert(baseline.clone());

        let mut current_depth = self.config.min_depth;
        while current_depth <= self.config.max_depth {
            frontier.max_depth_evaluated = frontier.max_depth_evaluated.max(current_depth);
            frontier.total_evaluated += 3;

            // Strategy 1: Unrolling up to current_depth with recurrence solving.
            let unroll_d = (current_depth / 2).max(1);
            let tree_unroll = SupercompilerDriver::new(self.func)
                .with_program_functions(self.program_funcs)
                .with_config(DriverConfig {
                    max_depth: current_depth.max(20),
                    max_unroll_depth: unroll_d,
                    solve_recurrences: true,
                    inline_calls: true,
                    ..DriverConfig::default()
                })
                .run();
            let res_unroll = residualize_process_tree(&tree_unroll, self.func);
            let cost_unroll = self.cost_model.evaluate(&res_unroll, &tree_unroll);
            let score_unroll = self.cost_model.score(&cost_unroll, self.contract.objective);
            frontier.insert(OracleCandidate {
                depth: current_depth,
                residual: res_unroll,
                tree: tree_unroll,
                cost: cost_unroll,
                fitness_score: score_unroll,
            });

            // Strategy 2: Deep symbolic specialization with knot-tying and recurrence.
            let tree_deep = SupercompilerDriver::new(self.func)
                .with_program_functions(self.program_funcs)
                .with_config(DriverConfig {
                    max_depth: current_depth,
                    max_unroll_depth: 0,
                    solve_recurrences: true,
                    inline_calls: true,
                    ..DriverConfig::default()
                })
                .run();
            let res_deep = residualize_process_tree(&tree_deep, self.func);
            let cost_deep = self.cost_model.evaluate(&res_deep, &tree_deep);
            let score_deep = self.cost_model.score(&cost_deep, self.contract.objective);
            frontier.insert(OracleCandidate {
                depth: current_depth,
                residual: res_deep,
                tree: tree_deep.clone(),
                cost: cost_deep,
                fitness_score: score_deep,
            });

            // Strategy 3: Distillation pass over the deep process tree.
            let mut tree_distill = tree_deep_clone(&tree_deep);
            let mut interner_clone = tree_distill.interner.clone();
            let mut distill_engine = DistillationEngine::new(self.func, &mut interner_clone);
            distill_engine.distill_process_tree(&mut tree_distill);
            let res_distill = residualize_process_tree(&tree_distill, self.func);
            let cost_distill = self.cost_model.evaluate(&res_distill, &tree_distill);
            let score_distill = self.cost_model.score(&cost_distill, self.contract.objective);
            frontier.insert(OracleCandidate {
                depth: current_depth,
                residual: res_distill,
                tree: tree_distill,
                cost: cost_distill,
                fitness_score: score_distill,
            });

            current_depth += self.config.step_depth;
        }

        let best = frontier.select_optimal(self.contract.objective).cloned();
        let mut decision = match &best {
            Some(candidate) => {
                let mut decision = self.cost_model.decide(
                    &baseline.cost,
                    Some(&candidate.cost),
                    self.contract,
                );
                decision.candidate_name = Some(format!("IDDFS_depth_{}", candidate.depth));
                decision.candidate_objective_score = candidate.fitness_score;
                decision
            }
            None => {
                let mut decision = self.cost_model.decide(&baseline.cost, None, self.contract);
                decision.candidate_objective_score = baseline_score;
                decision
            }
        };
        // If the frontier winner is the baseline itself, report the explicit reason even when
        // its score equals the baseline score.
        if decision.reason == MrscFallbackReason::AcceptedImprovement
            && best.as_ref().map(|c| c.depth) == Some(0)
        {
            decision = MrscSelectionDecision {
                accepted: false,
                fallback: true,
                reason: MrscFallbackReason::NoCandidate,
                candidate_objective_score: baseline_score,
                ..decision
            };
        }
        self.record_decision(&mut frontier, decision.clone());

        if decision.accepted {
            if let Some(candidate) = best {
                return (frontier, candidate);
            }
        }
        (frontier, baseline)
    }

    /// Run IDDFS oracle with an open cache reference.
    pub fn run_with_cache(
        &self,
        cache: &SpecializationCache,
    ) -> Result<(MirFunction, MrscCostVector, bool), String> {
        let funcs_map: std::collections::HashMap<String, &MirFunction> = self
            .program_funcs
            .iter()
            .map(|f| (f.name.clone(), f))
            .collect();
        let src_hash = if !funcs_map.is_empty() {
            super::cache::compute_composite_hash(&self.func.name, &funcs_map)
        } else {
            sha256_str(&format!("{:#?}", self.func))
        };
        let key = CacheKey::new(
            &self.func.name,
            src_hash,
            format!("mrsc_oracle_{:?}", self.config.objective),
        )
        .with_flags(format!("objective={:?}", self.config.objective));

        let baseline_tree = SupercompilerDriver::new(self.func)
            .with_program_functions(self.program_funcs)
            .run();
        let baseline_cost = self.cost_model.evaluate(self.func, &baseline_tree);

        // Check cache first.
        if let Some(cached) = cache.lookup(&key) {
            if let Ok(residual) = serde_json::from_str::<MirFunction>(&cached.residual_json) {
                let cached_cost = self
                    .cost_model
                    .evaluate_function(&residual, &super::drive::SupercompilerStats::default());
                let mut decision = self.cost_model.decide(
                    &baseline_cost,
                    Some(&cached_cost),
                    self.contract,
                );
                decision.candidate_name = Some("Cached_Specialization".to_string());
                decision.candidate_objective_score =
                    self.cost_model.score(&cached_cost, self.contract.objective);
                let mut telemetry_frontier = OracleParetoFrontier::new();
                self.record_decision(&mut telemetry_frontier, decision.clone());
                if decision.accepted {
                    return Ok((residual, cached_cost, true));
                }
                return Ok((self.func.clone(), baseline_cost, true));
            }
        }

        // Run full IDDFS oracle (internally gated).
        let (_frontier, winner) = self.explore_iddfs();

        // Store into cache.
        let residual_json = serde_json::to_string_pretty(&winner.residual)
            .map_err(|e| format!("Failed to serialize residual: {}", e))?;
        let entry = CachedSpecialization {
            key,
            residual_json,
            stats_nodes_explored: winner.tree.stats.nodes_explored,
            stats_branches_pruned: winner.tree.stats.branches_pruned,
            stats_loops_collapsed: winner.tree.stats.loops_collapsed,
            stats_knots_tied: winner.tree.stats.knots_tied,
            stats_calls_inlined: winner.tree.stats.calls_inlined,
            stats_sc_bce_eliminated: winner.tree.stats.sc_bce_eliminated,
            stats_residual_block_count: winner.residual.blocks.len(),
            stats_residual_stmt_count: winner
                .residual
                .blocks
                .iter()
                .map(|b| b.statements.len())
                .sum(),
        };

        cache
            .store(&entry)
            .map_err(|e| format!("Failed to store in cache: {}", e))?;

        Ok((winner.residual, winner.cost, false))
    }

    /// Run IDDFS oracle and persist the winning residual into the L2 specialization disk cache.
    pub fn run_and_cache(
        &self,
        cache_dir: &Path,
    ) -> Result<(MirFunction, MrscCostVector, bool), String> {
        let cache = SpecializationCache::open(cache_dir);
        self.run_with_cache(&cache)
    }
}

fn tree_deep_clone(tree: &ProcessTree) -> ProcessTree {
    tree.clone()
}
