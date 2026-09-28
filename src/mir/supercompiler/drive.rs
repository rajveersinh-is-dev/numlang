use std::collections::HashMap;
use std::fmt;

use crate::ast::{BinaryOp, UnaryOp};
use super::generalize::{solve_coupled_2var_recurrence, solve_recurrence};
use super::state::{Interval, SymbolicState};
use super::term::{SymTerm, SymTermId, TermInterner};
use super::whistle::{is_instance_of, state_embeds};
use crate::mir::dominance::detect_loops;
use crate::mir::lower::{MirBasicBlock, MirFunction, Rvalue, Statement};
use crate::mir::memory_ssa::MemoryVersionId;
use crate::mir::{compute_cfg, BasicBlockId, Place, Projection, Terminator};
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
    pub overflow: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SupercompilerStats {
    pub nodes_explored: usize,
    pub branches_pruned: usize,
    pub loops_collapsed: usize,
    pub knots_tied: usize,
    pub calls_inlined: usize,
    pub sc_bce_eliminated: usize,
}

impl fmt::Display for SupercompilerStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "nodes: {}, branches pruned: {}, loops collapsed: {}, knots tied: {}, calls inlined: {}, sc bce eliminated: {}",
            self.nodes_explored, self.branches_pruned, self.loops_collapsed, self.knots_tied, self.calls_inlined, self.sc_bce_eliminated
        )
    }
}

/// A single whistle-firing event — records that the HE check detected `ancestor ⊴ descendant`
/// at node `from_id`, causing a knot to be tied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhistleFiring {
    /// The node that was being driven when the whistle fired.
    pub from_id: ProcessNodeId,
    /// The ancestor node whose state embeds into the current state.
    pub ancestor_id: ProcessNodeId,
    /// The kind of stopping: HE whistle or header-visit cutoff.
    pub kind: WhistleKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhistleKind {
    /// Homeomorphic embedding detected (state_embeds returned true).
    HomeomorphicEmbedding,
    /// Header visit count >= 3 (empirical cutoff; no HE check fired first).
    HeaderVisitCutoff,
}

/// A complete termination certificate for one function's process tree.
/// Presence of this struct (with a non-empty `firings` vec) proves that
/// the driving loop terminated by whistle, not by accident.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminationWitness {
    pub firings: Vec<WhistleFiring>,
}

#[derive(Debug, Clone)]
pub struct ProcessTree {
    pub nodes: Vec<ProcessNode>,
    pub root: ProcessNodeId,
    pub interner: TermInterner,
    pub stats: SupercompilerStats,
    pub witness: TerminationWitness,
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

#[derive(Debug, Clone, Copy)]
pub struct DriverConfig {
    pub max_depth: usize,
    pub max_unroll_depth: usize,
    pub solve_recurrences: bool,
    pub inline_calls: bool,
    pub max_inline_depth: usize,
    pub max_inline_nodes: usize,
    pub inline_loop_body_calls: bool,
}

impl Default for DriverConfig {
    fn default() -> Self {
        DriverConfig {
            max_depth: 128,
            max_unroll_depth: 0,
            solve_recurrences: true,
            inline_calls: true,
            max_inline_depth: 3,
            max_inline_nodes: 512,
            inline_loop_body_calls: false,
        }
    }
}

pub struct SupercompilerDriver<'a> {
    func: &'a MirFunction,
    nodes: Vec<ProcessNode>,
    interner: TermInterner,
    block_map: HashMap<BasicBlockId, &'a MirBasicBlock>,
    loop_headers: std::collections::HashSet<BasicBlockId>,
    natural_loops: HashMap<BasicBlockId, std::collections::HashSet<BasicBlockId>>,
    next_node_id: usize,
    max_depth: usize,
    active_places: Vec<Place>,
    stats: SupercompilerStats,
    program_funcs: HashMap<String, &'a MirFunction>,
    call_stack: Vec<String>,
    pub config: DriverConfig,
    witness: TerminationWitness,
    param_refinements: HashMap<String, Interval>,
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
            natural_loops: loop_info.natural_loops,
            next_node_id: 0,
            max_depth: 128,
            active_places,
            stats: SupercompilerStats::default(),
            program_funcs: HashMap::new(),
            call_stack: vec![func.name.clone()],
            config: DriverConfig::default(),
            witness: TerminationWitness::default(),
            param_refinements: HashMap::new(),
        }
    }

    pub fn with_config(mut self, config: DriverConfig) -> Self {
        self.max_depth = config.max_depth;
        self.config = config;
        self
    }

    pub fn with_unroll_depth(mut self, depth: usize) -> Self {
        self.config.max_unroll_depth = depth;
        self
    }

    pub fn with_recurrence_solving(mut self, enable: bool) -> Self {
        self.config.solve_recurrences = enable;
        self
    }

    pub fn with_inlining(mut self, enable: bool) -> Self {
        self.config.inline_calls = enable;
        self
    }

    pub fn with_max_inline_depth(mut self, depth: usize) -> Self {
        self.config.max_inline_depth = depth;
        self
    }

    pub fn with_max_inline_nodes(mut self, nodes: usize) -> Self {
        self.config.max_inline_nodes = nodes;
        self
    }

    pub fn with_inline_loop_body_calls(mut self, enable: bool) -> Self {
        self.config.inline_loop_body_calls = enable;
        self
    }

    pub fn with_program_functions(mut self, funcs: &'a [MirFunction]) -> Self {
        self.program_funcs = funcs
            .iter()
            .filter(|f| f.name != self.func.name)
            .map(|f| (f.name.clone(), f))
            .collect();
        self
    }

    pub fn with_program_functions_map(mut self, map: HashMap<String, &'a MirFunction>) -> Self {
        self.program_funcs = map;
        self
    }

    pub fn with_call_stack(mut self, stack: Vec<String>) -> Self {
        self.call_stack = stack;
        self
    }

    pub fn with_param_refinement(mut self, param_name: &str, iv: Interval) -> Self {
        self.param_refinements.insert(param_name.to_string(), iv);
        self
    }

    pub fn interner(&self) -> &TermInterner {
        &self.interner
    }

    pub fn interner_mut(&mut self) -> &mut TermInterner {
        &mut self.interner
    }

    pub fn run(mut self) -> ProcessTree {
        if self.func.blocks.is_empty() {
            return ProcessTree {
                nodes: Vec::new(),
                root: ProcessNodeId(0),
                interner: self.interner,
                stats: self.stats,
                witness: self.witness,
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
            if let Some(iv) = self.param_refinements.get(param_name) {
                initial_state.refine(term, iv.clone());
            }
        }

        self.run_with_initial_state(initial_state)
    }

    pub fn run_with_initial_state(mut self, initial_state: SymbolicState) -> ProcessTree {
        if self.func.blocks.is_empty() {
            return ProcessTree {
                nodes: Vec::new(),
                root: ProcessNodeId(0),
                interner: self.interner,
                stats: self.stats,
                witness: self.witness,
            };
        }

        let root_id = self.alloc_node(initial_state);
        let mut ancestor_stack = Vec::new();

        self.drive_node(root_id, &mut ancestor_stack, 0);

        self.stats.nodes_explored = self.nodes.len();

        ProcessTree {
            nodes: self.nodes,
            root: root_id,
            interner: self.interner,
            stats: self.stats,
            witness: self.witness,
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
            overflow: false,
        });
        id
    }

    fn drive_node(
        &mut self,
        node_id: ProcessNodeId,
        ancestor_stack: &mut Vec<ProcessNodeId>,
        depth: usize,
    ) {
        if depth >= self.max_depth || self.nodes.len() >= self.config.max_inline_nodes {
            self.nodes[node_id.0].overflow = true;
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
                self.nodes[node_id.0].state = working_state;
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

                let cond_root = working_state.path_constraints.find_leader(cond_term);

                // Check interval-based condition evaluation
                let mut is_bce_elim = false;
                let interval_eval = if let SymTerm::Binary(op, left, right, _) = self.interner.get(cond_root) {
                    let l_iv = self.get_term_interval(*left, &working_state);
                    let r_iv = self.get_term_interval(*right, &working_state);
                    let res = evaluate_comparison_intervals(*op, &l_iv, &r_iv);
                    if res == Some(true) {
                        if let (Some(_), BinaryOp::Lt | BinaryOp::Le) = (r_iv.lo, op) {
                            if r_iv.lo == r_iv.hi && l_iv.definitely_nonneg() {
                                is_bce_elim = true;
                            }
                        }
                    }
                    res
                } else {
                    None
                };

                // Positive constraint evaluation combined with refinement interval check
                let evaluated = working_state
                    .path_constraints
                    .evaluate_condition(cond_term, &self.interner)
                    .or(interval_eval);

                match evaluated {
                    Some(true) => {
                        self.stats.branches_pruned += 1;
                        if is_bce_elim {
                            self.stats.sc_bce_eliminated += 1;
                        }
                        let mut next_state = working_state;
                        next_state.block = then_target.clone();
                        next_state.path_constraints.add_condition(cond_term, true, &self.interner);
                        narrow_condition_intervals(cond_term, true, &mut next_state, &self.interner);
                        self.handle_transition(node_id, next_state, ancestor_stack, depth);
                    }
                    Some(false) => {
                        self.stats.branches_pruned += 1;
                        let mut next_state = working_state;
                        next_state.block = else_target.clone();
                        next_state.path_constraints.add_condition(cond_term, false, &self.interner);
                        narrow_condition_intervals(cond_term, false, &mut next_state, &self.interner);
                        self.handle_transition(node_id, next_state, ancestor_stack, depth);
                    }
                    None => {
                        let mut then_state = working_state.clone();
                        then_state.block = then_target.clone();
                        then_state
                            .path_constraints
                            .add_condition(cond_term, true, &self.interner);
                        narrow_condition_intervals(cond_term, true, &mut then_state, &self.interner);

                        let mut else_state = working_state;
                        else_state.block = else_target.clone();
                        else_state
                            .path_constraints
                            .add_condition(cond_term, false, &self.interner);
                        narrow_condition_intervals(cond_term, false, &mut else_state, &self.interner);

                        // Dead-branch pruning if an interval becomes empty
                        let then_dead = then_state.refinements.values().any(|iv| iv.is_empty());
                        let else_dead = else_state.refinements.values().any(|iv| iv.is_empty());

                        if then_dead && !else_dead {
                            self.stats.branches_pruned += 1;
                            self.handle_transition(node_id, else_state, ancestor_stack, depth);
                            return;
                        }
                        if else_dead && !then_dead {
                            self.stats.branches_pruned += 1;
                            self.handle_transition(node_id, then_state, ancestor_stack, depth);
                            return;
                        }
                        if then_dead && else_dead {
                            self.nodes[node_id.0].overflow = true;
                            return;
                        }

                        if self.nodes.len() >= self.config.max_inline_nodes {
                            for &anc_id in ancestor_stack.iter().rev() {
                                if self.nodes[anc_id.0].state.block == self.nodes[node_id.0].state.block {
                                    self.stats.knots_tied += 1;
                                    self.nodes[node_id.0].edges.push(ProcessEdge::Knot(anc_id));
                                    return;
                                }
                            }
                            self.nodes[node_id.0].overflow = true;
                            return;
                        }

                        // Split into two child branches in the process tree
                        let then_node = self.alloc_node(then_state);
                        self.nodes[node_id.0]
                            .edges
                            .push(ProcessEdge::BranchTrue(then_node, cond_term));

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
            Terminator::Switch {
                value,
                targets,
                default,
            } => {
                let val_res = self.resolve_place(value, &working_state);
                let val_term = working_state
                    .get_value(&val_res)
                    .unwrap_or_else(|| self.interner.intern_var(val_res, Type::I64));
                let lead_val = working_state.path_constraints.find_leader(val_term);

                if let SymTerm::ConstInt(tag, _) = self.interner.get(lead_val) {
                    let mut matched = false;
                    for (case_val, target_bb) in targets {
                        if case_val == tag {
                            self.stats.branches_pruned += targets.len() - 1;
                            let mut next_state = working_state.clone();
                            next_state.block = target_bb.clone();
                            self.handle_transition(node_id, next_state, ancestor_stack, depth);
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        self.stats.branches_pruned += targets.len();
                        let mut next_state = working_state;
                        next_state.block = default.clone();
                        self.handle_transition(node_id, next_state, ancestor_stack, depth);
                    }
                } else {
                    let all_case_vals: Vec<i64> = targets.iter().map(|(v, _)| *v).collect();

                    for (i, (case_val, target_bb)) in targets.iter().enumerate() {
                        let mut arm_state = working_state.clone();
                        arm_state.block = target_bb.clone();
                        let case_term = self.interner.intern_int(*case_val);
                        let cond_eq = self.interner.intern_binary(
                            BinaryOp::Eq,
                            val_term,
                            case_term,
                            Type::Bool,
                        );
                        arm_state
                            .path_constraints
                            .add_condition(cond_eq, true, &self.interner);

                        for (j, &other_val) in all_case_vals.iter().enumerate() {
                            if j != i {
                                arm_state.path_constraints.add_not_equal_int(val_term, other_val);
                            }
                        }

                        let arm_node = self.alloc_node(arm_state);
                        self.nodes[node_id.0]
                            .edges
                            .push(ProcessEdge::BranchTrue(arm_node, cond_eq));
                        ancestor_stack.push(node_id);
                        self.drive_node(arm_node, ancestor_stack, depth + 1);
                        ancestor_stack.pop();
                    }

                    let mut def_state = working_state;
                    def_state.block = default.clone();
                    for &v in &all_case_vals {
                        def_state.path_constraints.add_not_equal_int(val_term, v);
                    }
                    let def_node = self.alloc_node(def_state);
                    self.nodes[node_id.0]
                        .edges
                        .push(ProcessEdge::Step(def_node));
                    ancestor_stack.push(node_id);
                    self.drive_node(def_node, ancestor_stack, depth + 1);
                    ancestor_stack.pop();
                }
            }
            Terminator::IndirectCall { next, dest, .. } => {
                let mut next_state = working_state;
                next_state.block = next.clone();
                let dest_term = self.interner.intern_var(dest.clone(), Type::I64);
                next_state.set_value(dest.clone(), dest_term);
                let next_node = self.alloc_node(next_state);
                self.nodes[node_id.0].edges.push(ProcessEdge::Step(next_node));
                ancestor_stack.push(node_id);
                self.drive_node(next_node, ancestor_stack, depth + 1);
                ancestor_stack.pop();
            }
            Terminator::Unreachable => {}
        }
    }

    fn resolve_place(&self, p: &Place, state: &SymbolicState) -> Place {
        let mut curr_place = p.clone();
        for _ in 0..10 {
            if let Some(&base_term_id) = state.env.get(&Place { local: curr_place.local.clone(), projections: vec![] }) {
                let root_id = state.path_constraints.find_leader(base_term_id);
                if let SymTerm::Var(base_place, _) = self.interner.get(root_id) {
                    if base_place.local != curr_place.local || !base_place.projections.is_empty() {
                        let mut new_proj = base_place.projections.clone();
                        new_proj.extend(curr_place.projections.clone());
                        curr_place = Place {
                            local: base_place.local.clone(),
                            projections: new_proj,
                        };
                        continue;
                    }
                }
            }
            break;
        }
        curr_place
    }

    fn get_dest_type(&self, dest: &Place) -> Type {
        self.func
            .locals
            .iter()
            .find(|l| l.name == dest.local)
            .map(|l| l.ty.clone())
            .unwrap_or(Type::I64)
    }

    fn get_term_interval(&self, term: SymTermId, state: &SymbolicState) -> Interval {
        let leader = state.path_constraints.find_leader(term);
        let mut iv = state.get_refinement(leader);
        if iv == Interval::FULL && leader != term {
            iv = state.get_refinement(term);
        }
        if let SymTerm::ConstInt(v, _) = self.interner.get(leader) {
            iv = iv.meet(&Interval::exact(*v));
        } else if let SymTerm::ConstInt(v, _) = self.interner.get(term) {
            iv = iv.meet(&Interval::exact(*v));
        }
        if let Some(&(lo, hi)) = state.path_constraints.integer_bounds.get(&leader) {
            iv = iv.meet(&Interval { lo, hi });
        }
        if let Some(&(lo, hi)) = state.path_constraints.integer_bounds.get(&term) {
            iv = iv.meet(&Interval { lo, hi });
        }
        iv
    }

    fn check_statement_bce(&mut self, dest: &Place, rval: &Rvalue, state: &SymbolicState) {
        let mut check_place_index = |p: &Place| {
            for proj in &p.projections {
                if let Projection::Index(idx_place) = proj {
                    if let Some(target_decl) = self.func.locals.iter().find(|l| l.name == p.local) {
                        if let Type::Array(_, len) = &target_decl.ty {
                            let idx_res = self.resolve_place(idx_place, state);
                            if let Some(idx_term) = state.get_value(&idx_res) {
                                let iv = self.get_term_interval(idx_term, state);
                                if iv.definitely_nonneg() && iv.definitely_lt(*len as i64) {
                                    self.stats.sc_bce_eliminated += 1;
                                }
                            }
                        }
                    }
                }
            }
        };

        check_place_index(dest);
        if let Rvalue::Use(src) = rval {
            check_place_index(src);
        }
    }

    fn drive_statement(&mut self, stmt: &Statement, state: &mut SymbolicState) {
        let Statement::Assign(dest, rval) = stmt;
        self.check_statement_bce(dest, rval, state);
        let dest_ty = self.get_dest_type(dest);
        let term = match rval {
            Rvalue::Use(p) => {
                let resolved_p = self.resolve_place(p, state);
                let val_term = if let Some(Projection::Payload(i)) = resolved_p.projections.first() {
                    let base_place = Place {
                        local: resolved_p.local.clone(),
                        projections: vec![],
                    };
                    if let Some(base_term_id) = state.get_value(&base_place) {
                        let root_id = state.path_constraints.find_leader(base_term_id);
                        if let SymTerm::Constructor(_, _, fields, _) = self.interner.get(root_id) {
                            if let Some(&f_term) = fields.get(*i) {
                                f_term
                            } else {
                                state.get_value(&resolved_p).unwrap_or_else(|| {
                                    self.interner.intern_var(resolved_p.clone(), dest_ty.clone())
                                })
                            }
                        } else {
                            state.get_value(&resolved_p).unwrap_or_else(|| {
                                self.interner.intern_var(resolved_p.clone(), dest_ty.clone())
                            })
                        }
                    } else {
                        state.get_value(&resolved_p).unwrap_or_else(|| {
                            self.interner.intern_var(resolved_p.clone(), dest_ty.clone())
                        })
                    }
                } else {
                    state.get_value(&resolved_p).unwrap_or_else(|| {
                        self.interner.intern_var(resolved_p.clone(), dest_ty.clone())
                    })
                };
                let p_iv = self.get_term_interval(val_term, state);
                if p_iv != Interval::FULL {
                    state.refine(val_term, p_iv);
                }
                val_term
            }
            Rvalue::Constant(lit) => {
                let term = self.interner.intern_const(lit.clone());
                if let crate::typecheck::typed_ast::TypedLiteral::Int(v, _) = lit {
                    state.refine(term, Interval::exact(*v));
                }
                term
            }
            Rvalue::BinaryOp(op, l, r) => {
                let l_res = self.resolve_place(l, state);
                let r_res = self.resolve_place(r, state);
                let l_term = state
                    .get_value(&l_res)
                    .unwrap_or_else(|| self.interner.intern_var(l_res, Type::I64));
                let r_term = state
                    .get_value(&r_res)
                    .unwrap_or_else(|| self.interner.intern_var(r_res, Type::I64));
                let res_ty = match op {
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge => Type::Bool,
                    _ => dest_ty.clone(),
                };
                let term = self.interner.intern_binary(*op, l_term, r_term, res_ty);

                let l_iv = self.get_term_interval(l_term, state);
                let r_iv = self.get_term_interval(r_term, state);
                let iv = match op {
                    BinaryOp::Add => {
                        let lo = match (l_iv.lo, r_iv.lo) {
                            (Some(a), Some(b)) => Some(a.saturating_add(b)),
                            _ => None,
                        };
                        let hi = match (l_iv.hi, r_iv.hi) {
                            (Some(a), Some(b)) => Some(a.saturating_add(b)),
                            _ => None,
                        };
                        Interval { lo, hi }
                    }
                    BinaryOp::Sub => {
                        let lo = match (l_iv.lo, r_iv.hi) {
                            (Some(a), Some(b)) => Some(a.saturating_sub(b)),
                            _ => None,
                        };
                        let hi = match (l_iv.hi, r_iv.lo) {
                            (Some(a), Some(b)) => Some(a.saturating_sub(b)),
                            _ => None,
                        };
                        Interval { lo, hi }
                    }
                    BinaryOp::Mul => {
                        let k_opt = match (l_iv.lo, l_iv.hi) {
                            (Some(k1), Some(k2)) if k1 == k2 => Some((k1, &r_iv)),
                            _ => match (r_iv.lo, r_iv.hi) {
                                (Some(k1), Some(k2)) if k1 == k2 => Some((k1, &l_iv)),
                                _ => None,
                            },
                        };

                        if let Some((k, other_iv)) = k_opt {
                            if k == 0 {
                                Interval::exact(0)
                            } else if k > 0 {
                                Interval {
                                    lo: other_iv.lo.map(|v| v.saturating_mul(k)),
                                    hi: other_iv.hi.map(|v| v.saturating_mul(k)),
                                }
                            } else {
                                Interval {
                                    lo: other_iv.hi.map(|v| v.saturating_mul(k)),
                                    hi: other_iv.lo.map(|v| v.saturating_mul(k)),
                                }
                            }
                        } else {
                            Interval::FULL
                        }
                    }
                    _ => Interval::FULL,
                };
                if iv != Interval::FULL {
                    state.refine(term, iv);
                }
                term
            }
            Rvalue::UnaryOp(op, inner) => {
                let in_res = self.resolve_place(inner, state);
                let in_term = state
                    .get_value(&in_res)
                    .unwrap_or_else(|| self.interner.intern_var(in_res, Type::I64));
                let res_ty = match op {
                    UnaryOp::Not => Type::Bool,
                    _ => dest_ty.clone(),
                };
                self.interner.intern_unary(*op, in_term, res_ty)
            }
            Rvalue::Discriminant(p) => {
                let resolved_p = self.resolve_place(p, state);
                let base_place = Place {
                    local: resolved_p.local.clone(),
                    projections: vec![],
                };
                if let Some(base_term_id) = state.get_value(&base_place) {
                    let root_id = state.path_constraints.find_leader(base_term_id);
                    if let SymTerm::Constructor(_, tag, _, _) = self.interner.get(root_id) {
                        self.interner.intern_int(*tag as i64)
                    } else {
                        self.interner.intern_discriminant(root_id)
                    }
                } else {
                    let var_id = self.interner.intern_var(base_place, Type::I64);
                    self.interner.intern_discriminant(var_id)
                }
            }
            Rvalue::EnumVariant {
                enum_name,
                variant_name,
                tag,
                fields,
            } => {
                let f_terms: Vec<SymTermId> = fields
                    .iter()
                    .map(|p| {
                        let p_res = self.resolve_place(p, state);
                        state
                            .get_value(&p_res)
                            .unwrap_or_else(|| self.interner.intern_var(p_res, Type::I64))
                    })
                    .collect();
                self.interner.intern_constructor(
                    variant_name.clone(),
                    *tag,
                    f_terms,
                    Type::Enum(enum_name.clone()),
                )
            }
            Rvalue::Call(callee, args) => {
                let arg_terms: Vec<SymTermId> = args
                    .iter()
                    .map(|p| {
                        let p_res = self.resolve_place(p, state);
                        state
                            .get_value(&p_res)
                            .unwrap_or_else(|| self.interner.intern_var(p_res, Type::I64))
                    })
                    .collect();

                let mut inlined_res = None;
                if self.config.inline_calls {
                    let in_loop_body = self
                        .natural_loops
                        .values()
                        .any(|loop_blocks| loop_blocks.contains(&state.block));

                    let loop_invariance_ok = if in_loop_body && !self.config.inline_loop_body_calls {
                        self.all_args_loop_invariant(&arg_terms, &state.block)
                    } else {
                        true
                    };

                    let callee_count = self.call_stack.iter().filter(|&s| s == callee).count();
                    let depth_ok = self.call_stack.len() < self.config.max_inline_depth
                        && callee_count < self.config.max_inline_depth;
                    let budget_ok = self.nodes.len() < self.config.max_inline_nodes;

                    if loop_invariance_ok && depth_ok && budget_ok {
                        if let Some(callee_func) = self.program_funcs.get(callee).copied() {
                            inlined_res = self.try_drive_interprocedural_call(callee_func, &arg_terms, state);
                            if inlined_res.is_some() {
                                self.stats.calls_inlined += 1;
                            }
                        }
                    }
                }

                inlined_res.unwrap_or_else(|| {
                    self.interner.intern_call(callee.clone(), arg_terms, dest_ty.clone())
                })
            }
            Rvalue::Phi(incoming) => {
                let mut phi_ops = Vec::new();
                for (b, p) in incoming {
                    let p_res = self.resolve_place(p, state);
                    if let Some(t) = state.get_value(&p_res) {
                        phi_ops.push((b.clone(), t));
                    }
                }
                self.interner.intern_phi(phi_ops, dest_ty.clone())
            }
            Rvalue::Alloc(inner_place) => {
                let in_res = self.resolve_place(inner_place, state);
                let inner_term = state
                    .get_value(&in_res)
                    .unwrap_or_else(|| self.interner.intern_var(in_res, Type::I64));
                state.next_heap_id += 1;
                let addr_term = self.interner.intern_ref(inner_term, dest_ty.clone());
                state.symbolic_heap.insert(addr_term, inner_term);
                addr_term
            }
            Rvalue::Load(ptr_place) => {
                let ptr_res = self.resolve_place(ptr_place, state);
                let ptr_term = state
                    .get_value(&ptr_res)
                    .unwrap_or_else(|| self.interner.intern_var(ptr_res, Type::Ptr(Box::new(dest_ty.clone()))));
                state.symbolic_heap.get(&ptr_term).copied().unwrap_or_else(|| {
                    self.interner.intern_deref(ptr_term, dest_ty.clone())
                })
            }
            _ => self.interner.intern_var(dest.clone(), dest_ty),
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
        let header_visits = ancestor_stack
            .iter()
            .filter(|&&anc_id| self.nodes[anc_id.0].state.block == next_state.block)
            .count();

        let should_try_knot = header_visits >= self.config.max_unroll_depth;

        if should_try_knot {
            for &anc_id in ancestor_stack.iter().rev() {
                let anc_state = &self.nodes[anc_id.0].state;
                if is_instance_of(anc_state, &next_state, &self.active_places) {
                    // Knot tied! Fold back to ancestor loop header
                    self.stats.knots_tied += 1;
                    self.nodes[from_id.0].edges.push(ProcessEdge::Knot(anc_id));
                    return;
                }
            }
        }

        // Budget cap check: if node count reaches max_inline_nodes, tie knot to earliest matching ancestor
        if self.nodes.len() >= self.config.max_inline_nodes {
            for &anc_id in ancestor_stack.iter() {
                if self.nodes[anc_id.0].state.block == next_state.block {
                    self.stats.knots_tied += 1;
                    self.nodes[from_id.0].edges.push(ProcessEdge::Knot(anc_id));
                    return;
                }
            }
            if let Some(&first_anc) = ancestor_stack.first() {
                self.stats.knots_tied += 1;
                self.nodes[from_id.0].edges.push(ProcessEdge::Knot(first_anc));
                return;
            }
        }

        // 2. Whistle test: does an ancestor loop header embed next_state or have repeated visits?
        let is_loop_header = self.loop_headers.contains(&next_state.block);
        if is_loop_header && self.config.solve_recurrences {
            let header_visits = ancestor_stack
                .iter()
                .filter(|&&anc_id| self.nodes[anc_id.0].state.block == next_state.block)
                .count();

            if header_visits >= 3 {
                if let Some(&first_anc_id) = ancestor_stack
                    .iter()
                    .find(|&&anc_id| self.nodes[anc_id.0].state.block == next_state.block)
                {
                    let anc_state = self.nodes[first_anc_id.0].state.clone();
                    if let Some(solved_state) =
                        self.try_solve_loop_recurrence(&anc_state, &next_state, ancestor_stack)
                    {
                        self.stats.loops_collapsed += 1;
                        let next_node = self.alloc_node(solved_state);
                        self.nodes[from_id.0].edges.push(ProcessEdge::Step(next_node));
                        ancestor_stack.push(from_id);
                        self.drive_node(next_node, ancestor_stack, depth + 1);
                        ancestor_stack.pop();
                        return;
                    } else if header_visits >= 12 {
                        // Repeated visits but recurrence solver failed: tie knot without partial unrolling
                        for &a_id in ancestor_stack.iter().rev() {
                            let a_st = &self.nodes[a_id.0].state;
                            if is_instance_of(a_st, &next_state, &self.active_places) {
                                self.stats.knots_tied += 1;
                                self.nodes[from_id.0].edges.push(ProcessEdge::Knot(a_id));
                                self.witness.firings.push(WhistleFiring {
                                    from_id,
                                    ancestor_id: a_id,
                                    kind: WhistleKind::HeaderVisitCutoff,
                                });
                                return;
                            }
                        }
                        let mut gen_state = anc_state.clone();
                        let mut next_var_id = self.interner.len();
                        for place in &self.active_places {
                            if let (Some(t_anc), Some(t_curr)) = (anc_state.get_value(place), next_state.get_value(place)) {
                                if t_anc != t_curr {
                                    let gen_res = crate::mir::supercompiler::generalize::most_specific_generalization(
                                        t_anc,
                                        t_curr,
                                        &mut self.interner,
                                        &mut next_var_id,
                                    );
                                    gen_state.set_value(place.clone(), gen_res.common_term);
                                }
                            }
                        }
                        let gen_node_id = self.alloc_node(gen_state);
                        self.stats.knots_tied += 1;
                        self.nodes[from_id.0].edges.push(ProcessEdge::Knot(gen_node_id));
                        self.witness.firings.push(WhistleFiring {
                            from_id,
                            ancestor_id: gen_node_id,
                            kind: WhistleKind::HeaderVisitCutoff,
                        });
                        return;
                    }
                }
            }

            for &anc_id in ancestor_stack.iter().rev() {
                let anc_state = self.nodes[anc_id.0].state.clone();
                if state_embeds(&anc_state, &next_state, &self.active_places, &self.interner) {
                    // Whistle blew! Growth detected across iterations.
                    // Try recurrence solver on mutating induction places:
                    if let Some(solved_state) =
                        self.try_solve_loop_recurrence(&anc_state, &next_state, ancestor_stack)
                    {
                        self.stats.loops_collapsed += 1;
                        let next_node = self.alloc_node(solved_state);
                        self.nodes[from_id.0].edges.push(ProcessEdge::Step(next_node));
                        ancestor_stack.push(from_id);
                        self.drive_node(next_node, ancestor_stack, depth + 1);
                        ancestor_stack.pop();
                        return;
                    } else {
                        // Whistle blew but recurrence solver failed (coupled / modular recurrence)
                        for &a_id in ancestor_stack.iter().rev() {
                            let a_st = &self.nodes[a_id.0].state;
                            if is_instance_of(a_st, &next_state, &self.active_places) {
                                self.stats.knots_tied += 1;
                                self.nodes[from_id.0].edges.push(ProcessEdge::Knot(a_id));
                                self.witness.firings.push(WhistleFiring {
                                    from_id,
                                    ancestor_id: a_id,
                                    kind: WhistleKind::HomeomorphicEmbedding,
                                });
                                return;
                            }
                        }
                        let mut gen_state = anc_state.clone();
                        let mut next_var_id = self.interner.len();
                        for place in &self.active_places {
                            if let (Some(t_anc), Some(t_curr)) = (anc_state.get_value(place), next_state.get_value(place)) {
                                if t_anc != t_curr {
                                    let gen_res = crate::mir::supercompiler::generalize::most_specific_generalization(
                                        t_anc,
                                        t_curr,
                                        &mut self.interner,
                                        &mut next_var_id,
                                    );
                                    gen_state.set_value(place.clone(), gen_res.common_term);
                                }
                            }
                        }
                        let gen_node_id = self.alloc_node(gen_state);
                        self.stats.knots_tied += 1;
                        self.nodes[from_id.0].edges.push(ProcessEdge::Knot(gen_node_id));
                        self.witness.firings.push(WhistleFiring {
                            from_id,
                            ancestor_id: gen_node_id,
                            kind: WhistleKind::HomeomorphicEmbedding,
                        });
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
        // If the loop contains array writes, we cannot collapse it to a scalar closed form
        if let Some(blocks) = self.natural_loops.get(&curr.block) {
            let loop_has_array_writes = blocks.iter().any(|b_id| {
                if let Some(b) = self.block_map.get(b_id) {
                    b.statements.iter().any(|stmt| {
                        let Statement::Assign(dest, _) = stmt;
                        dest.projections.iter().any(|p| matches!(p, Projection::Index(_)))
                    })
                } else {
                    false
                }
            });
            if loop_has_array_writes {
                return None;
            }
        }

        let mut solved_state = curr.clone();
        let mut any_solved = false;

        let mut bound_term = None;
        if let Some(block) = self.block_map.get(&curr.block) {
            if let Terminator::BranchIf { condition, .. } = &block.terminator {
                // First look for the statement defining condition in this block
                for stmt in &block.statements {
                    if let Statement::Assign(dest, Rvalue::BinaryOp(op, l, r)) = stmt {
                        if dest == condition && (*op == BinaryOp::Lt || *op == BinaryOp::Le) {
                            let mut real_l = l.clone();
                            for s in &block.statements {
                                if let Statement::Assign(d, Rvalue::Use(src)) = s {
                                    if d.local == l.local && src.projections.is_empty() {
                                        real_l = src.clone();
                                        break;
                                    }
                                }
                            }
                            let mut real_r = r.clone();
                            for s in &block.statements {
                                if let Statement::Assign(d, Rvalue::Use(src)) = s {
                                    if d.local == r.local && src.projections.is_empty() {
                                        real_r = src.clone();
                                        break;
                                    }
                                }
                            }

                            let mut const_val = None;
                            if let Some(concrete_t) = curr.get_value(&real_r).or_else(|| curr.get_value(r)) {
                                if let SymTerm::ConstInt(v, _) = self.interner.get(concrete_t) {
                                    const_val = Some(*v);
                                }
                            }
                            if const_val.is_none() {
                                for stmt2 in &block.statements {
                                    if let Statement::Assign(d, Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(v, _))) = stmt2 {
                                        if d.local == r.local || d.local == real_r.local {
                                            const_val = Some(*v);
                                            break;
                                        }
                                    }
                                }
                            }
                            if const_val.is_none() {
                                for b in self.block_map.values() {
                                    for stmt2 in &b.statements {
                                        if let Statement::Assign(d, Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(v, _))) = stmt2 {
                                            if d.local == r.local || d.local == real_r.local {
                                                const_val = Some(*v);
                                                break;
                                            }
                                        }
                                    }
                                    if const_val.is_some() {
                                        break;
                                    }
                                }
                            }

                            let iv_initial = anc.get_value(&real_l).or_else(|| anc.get_value(l)).and_then(|t| {
                                if let SymTerm::ConstInt(v, _) = self.interner.get(t) {
                                    Some(*v)
                                } else {
                                    None
                                }
                            }).unwrap_or(0);

                            if let Some(mut v) = const_val {
                                if *op == BinaryOp::Le {
                                    v += 1;
                                }
                                let iters = v - iv_initial;
                                bound_term = Some(self.interner.intern_int(iters));
                            } else {
                                let mut r_term = curr
                                    .get_value(&real_r)
                                    .or_else(|| curr.get_value(r))
                                    .unwrap_or_else(|| self.interner.intern_var(real_r.clone(), Type::I64));
                                let offset = if *op == BinaryOp::Le { 1 } else { 0 } - iv_initial;
                                if offset > 0 {
                                    let off_term = self.interner.intern_int(offset);
                                    r_term = self.interner.intern_binary(BinaryOp::Add, r_term, off_term, Type::I64);
                                } else if offset < 0 {
                                    let off_term = self.interner.intern_int(-offset);
                                    r_term = self.interner.intern_binary(BinaryOp::Sub, r_term, off_term, Type::I64);
                                }
                                bound_term = Some(r_term);
                            }
                            break;
                        }
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

        let mut solved_places = std::collections::HashSet::new();
        let mut solved_acc_place = None;
        if let Some((acc_place, closed_form, iv_place)) = self.try_solve_accumulator_loop(anc, curr, n_term) {
            solved_state.set_value(acc_place.clone(), closed_form);
            solved_state.set_value(iv_place.clone(), n_term);
            solved_places.insert(acc_place.local.clone());
            solved_places.insert(iv_place.local.clone());
            solved_acc_place = Some(acc_place);
            any_solved = true;
        }

        let mut unsolved_places = Vec::new();
        for place in &self.active_places {
            if let Some(ref acc_p) = solved_acc_place {
                if place.local == acc_p.local {
                    continue;
                }
            }

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

            let mut this_solved = false;
            if history.len() >= 3 {
                if let Some(closed_form) = solve_recurrence(&history, n_term, &mut self.interner) {
                    solved_state.set_value(place.clone(), closed_form);
                    any_solved = true;
                    this_solved = true;
                    solved_places.insert(place.local.clone());
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
                                this_solved = true;
                                solved_places.insert(place.local.clone());
                            }
                        }
                    }
                }
            }

            if !this_solved && history.len() >= 4 {
                unsolved_places.push((place.clone(), history));
            }
        }

        let mut nway_solved = false;
        if unsolved_places.len() >= 3 {
            let trajectories: Vec<Vec<i64>> = unsolved_places.iter().map(|(_, h)| h.clone()).collect();
            if let Some(sys) = crate::mir::supercompiler::recurrence::detect_nway_linear_system(&trajectories) {
                let terms = crate::mir::supercompiler::recurrence::solve_nway_recurrence(&sys, n_term, &mut self.interner);
                for (idx, (ref p, _)) in unsolved_places.iter().enumerate() {
                    solved_state.set_value(p.clone(), terms[idx]);
                    solved_places.insert(p.local.clone());
                }
                any_solved = true;
                nway_solved = true;
            } else if unsolved_places.len() <= 8 {
                'subset_search: for sz in (3..unsolved_places.len()).rev() {
                    for combo in get_combinations(unsolved_places.len(), sz) {
                        let sub_trajectories: Vec<Vec<i64>> = combo.iter().map(|&idx| unsolved_places[idx].1.clone()).collect();
                        if let Some(sys) = crate::mir::supercompiler::recurrence::detect_nway_linear_system(&sub_trajectories) {
                            let terms = crate::mir::supercompiler::recurrence::solve_nway_recurrence(&sys, n_term, &mut self.interner);
                            for (t_idx, &u_idx) in combo.iter().enumerate() {
                                let (ref p, _) = unsolved_places[u_idx];
                                solved_state.set_value(p.clone(), terms[t_idx]);
                                solved_places.insert(p.local.clone());
                            }
                            any_solved = true;
                            nway_solved = true;
                            break 'subset_search;
                        }
                    }
                }
            }
        }

        if !nway_solved && unsolved_places.len() >= 2 {
            let n = unsolved_places.len();
            for i in 0..n {
                for j in (i + 1)..n {
                    let (ref p_a, ref hist_a) = unsolved_places[i];
                    let (ref p_b, ref hist_b) = unsolved_places[j];
                    if let Some((term_a, term_b)) = solve_coupled_2var_recurrence(
                        hist_a,
                        hist_b,
                        n_term,
                        &mut self.interner,
                    ) {
                        solved_state.set_value(p_a.clone(), term_a);
                        solved_state.set_value(p_b.clone(), term_b);
                        solved_places.insert(p_a.local.clone());
                        solved_places.insert(p_b.local.clone());
                        any_solved = true;
                        break;
                    } else if let Some(sys) = crate::mir::supercompiler::recurrence::detect_nway_linear_system(&[hist_a.clone(), hist_b.clone()]) {
                        let terms = crate::mir::supercompiler::recurrence::solve_nway_recurrence(&sys, n_term, &mut self.interner);
                        solved_state.set_value(p_a.clone(), terms[0]);
                        solved_state.set_value(p_b.clone(), terms[1]);
                        solved_places.insert(p_a.local.clone());
                        solved_places.insert(p_b.local.clone());
                        any_solved = true;
                        break;
                    }
                }
            }
        }

        let mut mutating_places = Vec::new();
        for place in &self.active_places {
            if let (Some(t_anc), Some(t_curr)) = (anc.get_value(place), curr.get_value(place)) {
                if t_anc != t_curr {
                    mutating_places.push(&place.local);
                }
            }
        }

        let all_mutating_solved = mutating_places.iter().all(|local| solved_places.contains(*local));

        if any_solved && all_mutating_solved {
            Some(solved_state)
        } else {
            None
        }
    }

    fn try_solve_accumulator_loop(
        &mut self,
        base_state: &SymbolicState,
        curr: &SymbolicState,
        n_term: SymTermId,
    ) -> Option<(Place, SymTermId, Place)> {
        let blocks = self.natural_loops.get(&curr.block)?;
        let header_block = self.block_map.get(&curr.block)?;

        // Find loop condition: iv < bound or iv <= bound
        let iv = match &header_block.terminator {
            Terminator::BranchIf { condition, .. } => {
                let mut iv_found = None;
                for stmt in &header_block.statements {
                    if let Statement::Assign(dest, Rvalue::BinaryOp(op, l, _r)) = stmt {
                        if dest == condition && (*op == BinaryOp::Lt || *op == BinaryOp::Le) {
                            let mut base_l = l.clone();
                            for s in &header_block.statements {
                                if let Statement::Assign(d, Rvalue::Use(src)) = s {
                                    if d.local == l.local && src.projections.is_empty() {
                                        base_l = src.clone();
                                        break;
                                    }
                                }
                            }
                            iv_found = Some(base_l);
                            break;
                        }
                    }
                }
                iv_found?
            }
            _ => return None,
        };

        // Find accumulator candidates in loop body blocks (excluding header)
        for place in &self.active_places {
            if place.local == iv.local {
                continue;
            }

            // Check if place is updated as place = place + delta or place = delta + place
            let mut delta_place_opt = None;
            let mut update_count = 0;

            for b_id in blocks {
                if *b_id == curr.block {
                    continue;
                }
                if let Some(b) = self.block_map.get(b_id) {
                    for (idx, stmt) in b.statements.iter().enumerate() {
                        let Statement::Assign(dest, rval) = stmt;
                        if dest.local == place.local {
                            update_count += 1;
                            if let Rvalue::BinaryOp(BinaryOp::Add, l, r) = rval {
                                if is_place_or_alias(l, place, &b.statements, idx) {
                                    delta_place_opt = Some(r.clone());
                                } else if is_place_or_alias(r, place, &b.statements, idx) {
                                    delta_place_opt = Some(l.clone());
                                }
                            }
                        }
                    }
                }
            }

            if update_count != 1 {
                continue;
            }

            let delta_place = match delta_place_opt {
                Some(d) => d,
                None => continue,
            };

            // Get initial value of place and iv from base_state
            let initial_acc = if let Some(t) = base_state.get_value(place) {
                if let SymTerm::ConstInt(val, _) = self.interner.get(t) {
                    *val
                } else {
                    0
                }
            } else {
                0
            };

            let initial_iv = if let Some(t) = base_state.get_value(&iv) {
                if let SymTerm::ConstInt(val, _) = self.interner.get(t) {
                    *val
                } else {
                    0
                }
            } else {
                0
            };

            // Evaluate delta for iterations k = 0..7
            let mut samples = Vec::new();
            let mut running_sum = initial_acc;
            samples.push(running_sum);
            let mut eval_success = true;

            for k in 0..7 {
                let mut env: HashMap<String, i64> = HashMap::new();

                // Populate constants and known values from base_state
                for p in &self.active_places {
                    if let Some(t) = base_state.get_value(p) {
                        if let SymTerm::ConstInt(v, _) = self.interner.get(t) {
                            env.insert(p.local.clone(), *v);
                        }
                    }
                }
                // Bind iv to initial_iv + k
                env.insert(iv.local.clone(), initial_iv + k);

                // Evaluate statements in loop body
                for b_id in blocks {
                    if *b_id == curr.block {
                        continue;
                    }
                    if let Some(b) = self.block_map.get(b_id) {
                        for stmt in &b.statements {
                            let Statement::Assign(dest, rval) = stmt;
                            if dest.local == place.local {
                                continue;
                            }
                            let val_opt = match rval {
                                Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(v, _)) => Some(*v),
                                Rvalue::Use(p) => env.get(&p.local).copied(),
                                Rvalue::BinaryOp(op, l, r) => {
                                    if let (Some(&lv), Some(&rv)) = (env.get(&l.local), env.get(&r.local)) {
                                        match op {
                                            BinaryOp::Add => Some(lv.wrapping_add(rv)),
                                            BinaryOp::Sub => Some(lv.wrapping_sub(rv)),
                                            BinaryOp::Mul => Some(lv.wrapping_mul(rv)),
                                            BinaryOp::Div => if rv != 0 { Some(lv / rv) } else { None },
                                            BinaryOp::Mod => if rv != 0 { Some(lv % rv) } else { None },
                                            BinaryOp::BitAnd => Some(lv & rv),
                                            BinaryOp::BitOr => Some(lv | rv),
                                            BinaryOp::BitXor => Some(lv ^ rv),
                                            BinaryOp::Shl => Some(lv << (rv as u32 % 64)),
                                            BinaryOp::Shr => Some(lv >> (rv as u32 % 64)),
                                            _ => None,
                                        }
                                    } else {
                                        None
                                    }
                                }
                                Rvalue::UnaryOp(crate::ast::UnaryOp::Neg, p) => env.get(&p.local).map(|v| -v),
                                _ => None,
                            };
                            if let Some(v) = val_opt {
                                env.insert(dest.local.clone(), v);
                            }
                        }
                    }
                }

                if let Some(&delta_val) = env.get(&delta_place.local) {
                    running_sum = running_sum.wrapping_add(delta_val);
                    samples.push(running_sum);
                } else {
                    eval_success = false;
                    break;
                }
            }

            if eval_success && samples.len() >= 4 {
                if let Some(closed_form) = solve_recurrence(&samples, n_term, &mut self.interner) {
                    return Some((place.clone(), closed_form, iv));
                }
            }
        }

        None
    }

    fn all_args_loop_invariant(&self, arg_terms: &[SymTermId], block: &BasicBlockId) -> bool {
        let mut loop_blocks = std::collections::HashSet::new();
        for blocks in self.natural_loops.values() {
            if blocks.contains(block) {
                loop_blocks.extend(blocks.iter().cloned());
            }
        }

        if loop_blocks.is_empty() {
            return true;
        }

        let mut assigned_places = std::collections::HashSet::new();
        for b_id in &loop_blocks {
            if let Some(b) = self.block_map.get(b_id) {
                for stmt in &b.statements {
                    match stmt {
                        Statement::Assign(dest, _) => {
                            assigned_places.insert(dest.local.clone());
                        }
                    }
                }
            }
        }

        for &arg in arg_terms {
            if self.term_references_locals(arg, &assigned_places) {
                return false;
            }
        }

        true
    }

    fn term_references_locals(&self, term_id: SymTermId, locals: &std::collections::HashSet<String>) -> bool {
        match self.interner.get(term_id) {
            SymTerm::Var(p, _) => locals.contains(&p.local),
            SymTerm::Binary(_, l, r, _) => {
                self.term_references_locals(*l, locals) || self.term_references_locals(*r, locals)
            }
            SymTerm::Unary(_, inner, _) => self.term_references_locals(*inner, locals),
            SymTerm::Constructor(_, _, fields, _) => {
                fields.iter().any(|&f| self.term_references_locals(f, locals))
            }
            SymTerm::Select(c, t, e, _) => {
                self.term_references_locals(*c, locals)
                    || self.term_references_locals(*t, locals)
                    || self.term_references_locals(*e, locals)
            }
            SymTerm::Phi(incoming, _) => {
                incoming.iter().any(|(_, t)| self.term_references_locals(*t, locals))
            }
            SymTerm::Call(_, args, _) => {
                args.iter().any(|&a| self.term_references_locals(a, locals))
            }
            SymTerm::Ref(inner, _) => self.term_references_locals(*inner, locals),
            SymTerm::Deref(ptr, _) => self.term_references_locals(*ptr, locals),
            SymTerm::Discriminant(inner, _) => self.term_references_locals(*inner, locals),
            _ => false,
        }
    }

    fn try_drive_interprocedural_call(
        &mut self,
        callee: &'a MirFunction,
        args: &[SymTermId],
        caller_state: &SymbolicState,
    ) -> Option<SymTermId> {
        if callee.blocks.is_empty() || callee.params.len() != args.len() {
            return None;
        }

        let entry_id = callee.blocks[0].id.clone();
        let mut initial_state = SymbolicState::new(entry_id, MemoryVersionId::LIVE_ON_ENTRY);

        // Bind parameters to argument terms and propagate caller's refinements
        for ((param_name, _), &arg_term) in callee.params.iter().zip(args.iter()) {
            let p = Place {
                local: param_name.clone(),
                projections: vec![],
            };
            initial_state.set_value(p, arg_term);
            let arg_iv = self.get_term_interval(arg_term, caller_state);
            if arg_iv != Interval::FULL {
                initial_state.refine(arg_term, arg_iv);
            }
        }

        let mut child_call_stack = self.call_stack.clone();
        child_call_stack.push(callee.name.clone());

        let mut child_driver = SupercompilerDriver::new(callee)
            .with_config(self.config)
            .with_program_functions_map(self.program_funcs.clone())
            .with_call_stack(child_call_stack);

        // Seed child interner with current terms
        child_driver.interner = self.interner.clone();

        let child_tree = child_driver.run_with_initial_state(initial_state);

        self.stats.branches_pruned += child_tree.stats.branches_pruned;
        self.stats.sc_bce_eliminated += child_tree.stats.sc_bce_eliminated;

        let mut return_terms = Vec::new();
        for node in &child_tree.nodes {
            if let Some(ret) = node.return_term {
                return_terms.push(ret);
            }
        }

        if return_terms.len() == 1 {
            let child_ret = return_terms[0];
            Some(self.interner.import_from(&child_tree.interner, child_ret))
        } else if !return_terms.is_empty() {
            let first = return_terms[0];
            let all_same = return_terms.iter().all(|&t| {
                child_tree.interner.get(t) == child_tree.interner.get(first)
            });
            if all_same {
                Some(self.interner.import_from(&child_tree.interner, first))
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub fn evaluate_comparison_intervals(op: BinaryOp, l_iv: &Interval, r_iv: &Interval) -> Option<bool> {
    match op {
        BinaryOp::Lt => {
            // l < r
            if let (Some(l_hi), Some(r_lo)) = (l_iv.hi, r_iv.lo) {
                if l_hi < r_lo {
                    return Some(true);
                }
            }
            if let (Some(l_lo), Some(r_hi)) = (l_iv.lo, r_iv.hi) {
                if l_lo >= r_hi {
                    return Some(false);
                }
            }
            None
        }
        BinaryOp::Le => {
            if let (Some(l_hi), Some(r_lo)) = (l_iv.hi, r_iv.lo) {
                if l_hi <= r_lo {
                    return Some(true);
                }
            }
            if let (Some(l_lo), Some(r_hi)) = (l_iv.lo, r_iv.hi) {
                if l_lo > r_hi {
                    return Some(false);
                }
            }
            None
        }
        BinaryOp::Gt => evaluate_comparison_intervals(BinaryOp::Lt, r_iv, l_iv),
        BinaryOp::Ge => evaluate_comparison_intervals(BinaryOp::Le, r_iv, l_iv),
        BinaryOp::Eq => {
            if let (Some(l_lo), Some(l_hi), Some(r_lo), Some(r_hi)) = (l_iv.lo, l_iv.hi, r_iv.lo, r_iv.hi) {
                if l_lo == l_hi && r_lo == r_hi && l_lo == r_lo {
                    return Some(true);
                }
                if l_hi < r_lo || l_lo > r_hi {
                    return Some(false);
                }
            } else {
                if let (Some(l_hi), Some(r_lo)) = (l_iv.hi, r_iv.lo) {
                    if l_hi < r_lo {
                        return Some(false);
                    }
                }
                if let (Some(l_lo), Some(r_hi)) = (l_iv.lo, r_iv.hi) {
                    if l_lo > r_hi {
                        return Some(false);
                    }
                }
            }
            None
        }
        BinaryOp::Ne => evaluate_comparison_intervals(BinaryOp::Eq, l_iv, r_iv).map(|eq| !eq),
        _ => None,
    }
}

pub fn narrow_condition_intervals(
    cond: SymTermId,
    is_true: bool,
    state: &mut SymbolicState,
    interner: &TermInterner,
) {
    let cond_root = state.path_constraints.find_leader(cond);
    if let SymTerm::Binary(op, left, right, _) = interner.get(cond_root) {
        let l = state.path_constraints.find_leader(*left);
        let r = state.path_constraints.find_leader(*right);

        let l_const = match interner.get(l) {
            SymTerm::ConstInt(v, _) => Some(*v),
            _ => None,
        };
        let r_const = match interner.get(r) {
            SymTerm::ConstInt(v, _) => Some(*v),
            _ => None,
        };

        if let Some(k) = r_const {
            narrow_single_var(l, *op, k, is_true, state);
        } else if let Some(k) = l_const {
            if let Some(flipped) = flip_relational_op(*op) {
                narrow_single_var(r, flipped, k, is_true, state);
            }
        } else {
            narrow_two_vars(l, r, *op, is_true, state);
        }
    }
}

fn flip_relational_op(op: BinaryOp) -> Option<BinaryOp> {
    match op {
        BinaryOp::Lt => Some(BinaryOp::Gt),
        BinaryOp::Le => Some(BinaryOp::Ge),
        BinaryOp::Gt => Some(BinaryOp::Lt),
        BinaryOp::Ge => Some(BinaryOp::Le),
        BinaryOp::Eq => Some(BinaryOp::Eq),
        BinaryOp::Ne => Some(BinaryOp::Ne),
        _ => None,
    }
}

fn invert_relational_op(op: BinaryOp) -> Option<BinaryOp> {
    match op {
        BinaryOp::Lt => Some(BinaryOp::Ge),
        BinaryOp::Le => Some(BinaryOp::Gt),
        BinaryOp::Gt => Some(BinaryOp::Le),
        BinaryOp::Ge => Some(BinaryOp::Lt),
        BinaryOp::Eq => Some(BinaryOp::Ne),
        BinaryOp::Ne => Some(BinaryOp::Eq),
        _ => None,
    }
}

fn narrow_single_var(x: SymTermId, op: BinaryOp, k: i64, is_true: bool, state: &mut SymbolicState) {
    let eff_op = if is_true {
        op
    } else if let Some(inv) = invert_relational_op(op) {
        inv
    } else {
        return;
    };

    match eff_op {
        BinaryOp::Lt => {
            state.refine(x, Interval { lo: None, hi: Some(k.saturating_sub(1)) });
        }
        BinaryOp::Le => {
            state.refine(x, Interval { lo: None, hi: Some(k) });
        }
        BinaryOp::Gt => {
            state.refine(x, Interval { lo: Some(k.saturating_add(1)), hi: None });
        }
        BinaryOp::Ge => {
            state.refine(x, Interval { lo: Some(k), hi: None });
        }
        BinaryOp::Eq => {
            state.refine(x, Interval::exact(k));
        }
        BinaryOp::Ne => {
            let cur = state.get_refinement(x);
            if cur.lo == Some(k) {
                state.refine(x, Interval { lo: Some(k.saturating_add(1)), hi: cur.hi });
            } else if cur.hi == Some(k) {
                state.refine(x, Interval { lo: cur.lo, hi: Some(k.saturating_sub(1)) });
            }
        }
        _ => {}
    }
}

fn narrow_two_vars(l: SymTermId, r: SymTermId, op: BinaryOp, is_true: bool, state: &mut SymbolicState) {
    let eff_op = if is_true {
        op
    } else if let Some(inv) = invert_relational_op(op) {
        inv
    } else {
        return;
    };

    let l_iv = state.get_refinement(l);
    let r_iv = state.get_refinement(r);

    match eff_op {
        BinaryOp::Lt => {
            if let Some(r_hi) = r_iv.hi {
                state.refine(l, Interval { lo: None, hi: Some(r_hi.saturating_sub(1)) });
            }
            if let Some(l_lo) = l_iv.lo {
                state.refine(r, Interval { lo: Some(l_lo.saturating_add(1)), hi: None });
            }
        }
        BinaryOp::Le => {
            if let Some(r_hi) = r_iv.hi {
                state.refine(l, Interval { lo: None, hi: Some(r_hi) });
            }
            if let Some(l_lo) = l_iv.lo {
                state.refine(r, Interval { lo: Some(l_lo), hi: None });
            }
        }
        BinaryOp::Gt => {
            narrow_two_vars(r, l, BinaryOp::Lt, true, state);
        }
        BinaryOp::Ge => {
            narrow_two_vars(r, l, BinaryOp::Le, true, state);
        }
        BinaryOp::Eq => {
            let meet = l_iv.meet(&r_iv);
            state.refine(l, meet.clone());
            state.refine(r, meet);
        }
        _ => {}
    }
}

fn is_place_or_alias(p: &Place, target: &Place, stmts: &[Statement], idx: usize) -> bool {
    if p.local == target.local && p.projections == target.projections {
        return true;
    }
    for s in &stmts[..idx] {
        if let Statement::Assign(dest, Rvalue::Use(src)) = s {
            if dest.local == p.local && src.local == target.local && src.projections == target.projections {
                return true;
            }
        }
    }
    false
}

fn get_combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    let mut current = Vec::new();
    fn backtrack(start: usize, n: usize, k: usize, current: &mut Vec<usize>, result: &mut Vec<Vec<usize>>) {
        if current.len() == k {
            result.push(current.clone());
            return;
        }
        for i in start..n {
            current.push(i);
            backtrack(i + 1, n, k, current, result);
            current.pop();
        }
    }
    backtrack(0, n, k, &mut current, &mut result);
    result
}
