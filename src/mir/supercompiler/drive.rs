use std::collections::HashMap;
use std::fmt;

use super::generalize::solve_recurrence;
use super::state::SymbolicState;
use super::term::{SymTerm, SymTermId, TermInterner};
use super::whistle::{is_instance_of, state_embeds};
use crate::mir::dominance::detect_loops;
use crate::mir::lower::{MirBasicBlock, MirFunction, Rvalue, Statement};
use crate::mir::memory_ssa::MemoryVersionId;
use crate::mir::{compute_cfg, BasicBlockId, Place, Terminator};
use crate::typecheck::types::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessNodeId(pub usize);

#[derive(Debug, Clone)]
pub enum ProcessEdge {
    Step(ProcessNodeId),
    BranchTrue(ProcessNodeId, SymTermId),
    BranchFalse(ProcessNodeId, SymTermId),
    Knot(ProcessNodeId),
}

#[derive(Debug, Clone)]
pub struct ProcessNode {
    pub id: ProcessNodeId,
    pub state: SymbolicState,
    pub edges: Vec<ProcessEdge>,
    pub return_term: Option<SymTermId>,
}

#[derive(Debug, Clone, Default)]
pub struct SupercompilerStats {
    pub nodes_explored: usize,
    pub branches_pruned: usize,
    pub loops_collapsed: usize,
    pub knots_tied: usize,
}

impl fmt::Display for SupercompilerStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "nodes: {}, branches pruned: {}, loops collapsed: {}, knots tied: {}",
            self.nodes_explored, self.branches_pruned, self.loops_collapsed, self.knots_tied
        )
    }
}

#[derive(Debug, Clone)]
pub struct ProcessTree {
    pub nodes: Vec<ProcessNode>,
    pub root: ProcessNodeId,
    pub interner: TermInterner,
    pub stats: SupercompilerStats,
}

impl ProcessTree {
    pub fn display(&self) -> String {
        let mut out = String::new();
        out.push_str("Process Tree:\n");
        for node in &self.nodes {
            out.push_str(&format!(
                "  Node #{}: block = BasicBlockId({})\n",
                node.id.0, node.state.block.0
            ));
            if let Some(ret) = node.return_term {
                out.push_str(&format!("    Return: {}\n", ret));
            }
            for edge in &node.edges {
                match edge {
                    ProcessEdge::Step(next) => {
                        out.push_str(&format!("    -> Step to Node #{}\n", next.0));
                    }
                    ProcessEdge::BranchTrue(next, cond) => {
                        out.push_str(&format!(
                            "    -> BranchTrue to Node #{} [condition: {}]\n",
                            next.0, cond
                        ));
                    }
                    ProcessEdge::BranchFalse(next, cond) => {
                        out.push_str(&format!(
                            "    -> BranchFalse to Node #{} [condition: !{}]\n",
                            next.0, cond
                        ));
                    }
                    ProcessEdge::Knot(anc) => {
                        out.push_str(&format!("    -> Knot to Ancestor Node #{}\n", anc.0));
                    }
                }
            }
        }
        out.push_str(&format!("  Stats: {}\n", self.stats));
        out
    }
}

pub struct SupercompilerDriver<'a> {
    func: &'a MirFunction,
    nodes: Vec<ProcessNode>,
    interner: TermInterner,
    block_map: HashMap<BasicBlockId, &'a MirBasicBlock>,
    loop_headers: std::collections::HashSet<BasicBlockId>,
    next_node_id: usize,
    max_depth: usize,
    active_places: Vec<Place>,
    stats: SupercompilerStats,
}

impl<'a> SupercompilerDriver<'a> {
    pub fn new(func: &'a MirFunction) -> Self {
        let block_map: HashMap<BasicBlockId, &'a MirBasicBlock> =
            func.blocks.iter().map(|b| (b.id.clone(), b)).collect();

        let (preds, succs) = compute_cfg(&func.blocks);
        let all_block_ids: Vec<BasicBlockId> = func.blocks.iter().map(|b| b.id.clone()).collect();
        let entry = func.blocks[0].id.clone();
        let dom = crate::mir::dominance::compute_dominance(entry.clone(), &preds, &succs, &all_block_ids);
        let loop_info = detect_loops(entry, &preds, &succs, &all_block_ids, &dom);

        let mut active_places = Vec::new();
        for local in &func.locals {
            active_places.push(Place {
                local: local.name.clone(),
                projections: vec![],
            });
        }

        SupercompilerDriver {
            func,
            nodes: Vec::new(),
            interner: TermInterner::new(),
            block_map,
            loop_headers: loop_info.headers,
            next_node_id: 0,
            max_depth: 128,
            active_places,
            stats: SupercompilerStats::default(),
        }
    }

    pub fn run(mut self) -> ProcessTree {
        if self.func.blocks.is_empty() {
            return ProcessTree {
                nodes: Vec::new(),
                root: ProcessNodeId(0),
                interner: self.interner,
                stats: self.stats,
            };
        }

        let entry_id = self.func.blocks[0].id.clone();
        let mut initial_state = SymbolicState::new(entry_id, MemoryVersionId::LIVE_ON_ENTRY);

        // Bind function parameters to symbolic variables
        for (param_name, param_ty) in &self.func.params {
            let p = Place {
                local: param_name.clone(),
                projections: vec![],
            };
            let term = self.interner.intern_var(p.clone(), param_ty.clone());
            initial_state.set_value(p, term);
        }

        let root_id = self.alloc_node(initial_state.clone());
        let mut ancestor_stack = Vec::new();

        self.drive_node(root_id, &mut ancestor_stack, 0);

        self.stats.nodes_explored = self.nodes.len();

        ProcessTree {
            nodes: self.nodes,
            root: root_id,
            interner: self.interner,
            stats: self.stats,
        }
    }

    fn alloc_node(&mut self, state: SymbolicState) -> ProcessNodeId {
        let id = ProcessNodeId(self.next_node_id);
        self.next_node_id += 1;
        self.nodes.push(ProcessNode {
            id,
            state,
            edges: Vec::new(),
            return_term: None,
        });
        id
    }

    fn drive_node(
        &mut self,
        node_id: ProcessNodeId,
        ancestor_stack: &mut Vec<ProcessNodeId>,
        depth: usize,
    ) {
        if depth >= self.max_depth {
            return;
        }

        let current_state = self.nodes[node_id.0].state.clone();
        let (statements, terminator) = match self.block_map.get(&current_state.block) {
            Some(b) => (b.statements.clone(), b.terminator.clone()),
            None => return,
        };

        // Step 1: Drive all statements in current block sequentially
        let mut working_state = current_state;
        for stmt in &statements {
            self.drive_statement(stmt, &mut working_state);
        }

        // Step 2: Handle block terminator
        match &terminator {
            Terminator::Return { value } => {
                let ret_term = value.as_ref().and_then(|p| working_state.get_value(p));
                self.nodes[node_id.0].return_term = ret_term;
            }
            Terminator::Branch { target } => {
                let mut next_state = working_state;
                next_state.block = target.clone();
                self.handle_transition(node_id, next_state, ancestor_stack, depth);
            }
            Terminator::BranchIf {
                condition,
                then_target,
                else_target,
            } => {
                let cond_term = working_state.get_value(condition).unwrap_or_else(|| {
                    self.interner.intern_var(condition.clone(), Type::Bool)
                });

                // Positive constraint evaluation
                let evaluated = working_state
                    .path_constraints
                    .evaluate_condition(cond_term, &self.interner);

                match evaluated {
                    Some(true) => {
                        self.stats.branches_pruned += 1;
                        let mut next_state = working_state;
                        next_state.block = then_target.clone();
                        next_state.path_constraints.add_condition(cond_term, true, &self.interner);
                        self.handle_transition(node_id, next_state, ancestor_stack, depth);
                    }
                    Some(false) => {
                        self.stats.branches_pruned += 1;
                        let mut next_state = working_state;
                        next_state.block = else_target.clone();
                        next_state.path_constraints.add_condition(cond_term, false, &self.interner);
                        self.handle_transition(node_id, next_state, ancestor_stack, depth);
                    }
                    None => {
                        // Split into two child branches in the process tree
                        let mut then_state = working_state.clone();
                        then_state.block = then_target.clone();
                        then_state
                            .path_constraints
                            .add_condition(cond_term, true, &self.interner);
                        let then_node = self.alloc_node(then_state);
                        self.nodes[node_id.0]
                            .edges
                            .push(ProcessEdge::BranchTrue(then_node, cond_term));

                        let mut else_state = working_state;
                        else_state.block = else_target.clone();
                        else_state
                            .path_constraints
                            .add_condition(cond_term, false, &self.interner);
                        let else_node = self.alloc_node(else_state);
                        self.nodes[node_id.0]
                            .edges
                            .push(ProcessEdge::BranchFalse(else_node, cond_term));

                        ancestor_stack.push(node_id);
                        self.drive_node(then_node, ancestor_stack, depth + 1);
                        self.drive_node(else_node, ancestor_stack, depth + 1);
                        ancestor_stack.pop();
                    }
                }
            }
            Terminator::Switch { default, .. } => {
                let mut next_state = working_state;
                next_state.block = default.clone();
                self.handle_transition(node_id, next_state, ancestor_stack, depth);
            }
            Terminator::Unreachable => {}
        }
    }

    fn drive_statement(&mut self, stmt: &Statement, state: &mut SymbolicState) {
        let Statement::Assign(dest, rval) = stmt;
        let term = match rval {
            Rvalue::Use(p) => state
                .get_value(p)
                .unwrap_or_else(|| self.interner.intern_var(p.clone(), Type::I64)),
            Rvalue::Constant(lit) => self.interner.intern_const(lit.clone()),
            Rvalue::BinaryOp(op, l, r) => {
                let l_term = state
                    .get_value(l)
                    .unwrap_or_else(|| self.interner.intern_var(l.clone(), Type::I64));
                let r_term = state
                    .get_value(r)
                    .unwrap_or_else(|| self.interner.intern_var(r.clone(), Type::I64));
                self.interner.intern_binary(*op, l_term, r_term, Type::I64)
            }
            Rvalue::UnaryOp(op, inner) => {
                let in_term = state
                    .get_value(inner)
                    .unwrap_or_else(|| self.interner.intern_var(inner.clone(), Type::I64));
                self.interner.intern_unary(*op, in_term, Type::I64)
            }
            Rvalue::Call(callee, args) => {
                let arg_terms: Vec<SymTermId> = args
                    .iter()
                    .map(|p| {
                        state
                            .get_value(p)
                            .unwrap_or_else(|| self.interner.intern_var(p.clone(), Type::I64))
                    })
                    .collect();
                self.interner.intern_call(callee.clone(), arg_terms, Type::I64)
            }
            Rvalue::Phi(incoming) => {
                let mut phi_ops = Vec::new();
                for (b, p) in incoming {
                    if let Some(t) = state.get_value(p) {
                        phi_ops.push((b.clone(), t));
                    }
                }
                self.interner.intern_phi(phi_ops, Type::I64)
            }
            _ => self.interner.intern_var(dest.clone(), Type::I64),
        };

        state.set_value(dest.clone(), term);
    }

    fn handle_transition(
        &mut self,
        from_id: ProcessNodeId,
        next_state: SymbolicState,
        ancestor_stack: &mut Vec<ProcessNodeId>,
        depth: usize,
    ) {
        // 1. Knot-tying test: is next_state an exact instance of an ancestor?
        for &anc_id in ancestor_stack.iter().rev() {
            let anc_state = &self.nodes[anc_id.0].state;
            if is_instance_of(anc_state, &next_state, &self.active_places) {
                // Knot tied! Fold back to ancestor loop header
                self.stats.knots_tied += 1;
                self.nodes[from_id.0].edges.push(ProcessEdge::Knot(anc_id));
                return;
            }
        }

        // 2. Whistle test: does an ancestor loop header embed next_state?
        let is_loop_header = self.loop_headers.contains(&next_state.block);
        if is_loop_header {
            for &anc_id in ancestor_stack.iter().rev() {
                let anc_state = self.nodes[anc_id.0].state.clone();
                if state_embeds(&anc_state, &next_state, &self.active_places, &self.interner) {
                    // Whistle blew! Growth detected across iterations.
                    // Try recurrence solver on mutating induction places:
                    if let Some(solved_state) = self.try_solve_loop_recurrence(&anc_state, &next_state, ancestor_stack) {
                        self.stats.loops_collapsed += 1;
                        let next_node = self.alloc_node(solved_state);
                        self.nodes[from_id.0].edges.push(ProcessEdge::Step(next_node));
                        ancestor_stack.push(from_id);
                        self.drive_node(next_node, ancestor_stack, depth + 1);
                        ancestor_stack.pop();
                        return;
                    }
                }
            }
        }

        // 3. Normal transition
        let next_node = self.alloc_node(next_state);
        self.nodes[from_id.0].edges.push(ProcessEdge::Step(next_node));

        ancestor_stack.push(from_id);
        self.drive_node(next_node, ancestor_stack, depth + 1);
        ancestor_stack.pop();
    }

    fn try_solve_loop_recurrence(
        &mut self,
        anc: &SymbolicState,
        curr: &SymbolicState,
        ancestor_stack: &[ProcessNodeId],
    ) -> Option<SymbolicState> {
        let mut solved_state = curr.clone();
        let mut any_solved = false;

        let mut bound_term = None;
        if let Some(block) = self.block_map.get(&curr.block) {
            if let Terminator::BranchIf { condition, .. } = &block.terminator {
                if let Some(cond_t) = curr.get_value(condition) {
                    if let SymTerm::Binary(_, _, r, _) = self.interner.get(cond_t) {
                        bound_term = Some(*r);
                    }
                }
            }
        }
        let n_term = bound_term.unwrap_or_else(|| {
            self.interner.intern_var(
                Place {
                    local: "n".to_string(),
                    projections: vec![],
                },
                Type::I64,
            )
        });

        for place in &self.active_places {
            // Collect historical values of this place from loop header ancestors
            let mut history = Vec::new();
            for &anc_id in ancestor_stack {
                let a_state = &self.nodes[anc_id.0].state;
                if a_state.block == curr.block {
                    if let Some(t) = a_state.get_value(place) {
                        if let SymTerm::ConstInt(val, _) = self.interner.get(t) {
                            history.push(*val);
                        }
                    }
                }
            }
            if let Some(t_curr) = curr.get_value(place) {
                if let SymTerm::ConstInt(val, _) = self.interner.get(t_curr) {
                    history.push(*val);
                }
            }

            if history.len() >= 3 {
                if let Some(closed_form) = solve_recurrence(&history, n_term, &mut self.interner) {
                    solved_state.set_value(place.clone(), closed_form);
                    any_solved = true;
                }
            } else if let (Some(t_anc), Some(t_curr)) = (anc.get_value(place), curr.get_value(place)) {
                if t_anc != t_curr {
                    if let (
                        SymTerm::ConstInt(v0, _),
                        SymTerm::ConstInt(v1, _),
                    ) = (self.interner.get(t_anc), self.interner.get(t_curr))
                    {
                        let step = v1 - v0;
                        if step != 0 {
                            let samples = vec![
                                *v0,
                                *v1,
                                v1 + step,
                                v1 + step + step,
                                v1 + step + step + step,
                            ];
                            if let Some(closed_form) =
                                solve_recurrence(&samples, n_term, &mut self.interner)
                            {
                                solved_state.set_value(place.clone(), closed_form);
                                any_solved = true;
                            }
                        }
                    }
                }
            }
        }

        if any_solved {
            Some(solved_state)
        } else {
            None
        }
    }
}
