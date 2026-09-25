//! Distillation: Global Process-Tree Generalization (Hamilton 2007).
//!
//! Unlike classical local supercompilation, distillation examines the relationship
//! between distinct branches in the process tree, identifying structural isomorphisms
//! that allow folding recursive definitions spanning multiple branches or functions.

use std::collections::{HashMap, HashSet};

use super::drive::{ProcessEdge, ProcessNode, ProcessNodeId, ProcessTree};
use super::term::TermInterner;
use crate::mir::lower::MirFunction;

pub struct DistillationEngine<'a> {
    pub func: &'a MirFunction,
    pub interner: &'a mut TermInterner,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum TreeShape {
    Leaf(Option<u64>),
    Unary(usize, Box<TreeShape>),
    Binary(usize, Box<TreeShape>, Box<TreeShape>),
    Knot(usize),
}

impl<'a> DistillationEngine<'a> {
    pub fn new(func: &'a MirFunction, interner: &'a mut TermInterner) -> Self {
        DistillationEngine { func, interner }
    }

    /// Identifies identical sub-tree structures across sibling branches
    /// and folds them into generalized knots or pruned equivalent paths.
    pub fn distill_process_tree(&mut self, tree: &mut ProcessTree) -> usize {
        let mut folds = 0;
        if tree.nodes.is_empty() {
            return 0;
        }

        // 1. Build an adjacency map of the tree
        let n = tree.nodes.len();
        let mut children: Vec<Vec<ProcessNodeId>> = vec![Vec::new(); n];
        for node in &tree.nodes {
            for edge in &node.edges {
                match edge {
                    ProcessEdge::Step(next)
                    | ProcessEdge::BranchTrue(next, _)
                    | ProcessEdge::BranchFalse(next, _) => {
                        if next.0 < n {
                            children[node.id.0].push(*next);
                        }
                    }
                    ProcessEdge::Knot(_) => {}
                }
            }
        }

        // 2. Compute tree shapes and canonical hashes bottom-up
        let mut shapes: HashMap<ProcessNodeId, TreeShape> = HashMap::new();
        let mut visited: HashSet<ProcessNodeId> = HashSet::new();

        for i in 0..n {
            let nid = ProcessNodeId(i);
            self.compute_shape(nid, &children, &tree.nodes, &mut shapes, &mut visited);
        }

        // 3. Find structural isomorphisms across distinct subtrees
        let mut shape_to_nodes: HashMap<TreeShape, Vec<ProcessNodeId>> = HashMap::new();
        for (nid, shape) in &shapes {
            // Only consider non-trivial subtrees (not single leaf nodes)
            if !matches!(shape, TreeShape::Leaf(_)) {
                shape_to_nodes.entry(shape.clone()).or_default().push(*nid);
            }
        }

        // 4. For isomorphic subtrees with different roots, unify redundant computations
        for (_shape, nodes) in shape_to_nodes {
            if nodes.len() >= 2 {
                let canonical = nodes[0];
                for &duplicate in &nodes[1..] {
                    if duplicate != canonical && !self.is_ancestor(canonical, duplicate, &children) {
                        // Redirect incoming edges to duplicate towards canonical or mark as knot
                        for node in &mut tree.nodes {
                            for edge in &mut node.edges {
                                match edge {
                                    ProcessEdge::Step(target) if *target == duplicate => {
                                        *edge = ProcessEdge::Knot(canonical);
                                        folds += 1;
                                    }
                                    ProcessEdge::BranchTrue(target, c) if *target == duplicate => {
                                        *edge = ProcessEdge::BranchTrue(canonical, *c);
                                        folds += 1;
                                    }
                                    ProcessEdge::BranchFalse(target, c) if *target == duplicate => {
                                        *edge = ProcessEdge::BranchFalse(canonical, *c);
                                        folds += 1;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }

        // 5. Update tree statistics
        tree.stats.loops_collapsed += folds;
        folds
    }

    fn compute_shape(
        &self,
        nid: ProcessNodeId,
        children: &[Vec<ProcessNodeId>],
        nodes: &[ProcessNode],
        shapes: &mut HashMap<ProcessNodeId, TreeShape>,
        visited: &mut HashSet<ProcessNodeId>,
    ) -> TreeShape {
        if let Some(s) = shapes.get(&nid) {
            return s.clone();
        }
        if visited.contains(&nid) {
            return TreeShape::Knot(nid.0);
        }
        visited.insert(nid);

        let ch = &children[nid.0];
        let shape = match ch.len() {
            0 => {
                let term_disc = nodes[nid.0].return_term.map(|t| t.0 as u64);
                TreeShape::Leaf(term_disc)
            }
            1 => {
                let child_shape = self.compute_shape(ch[0], children, nodes, shapes, visited);
                TreeShape::Unary(nodes[nid.0].state.block.0, Box::new(child_shape))
            }
            2 => {
                let left_shape = self.compute_shape(ch[0], children, nodes, shapes, visited);
                let right_shape = self.compute_shape(ch[1], children, nodes, shapes, visited);
                TreeShape::Binary(nodes[nid.0].state.block.0, Box::new(left_shape), Box::new(right_shape))
            }
            _ => {
                TreeShape::Leaf(None)
            }
        };

        visited.remove(&nid);
        shapes.insert(nid, shape.clone());
        shape
    }

    fn is_ancestor(&self, anc: ProcessNodeId, desc: ProcessNodeId, children: &[Vec<ProcessNodeId>]) -> bool {
        let mut queue = vec![anc];
        let mut seen = HashSet::new();
        while let Some(curr) = queue.pop() {
            if curr == desc {
                return true;
            }
            if seen.insert(curr) && curr.0 < children.len() {
                for &next in &children[curr.0] {
                    queue.push(next);
                }
            }
        }
        false
    }
}
