//! Phase 35: Post-Distillation Process-Tree & MIR Residual Compaction.
//!
//! Provides two compaction entry points:
//! 1. `compact_process_tree`: Prunes dead overflow nodes and deduplicates alpha-equivalent
//!    knot targets prior to residualization.
//! 2. `compact_mir_function`: Peephole optimization pass over residualized MIR functions,
//!    removing no-op identity assignments, performing conservative copy propagation
//!    into return terminators (eta-reduction), and folding empty branch-only blocks
//!    (jump threading) to a combined fixpoint.

use std::collections::{HashMap, HashSet};

use super::drive::{ProcessEdge, ProcessNodeId, ProcessTree};
use crate::mir::lower::{MirFunction, Rvalue, Statement};
use crate::mir::{BasicBlockId, Place, Projection, Terminator};

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
/// 3. Empty-branch folding: remove blocks with no statements whose terminator
///    is an unconditional Branch, redirecting all references to their target.
///
/// The three passes are rerun to a combined fixpoint: folding can expose new
/// no-op statements (and vice versa). Returns the number of statements and
/// blocks removed.
pub fn compact_mir_function(func: &mut MirFunction) -> usize {
    let mut total_removed = 0;
    loop {
        let p1 = run_noop_removal_pass(func);
        let p2 = run_copy_propagation_pass(func);
        let p3 = run_empty_branch_folding_pass(func);
        let iter_removed = p1 + p2 + p3;
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
                        && !src
                            .projections
                            .iter()
                            .any(|p| matches!(p, Projection::Index(_)))
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

/// Pass 3: Fold empty branch-only blocks (jump threading).
///
/// A block is foldable when it has no statements, no block arguments, and its
/// terminator is an unconditional `Branch` to a *different*, still-live block.
/// The following cases are deliberately left alone:
///   * the entry block (`blocks[0]`, which codegen treats as the function entry),
///   * self-loops and cycles made entirely of foldable blocks (infinite loops),
///   * blocks whose id is used as a Phi incoming label (rewriting those labels
///     would require splitting the Phi by predecessor),
///   * blocks that forward into a block with arguments.
///
/// For the remaining blocks, every BasicBlockId reference in surviving
/// terminators is redirected to the folded block's ultimate non-foldable
/// target, and the folded blocks are physically removed from `func.blocks`.
/// Ids are *not* renumbered: codegen builds a `BasicBlockId -> machine block`
/// map and only relies on `blocks[0]` being the entry, so stable (possibly
/// non-contiguous) ids are safe and keep back-edge heuristics consistent.
fn run_empty_branch_folding_pass(func: &mut MirFunction) -> usize {
    let Some(entry_id) = func.blocks.first().map(|b| b.id.clone()) else {
        return 0;
    };
    let live_ids: HashSet<BasicBlockId> = func.blocks.iter().map(|b| b.id.clone()).collect();
    let blocks_with_args: HashSet<BasicBlockId> = func
        .blocks
        .iter()
        .filter(|b| !b.arguments.is_empty())
        .map(|b| b.id.clone())
        .collect();

    // Phi incoming labels name the predecessor block; dropping such a block
    // would leave the Phi with a dangling/incorrect label.
    let phi_labels: HashSet<BasicBlockId> = func
        .blocks
        .iter()
        .flat_map(|b| b.statements.iter())
        .filter_map(|stmt| match stmt {
            Statement::Assign(_, Rvalue::Phi(incoming)) => Some(incoming),
            _ => None,
        })
        .flat_map(|incoming| incoming.iter().map(|(id, _)| id.clone()))
        .collect();

    // Candidate -> immediate forwarding target.
    let mut forward: HashMap<BasicBlockId, BasicBlockId> = HashMap::new();
    for block in &func.blocks {
        if !block.statements.is_empty() || !block.arguments.is_empty() {
            continue;
        }
        if block.id == entry_id || phi_labels.contains(&block.id) {
            continue;
        }
        if let Terminator::Branch { target } = &block.terminator {
            // Self-loops are never folded, and the target must still exist.
            if target != &block.id
                && live_ids.contains(target)
                && !blocks_with_args.contains(target)
            {
                forward.insert(block.id.clone(), target.clone());
            }
        }
    }
    if forward.is_empty() {
        return 0;
    }

    // Resolve each candidate to its ultimate target in one shot, which folds
    // arbitrarily long chains without needing a pass per link. Chains that end
    // in a cycle of foldable blocks are preserved conservatively.
    let mut final_target: HashMap<BasicBlockId, BasicBlockId> = HashMap::new();
    let mut skipped: HashSet<BasicBlockId> = HashSet::new();

    for start in forward.keys() {
        if final_target.contains_key(start) || skipped.contains(start) {
            continue;
        }
        let mut path: Vec<BasicBlockId> = Vec::new();
        let mut seen: HashSet<BasicBlockId> = HashSet::new();
        let mut cur = start.clone();
        let mut target: Option<BasicBlockId> = None;
        let mut abort = false;

        loop {
            if skipped.contains(&cur) {
                abort = true; // leads into a previously detected cycle
                break;
            }
            if let Some(resolved) = final_target.get(&cur).cloned() {
                target = Some(resolved);
                break;
            }
            let Some(next) = forward.get(&cur).cloned() else {
                target = Some(cur.clone()); // reached a non-foldable block
                break;
            };
            if !seen.insert(cur.clone()) {
                abort = true; // foldable cycle / loop back-edge: keep it
                break;
            }
            path.push(cur.clone());
            cur = next;
        }

        if abort {
            for id in path {
                skipped.insert(id);
            }
        } else if let Some(target) = target {
            for id in path {
                final_target.insert(id, target.clone());
            }
        }
    }

    let removed_count = final_target.len();
    if removed_count == 0 {
        return 0;
    }

    func.blocks
        .retain(|b| !final_target.contains_key(&b.id));
    for block in &mut func.blocks {
        redirect_terminator_blocks(&mut block.terminator, &final_target);
    }
    removed_count
}

/// Rewrite every BasicBlockId referenced by a terminator through the redirect map.
fn redirect_terminator_blocks(
    term: &mut Terminator,
    redirect: &HashMap<BasicBlockId, BasicBlockId>,
) {
    fn redirect_id(id: &mut BasicBlockId, redirect: &HashMap<BasicBlockId, BasicBlockId>) {
        if let Some(new_id) = redirect.get(id) {
            *id = new_id.clone();
        }
    }

    match term {
        Terminator::Branch { target } => redirect_id(target, redirect),
        Terminator::BranchIf {
            then_target,
            else_target,
            ..
        } => {
            redirect_id(then_target, redirect);
            redirect_id(else_target, redirect);
        }
        Terminator::Switch {
            targets, default, ..
        } => {
            redirect_id(default, redirect);
            for (_, target) in targets {
                redirect_id(target, redirect);
            }
        }
        Terminator::IndirectCall { next, .. } => redirect_id(next, redirect),
        Terminator::Fork { left, right, join } => {
            redirect_id(left, redirect);
            redirect_id(right, redirect);
            redirect_id(join, redirect);
        }
        Terminator::Force { cont, .. } => redirect_id(cont, redirect),
        Terminator::TypeGuard {
            fast_path, deopt_stub, ..
        } => {
            redirect_id(fast_path, redirect);
            redirect_id(deopt_stub, redirect);
        }
        Terminator::Return { .. } | Terminator::Unreachable => {}
    }
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
        Rvalue::Select(c, t, e) => {
            record_place_uses(c, uses);
            record_place_uses(t, uses);
            record_place_uses(e, uses);
        }
        Rvalue::Constant(_) | Rvalue::FnPtr(_) => {}
        Rvalue::Thunk { env, .. } => {
            for e in env {
                *uses.entry(e.clone()).or_insert(0) += 1;
            }
        }
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
        Terminator::Force { thunk, .. } => {
            *uses.entry(thunk.clone()).or_insert(0) += 1;
        }
        Terminator::TypeGuard { local, .. } => {
            record_place_uses(local, uses);
        }
        Terminator::Branch { .. }
        | Terminator::Return { value: None }
        | Terminator::Unreachable
        | Terminator::Fork { .. } => {}
    }
}
