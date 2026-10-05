//! Phase 36: Independence Analysis for Parallel Residualization.
//!
//! Detects provably data-independent subtrees in the ProcessTree by analyzing
//! their read and write footprints in the symbolic state environment.

use std::collections::HashSet;

use super::drive::{ProcessEdge, ProcessNodeId, ProcessTree};
use super::term::{SymTerm, SymTermId, TermInterner};

/// Read/write footprint of a process-tree subtree, in terms of local variable names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadWriteSet {
    pub reads: HashSet<String>,
    pub writes: HashSet<String>,
}

/// True if two ReadWriteSets are data-independent (no write-read or write-write conflicts).
pub fn sets_are_independent(a: &ReadWriteSet, b: &ReadWriteSet) -> bool {
    a.writes.is_disjoint(&b.reads)
        && a.writes.is_disjoint(&b.writes)
        && b.writes.is_disjoint(&a.reads)
}

/// Recursively extract all free local variable names referenced in a symbolic term.
pub fn collect_term_vars(interner: &TermInterner, term_id: SymTermId, vars: &mut HashSet<String>) {
    let mut visited = HashSet::new();
    let mut stack = vec![term_id];
    while let Some(t) = stack.pop() {
        if !visited.insert(t) {
            continue;
        }
        match interner.get(t) {
            SymTerm::Var(p, _) => {
                vars.insert(p.local.clone());
                for proj in &p.projections {
                    if let crate::mir::Projection::Index(idx_place) = proj {
                        vars.insert(idx_place.local.clone());
                    }
                }
            }
            SymTerm::Binary(_, l, r, _) => {
                stack.push(*l);
                stack.push(*r);
            }
            SymTerm::Unary(_, sub, _)
            | SymTerm::Ref(sub, _)
            | SymTerm::Deref(sub, _)
            | SymTerm::Discriminant(sub, _) => {
                stack.push(*sub);
            }
            SymTerm::Select(c, thn, els, _) => {
                stack.push(*c);
                stack.push(*thn);
                stack.push(*els);
            }
            SymTerm::Constructor(_, _, args, _)
            | SymTerm::Call(_, args, _)
            | SymTerm::ClosureVal(_, args, _)
            | SymTerm::Thunk(_, args, _) => {
                stack.extend(args.iter().copied());
            }
            SymTerm::Phi(branches, _) => {
                for (_, branch_term) in branches {
                    stack.push(*branch_term);
                }
            }
            SymTerm::ConstInt(..)
            | SymTerm::ConstFloat(..)
            | SymTerm::ConstBool(..)
            | SymTerm::ConstStr(..) => {}
        }
    }
}

/// Walk all nodes reachable from `root_id` in the process tree and collect
/// all local names that are read and written in the symbolic states.
pub fn collect_subtree_rw_set(tree: &ProcessTree, root_id: ProcessNodeId) -> ReadWriteSet {
    let mut rw = ReadWriteSet::default();
    if root_id.0 >= tree.nodes.len() {
        return rw;
    }

    let mut visited = HashSet::new();
    let mut queue = vec![root_id];

    while let Some(curr_id) = queue.pop() {
        if !visited.insert(curr_id) {
            continue;
        }
        let node = &tree.nodes[curr_id.0];
        if node.overflow {
            continue;
        }

        // Environment reads & writes
        for (place, &term_id) in &node.state.env {
            rw.writes.insert(place.local.clone());
            collect_term_vars(&tree.interner, term_id, &mut rw.reads);
        }

        // Return term read
        if let Some(ret_t) = node.return_term {
            collect_term_vars(&tree.interner, ret_t, &mut rw.reads);
        }

        // Edge traversal
        for edge in &node.edges {
            match edge {
                ProcessEdge::Step(t) => {
                    queue.push(*t);
                }
                ProcessEdge::BranchTrue(t, cond) | ProcessEdge::BranchFalse(t, cond) => {
                    collect_term_vars(&tree.interner, *cond, &mut rw.reads);
                    queue.push(*t);
                }
                ProcessEdge::Knot(anc) => {
                    if anc.0 < tree.nodes.len() {
                        let anc_node = &tree.nodes[anc.0];
                        for (p, &anc_t) in &anc_node.state.env {
                            if let Some(&curr_t) = node.state.env.get(p) {
                                if curr_t != anc_t {
                                    rw.writes.insert(p.local.clone());
                                    collect_term_vars(&tree.interner, curr_t, &mut rw.reads);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    rw
}

/// For each pair of sibling ProcessEdge::Knot targets in the tree, check independence.
/// Returns a Vec of (from_node_id, left_knot_target, right_knot_target) triples
/// where the two subtrees are provably data-independent.
pub fn find_parallel_knot_pairs(
    tree: &ProcessTree,
) -> Vec<(ProcessNodeId, ProcessNodeId, ProcessNodeId)> {
    let mut pairs = Vec::new();
    for node in &tree.nodes {
        if node.overflow {
            continue;
        }
        let knots: Vec<ProcessNodeId> = node
            .edges
            .iter()
            .filter_map(|e| match e {
                ProcessEdge::Knot(t) => Some(*t),
                _ => None,
            })
            .collect();

        for i in 0..knots.len() {
            for j in (i + 1)..knots.len() {
                let k1 = knots[i];
                let k2 = knots[j];
                let rw1 = collect_subtree_rw_set(tree, k1);
                let rw2 = collect_subtree_rw_set(tree, k2);
                if sets_are_independent(&rw1, &rw2) {
                    pairs.push((node.id, k1, k2));
                }
            }
        }
    }
    pairs
}
