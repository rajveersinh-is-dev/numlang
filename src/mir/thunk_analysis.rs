//! Demand-propagation analysis for lazy thunks.
//!
//! Computes which thunks are demanded on execution paths through a MIR function:
//! - `Bottom`: thunk is never forced on any reachable path.
//! - `GuardDemanded`: thunk is forced on some paths (e.g. inside a conditional or switch branch).
//! - `FullyDemanded`: thunk is unconditionally forced on every execution path to exit.
//!
//! In addition, [`compute_stream_thunk_demands`] is a loop-aware analysis for
//! stream-style producer/consumer pipelines (the `stream_take` pattern): a thunk
//! that is forced inside the consumer loop of a codata chain is classified as
//! `FullyDemanded`, because the pipeline pulls on it every iteration. This lets
//! the stream-fusion passes collapse the chain; thunks not forced in loops keep
//! their path-sensitive demand, so we never force more than demanded.

use crate::mir::lower::{MirFunction, Rvalue, Statement};
use crate::mir::{compute_cfg, BasicBlockId, Terminator};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Demand {
    /// Thunk is never forced on any path through this function.
    Bottom,
    /// Thunk is forced only on some paths (e.g. inside a conditional).
    GuardDemanded,
    /// Thunk is forced on every execution path (unconditionally demanded).
    FullyDemanded,
}

impl Demand {
    pub fn join(self, other: Demand) -> Demand {
        match (self, other) {
            (Demand::Bottom, d) | (d, Demand::Bottom) => d,
            (Demand::FullyDemanded, Demand::FullyDemanded) => Demand::FullyDemanded,
            _ => Demand::GuardDemanded,
        }
    }

    pub fn branch_meet(self, other: Demand) -> Demand {
        match (self, other) {
            (Demand::FullyDemanded, Demand::FullyDemanded) => Demand::FullyDemanded,
            (Demand::Bottom, Demand::Bottom) => Demand::Bottom,
            _ => Demand::GuardDemanded,
        }
    }
}

/// Computes thunk demand classification for all thunks in a MIR function.
pub fn compute_thunk_demands(func: &MirFunction) -> HashMap<String, Demand> {
    if func.blocks.is_empty() {
        return HashMap::new();
    }

    let mut candidate_thunks: HashSet<String> = HashSet::new();
    for block in &func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(dest, rval) = stmt;
            if let Rvalue::Thunk { .. } = rval {
                candidate_thunks.insert(dest.local.clone());
            }
        }
        if let Terminator::Force { thunk, .. } = &block.terminator {
            candidate_thunks.insert(thunk.clone());
        }
    }

    if candidate_thunks.is_empty() {
        return HashMap::new();
    }

    let mut in_demands: HashMap<BasicBlockId, HashMap<String, Demand>> = HashMap::new();
    let mut out_demands: HashMap<BasicBlockId, HashMap<String, Demand>> = HashMap::new();

    for block in &func.blocks {
        let mut initial = HashMap::new();
        for t in &candidate_thunks {
            initial.insert(t.clone(), Demand::Bottom);
        }
        in_demands.insert(block.id.clone(), initial.clone());
        out_demands.insert(block.id.clone(), initial);
    }

    let mut changed = true;
    let mut iterations = 0;
    let max_iterations = func.blocks.len() * candidate_thunks.len() * 3 + 16;

    while changed && iterations < max_iterations {
        changed = false;
        iterations += 1;

        for block in func.blocks.iter().rev() {
            // Compute OUT[b] from successors' IN
            let mut new_out: HashMap<String, Demand> = HashMap::new();
            for t in &candidate_thunks {
                new_out.insert(t.clone(), Demand::Bottom);
            }

            match &block.terminator {
                Terminator::Return { .. } | Terminator::Unreachable => {
                    // Stays Bottom
                }
                Terminator::Branch { target } => {
                    if let Some(target_in) = in_demands.get(target) {
                        for (t, d) in target_in {
                            new_out.insert(t.clone(), *d);
                        }
                    }
                }
                Terminator::Force { cont, .. } => {
                    if let Some(cont_in) = in_demands.get(cont) {
                        for (t, d) in cont_in {
                            new_out.insert(t.clone(), *d);
                        }
                    }
                }
                Terminator::BranchIf {
                    then_target,
                    else_target,
                    ..
                } => {
                    let default_in = HashMap::new();
                    let then_in = in_demands.get(then_target).unwrap_or(&default_in);
                    let else_in = in_demands.get(else_target).unwrap_or(&default_in);

                    for t in &candidate_thunks {
                        let d_then = then_in.get(t).copied().unwrap_or(Demand::Bottom);
                        let d_else = else_in.get(t).copied().unwrap_or(Demand::Bottom);
                        new_out.insert(t.clone(), d_then.branch_meet(d_else));
                    }
                }
                Terminator::Switch {
                    targets, default, ..
                } => {
                    let default_in = HashMap::new();
                    let def_in = in_demands.get(default).unwrap_or(&default_in);

                    for t in &candidate_thunks {
                        let mut all_demands = Vec::with_capacity(targets.len() + 1);
                        all_demands.push(def_in.get(t).copied().unwrap_or(Demand::Bottom));
                        for (_, target_bb) in targets {
                            let t_in = in_demands.get(target_bb).unwrap_or(&default_in);
                            all_demands.push(t_in.get(t).copied().unwrap_or(Demand::Bottom));
                        }

                        let combined = if all_demands.iter().all(|&d| d == Demand::FullyDemanded) {
                            Demand::FullyDemanded
                        } else if all_demands.iter().any(|&d| d != Demand::Bottom) {
                            Demand::GuardDemanded
                        } else {
                            Demand::Bottom
                        };
                        new_out.insert(t.clone(), combined);
                    }
                }
                Terminator::IndirectCall { next, .. } => {
                    if let Some(next_in) = in_demands.get(next) {
                        for (t, d) in next_in {
                            new_out.insert(t.clone(), *d);
                        }
                    }
                }
                Terminator::Fork { left, right, .. } => {
                    let default_in = HashMap::new();
                    let l_in = in_demands.get(left).unwrap_or(&default_in);
                    let r_in = in_demands.get(right).unwrap_or(&default_in);
                    for t in &candidate_thunks {
                        let dl = l_in.get(t).copied().unwrap_or(Demand::Bottom);
                        let dr = r_in.get(t).copied().unwrap_or(Demand::Bottom);
                        new_out.insert(t.clone(), dl.join(dr));
                    }
                }
                Terminator::TypeGuard {
                    fast_path,
                    deopt_stub,
                    ..
                } => {
                    let default_in = HashMap::new();
                    let fast_in = in_demands.get(fast_path).unwrap_or(&default_in);
                    let deopt_in = in_demands.get(deopt_stub).unwrap_or(&default_in);
                    for t in &candidate_thunks {
                        let d_fast = fast_in.get(t).copied().unwrap_or(Demand::Bottom);
                        let d_deopt = deopt_in.get(t).copied().unwrap_or(Demand::Bottom);
                        new_out.insert(t.clone(), d_fast.branch_meet(d_deopt));
                    }
                }
            }

            // Transfer function: IN[b] = OUT[b] updated by block body and terminator
            let mut new_in = new_out.clone();
            if let Terminator::Force { thunk, .. } = &block.terminator {
                new_in.insert(thunk.clone(), Demand::FullyDemanded);
            }

            let Some(curr_in) = in_demands.get_mut(&block.id) else {
                continue;
            };
            if *curr_in != new_in {
                *curr_in = new_in;
                changed = true;
            }
            out_demands.insert(block.id.clone(), new_out);
        }
    }

    let entry_id = &func.blocks[0].id;
    let entry_in = in_demands.get(entry_id).cloned().unwrap_or_default();

    let mut result = HashMap::new();
    for t in &candidate_thunks {
        let d = entry_in.get(t).copied().unwrap_or(Demand::Bottom);
        result.insert(t.clone(), d);
    }

    result
}

/// Returns the set of blocks that lie on a cycle (can reach themselves via at least one edge).
fn blocks_in_cycles(func: &MirFunction) -> HashSet<BasicBlockId> {
    let (_preds, succs) = compute_cfg(&func.blocks);
    let mut in_cycle = HashSet::new();
    for block in &func.blocks {
        let start = block.id.clone();
        let mut seen: HashSet<BasicBlockId> = HashSet::new();
        let mut work: Vec<BasicBlockId> = succs.get(&start).cloned().unwrap_or_default();
        while let Some(b) = work.pop() {
            if b == start {
                in_cycle.insert(start.clone());
                break;
            }
            if seen.insert(b.clone()) {
                if let Some(s) = succs.get(&b) {
                    work.extend(s.iter().cloned());
                }
            }
        }
    }
    in_cycle
}

/// Collects the blocks that force each thunk (by local name).
fn force_sites(func: &MirFunction) -> HashMap<String, Vec<BasicBlockId>> {
    let mut sites: HashMap<String, Vec<BasicBlockId>> = HashMap::new();
    for block in &func.blocks {
        if let Terminator::Force { thunk, .. } = &block.terminator {
            sites.entry(thunk.clone()).or_default().push(block.id.clone());
        }
    }
    sites
}

/// Computes thunk demand for stream-style producer/consumer pipelines.
///
/// Unlike [`compute_thunk_demands`] (a path-sensitive analysis), this analysis is
/// loop-aware: a thunk that is forced inside the consumer loop of a codata chain
/// (e.g. the `stream_take` pattern, where an internal branch sits inside the loop)
/// is classified `FullyDemanded`, because the pipeline pulls on the thunk on every
/// iteration. This is what lets the stream-fusion passes collapse the chain.
/// Thunks that are not forced within any loop keep their
/// [`compute_thunk_demands`] classification, so we never force more than demanded
/// and laziness semantics are preserved.
pub fn compute_stream_thunk_demands(func: &MirFunction) -> HashMap<String, Demand> {
    let base = compute_thunk_demands(func);
    if func.blocks.is_empty() {
        return base;
    }
    let in_cycle = blocks_in_cycles(func);
    let sites = force_sites(func);
    let mut result = base;
    for (thunk, blocks) in sites {
        // Stream pipeline: every force site of this thunk is inside a loop.
        let all_in_loops = !blocks.is_empty() && blocks.iter().all(|b| in_cycle.contains(b));
        if all_in_loops {
            result.insert(thunk, Demand::FullyDemanded);
        }
    }
    result
}
