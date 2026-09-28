//! Phase 35: Post-Distillation Process-Tree & MIR Residual Compaction.
//!
//! Provides two compaction passes:
//! 1. `compact_process_tree`: Prunes dead overflow nodes and deduplicates alpha-equivalent
//!    knot targets prior to residualization.
//! 2. `compact_mir_function`: Peephole optimization pass over residualized MIR functions,
//!    removing no-op identity assignments and performing conservative copy propagation
//!    into return terminators (eta-reduction).

use std::collections::{HashMap, HashSet};

use super::drive::{ProcessEdge, ProcessNodeId, ProcessTree};
use crate::mir::lower::{MirFunction, Rvalue, Statement};
use crate::mir::{Place, Projection, Terminator};

/// Phase 35: Compact a ProcessTree before residualization.
///
/// 1. Dead-node elimination: DFS/BFS from tree.root; mark unreachable nodes
///    with overflow = true so residualize emits Unreachable for them.
/// 2. Knot-target deduplication: find pairs of Knot edges pointing to
///    alpha-equivalent target states (same block id AND identical env keys
///    and term values). Redirect the redundant edge to the canonical target.
/// 3. Returns (dead_eliminated, knots_deduped).
///
/// IMPORTANT: Do NOT physically remove nodes from tree.nodes Vec.
/// Mark dead ones with overflow = true; residualize already handles them.
pub fn compact_process_tree(tree: &mut ProcessTree) -> (usize, usize) {
    if tree.nodes.is_empty() || tree.root.0 >= tree.nodes.len() {
        return (0, 0);
    }

    // Step 1: Knot-target deduplication
    let mut knot_targets: Vec<ProcessNodeId> = Vec::new();
    for node in &tree.nodes {
        for edge in &node.edges {
            if let ProcessEdge::Knot(target) = edge {
                if !knot_targets.contains(target) {
                    knot_targets.push(*target);
                }
            }
        }
    }
    knot_targets.sort_by_key(|id| id.0);

    let mut knots_deduped = 0;
    let mut remap: HashMap<ProcessNodeId, ProcessNodeId> = HashMap::new();

    for i in 0..knot_targets.len() {
        let target_b = knot_targets[i];
        if remap.contains_key(&target_b) {
            continue;
        }
        for &target_a in knot_targets.iter().take(i) {
            let canonical_a = remap.get(&target_a).copied().unwrap_or(target_a);
            if tree.nodes[canonical_a.0].state.block == tree.nodes[target_b.0].state.block
                && tree.nodes[canonical_a.0].state.env == tree.nodes[target_b.0].state.env
            {
                remap.insert(target_b, canonical_a);
                tree.nodes[target_b.0].overflow = true;
                tree.nodes[target_b.0].return_term = None;
                tree.nodes[target_b.0].edges.clear();
                knots_deduped += 1;
                break;
            }
        }
    }

    if !remap.is_empty() {
        for node in &mut tree.nodes {
            for edge in &mut node.edges {
                if let ProcessEdge::Knot(target) = edge {
                    if let Some(&new_target) = remap.get(target) {
                        *edge = ProcessEdge::Knot(new_target);
                    }
                }
            }
        }
    }

    // Step 2: Dead-node elimination (reachability marking from tree.root)
    let mut reachable = HashSet::new();
    let mut stack = vec![tree.root];
    while let Some(node_id) = stack.pop() {
        if !reachable.insert(node_id) {
            continue;
        }
        let node = &tree.nodes[node_id.0];
        if node.overflow {
            // Execution terminates at overflow nodes with Unreachable; children are dead
            continue;
        }
        for edge in &node.edges {
            let target = match edge {
                ProcessEdge::Step(t) | ProcessEdge::Knot(t) => *t,
                ProcessEdge::BranchTrue(t, _) | ProcessEdge::BranchFalse(t, _) => *t,
            };
            stack.push(target);
        }
    }

    let mut dead_eliminated = 0;
    for node in &mut tree.nodes {
        if !reachable.contains(&node.id) {
            if !node.overflow {
                dead_eliminated += 1;
            }
            node.overflow = true;
            node.return_term = None;
            node.edges.clear();
        }
    }

    (dead_eliminated, knots_deduped)
}

/// Phase 35: Peephole-compact a residualized MirFunction.
///
/// 1. No-op assignment removal: remove Statement::Assign(p, Rvalue::Use(q))
///    where p == q (same Place — identity assignment).
/// 2. Trivial copy propagation: if a local is defined exactly once as
///    Rvalue::Use(other_place) and used exactly once immediately as the
///    return value, substitute inline and delete the assignment.
/// 3. Returns the number of statements removed.
pub fn compact_mir_function(func: &mut MirFunction) -> usize {
    let mut total_removed = 0;
    loop {
        let p1 = run_noop_removal_pass(func);
        let p2 = run_copy_propagation_pass(func);
        let iter_removed = p1 + p2;
        total_removed += iter_removed;
        if iter_removed == 0 {
            break;
        }
    }
    total_removed
}

/// Pass 1: Remove identity assignments `p = Use(p)`.
fn run_noop_removal_pass(func: &mut MirFunction) -> usize {
    let mut removed = 0;
    for block in &mut func.blocks {
        let before = block.statements.len();
        block.statements.retain(|stmt| {
            if let Statement::Assign(dest, Rvalue::Use(src)) = stmt {
                return dest != src;
            }
            true
        });
        removed += before - block.statements.len();
    }
    removed
}

/// Pass 2: Trivial copy propagation into return terminators.
fn run_copy_propagation_pass(func: &mut MirFunction) -> usize {
    // Collect definition counts and usages of all locals across the function
    let mut defs: HashMap<String, usize> = HashMap::new();
    let mut uses: HashMap<String, usize> = HashMap::new();

    for (param_name, _) in &func.params {
        *defs.entry(param_name.clone()).or_default() += 1;
    }

    for block in &func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(dest, rval) = stmt;
            *defs.entry(dest.local.clone()).or_default() += 1;
            for proj in &dest.projections {
                if let Projection::Index(idx_place) = proj {
                    record_place_uses(idx_place, &mut uses);
                }
            }
            record_rvalue_uses(rval, &mut uses);
        }
        record_terminator_uses(&block.terminator, &mut uses);
    }

    let mut candidates: Vec<(usize, usize, Place)> = Vec::new(); // (block_idx, stmt_idx, new_ret_place)

    for (b_idx, block) in func.blocks.iter().enumerate() {
        if let Terminator::Return {
            value: Some(ref ret_place),
        } = block.terminator
        {
            if !ret_place.projections.is_empty() {
                continue;
            }
            let ret_local = &ret_place.local;
            if defs.get(ret_local) != Some(&1) || uses.get(ret_local) != Some(&1) {
                continue;
            }

            // Find definition in the same block
            let mut def_stmt_idx = None;
            for (s_idx, stmt) in block.statements.iter().enumerate() {
                if let Statement::Assign(dest, Rvalue::Use(src)) = stmt {
                    if dest.local == *ret_local
                        && dest.projections.is_empty()
                        && src.local != *ret_local
                        && !src.projections.iter().any(|p| matches!(p, Projection::Index(_)))
                    {
                        def_stmt_idx = Some((s_idx, src.clone()));
                        break;
                    }
                }
            }

            if let Some((s_idx, src_place)) = def_stmt_idx {
                // Ensure src.local is not reassigned anywhere between s_idx + 1 and the end of this block
                let mut src_reassigned = false;
                for subsequent_stmt in &block.statements[s_idx + 1..] {
                    let Statement::Assign(d, _) = subsequent_stmt;
                    if d.local == src_place.local {
                        src_reassigned = true;
                        break;
                    }
                }

                if !src_reassigned {
                    candidates.push((b_idx, s_idx, src_place));
                }
            }
        }
    }

    if candidates.is_empty() {
        return 0;
    }

    let mut removed = 0;
    for (b_idx, s_idx, new_ret_place) in candidates {
        let block = &mut func.blocks[b_idx];
        block.terminator = Terminator::Return {
            value: Some(new_ret_place),
        };
        block.statements.remove(s_idx);
        removed += 1;
    }

    removed
}

fn record_place_uses(place: &Place, uses: &mut HashMap<String, usize>) {
    *uses.entry(place.local.clone()).or_default() += 1;
    for proj in &place.projections {
        if let Projection::Index(idx_place) = proj {
            record_place_uses(idx_place, uses);
        }
    }
}

fn record_rvalue_uses(rvalue: &Rvalue, uses: &mut HashMap<String, usize>) {
    match rvalue {
        Rvalue::Use(p)
        | Rvalue::UnaryOp(_, p)
        | Rvalue::Discriminant(p)
        | Rvalue::Alloc(p)
        | Rvalue::Load(p) => {
            record_place_uses(p, uses);
        }
        Rvalue::BinaryOp(_, p1, p2) => {
            record_place_uses(p1, uses);
            record_place_uses(p2, uses);
        }
        Rvalue::Call(_, args)
        | Rvalue::Array(args)
        | Rvalue::ClosureAlloc { captured: args, .. } => {
            for p in args {
                record_place_uses(p, uses);
            }
        }
        Rvalue::Struct(_, fields) => {
            for (_, p) in fields {
                record_place_uses(p, uses);
            }
        }
        Rvalue::EnumVariant { fields, .. } => {
            for p in fields {
                record_place_uses(p, uses);
            }
        }
        Rvalue::Phi(incoming) => {
            for (_, p) in incoming {
                record_place_uses(p, uses);
            }
        }
        Rvalue::Constant(_) | Rvalue::FnPtr(_) => {}
    }
}

fn record_terminator_uses(term: &Terminator, uses: &mut HashMap<String, usize>) {
    match term {
        Terminator::BranchIf { condition, .. } => {
            record_place_uses(condition, uses);
        }
        Terminator::Switch { value, .. } => {
            record_place_uses(value, uses);
        }
        Terminator::Return { value: Some(p) } => {
            record_place_uses(p, uses);
        }
        Terminator::IndirectCall { callee, args, .. } => {
            record_place_uses(callee, uses);
            for a in args {
                record_place_uses(a, uses);
            }
        }
        Terminator::Branch { .. }
        | Terminator::Return { value: None }
        | Terminator::Unreachable => {}
    }
}
