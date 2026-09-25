use std::collections::HashMap;
use std::fmt;

use crate::ast::BinaryOp;
use super::generalize::{solve_coupled_2var_recurrence, solve_recurrence};
use super::state::SymbolicState;
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
    natural_loops: HashMap<BasicBlockId, std::collections::HashSet<BasicBlockId>>,
    next_node_id: usize,
    max_depth: usize,
    active_places: Vec<Place>,
    stats: SupercompilerStats,
    program_funcs: HashMap<String, &'a MirFunction>,
    call_stack: Vec<String>,
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
            call_stack: Vec::new(),
        }
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

        self.run_with_initial_state(initial_state)
    }

    pub fn run_with_initial_state(mut self, initial_state: SymbolicState) -> ProcessTree {
        if self.func.blocks.is_empty() {
            return ProcessTree {
                nodes: Vec::new(),
                root: ProcessNodeId(0),
                interner: self.interner,
                stats: self.stats,
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
            Terminator::Switch {
                value,
                targets,
                default,
            } => {
                let val_term = working_state
                    .get_value(value)
                    .unwrap_or_else(|| self.interner.intern_var(value.clone(), Type::I64));
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

    fn drive_statement(&mut self, stmt: &Statement, state: &mut SymbolicState) {
        let Statement::Assign(dest, rval) = stmt;
        let term = match rval {
            Rvalue::Use(p) => {
                if let Some(Projection::Payload(i)) = p.projections.first() {
                    let base_place = Place {
                        local: p.local.clone(),
                        projections: vec![],
                    };
                    if let Some(base_term_id) = state.get_value(&base_place) {
                        let root_id = state.path_constraints.find_leader(base_term_id);
                        if let SymTerm::Constructor(_, _, fields, _) = self.interner.get(root_id) {
                            if let Some(&f_term) = fields.get(*i) {
                                f_term
                            } else {
                                state.get_value(p).unwrap_or_else(|| {
                                    self.interner.intern_var(p.clone(), Type::I64)
                                })
                            }
                        } else {
                            state.get_value(p).unwrap_or_else(|| {
                                self.interner.intern_var(p.clone(), Type::I64)
                            })
                        }
                    } else {
                        state.get_value(p).unwrap_or_else(|| {
                            self.interner.intern_var(p.clone(), Type::I64)
                        })
                    }
                } else {
                    state.get_value(p).unwrap_or_else(|| {
                        self.interner.intern_var(p.clone(), Type::I64)
                    })
                }
            }
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
            Rvalue::Discriminant(p) => {
                let base_place = Place {
                    local: p.local.clone(),
                    projections: vec![],
                };
                if let Some(base_term_id) = state.get_value(&base_place) {
                    let root_id = state.path_constraints.find_leader(base_term_id);
                    if let SymTerm::Constructor(_, tag, _, _) = self.interner.get(root_id) {
                        self.interner.intern_int(*tag as i64)
                    } else {
                        self.interner.intern_var(dest.clone(), Type::I64)
                    }
                } else {
                    self.interner.intern_var(dest.clone(), Type::I64)
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
                        state
                            .get_value(p)
                            .unwrap_or_else(|| self.interner.intern_var(p.clone(), Type::I64))
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
                        state
                            .get_value(p)
                            .unwrap_or_else(|| self.interner.intern_var(p.clone(), Type::I64))
                    })
                    .collect();

                let mut inlined_res = None;
                if let Some(callee_func) = self.program_funcs.get(callee).copied() {
                    if !self.call_stack.contains(callee) && self.call_stack.len() < 8 {
                        inlined_res = self.try_drive_interprocedural_call(callee_func, &arg_terms);
                    }
                }

                inlined_res.unwrap_or_else(|| {
                    self.interner.intern_call(callee.clone(), arg_terms, Type::I64)
                })
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
            Rvalue::Alloc(inner_place) => {
                let inner_term = state
                    .get_value(inner_place)
                    .unwrap_or_else(|| self.interner.intern_var(inner_place.clone(), Type::I64));
                state.next_heap_id += 1;
                let addr_term = self.interner.intern_ref(inner_term, Type::Ptr(Box::new(Type::I64)));
                state.symbolic_heap.insert(addr_term, inner_term);
                addr_term
            }
            Rvalue::Load(ptr_place) => {
                let ptr_term = state
                    .get_value(ptr_place)
                    .unwrap_or_else(|| self.interner.intern_var(ptr_place.clone(), Type::Ptr(Box::new(Type::I64))));
                state.symbolic_heap.get(&ptr_term).copied().unwrap_or_else(|| {
                    self.interner.intern_deref(ptr_term, Type::I64)
                })
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

        // 2. Whistle test: does an ancestor loop header embed next_state or have repeated visits?
        let is_loop_header = self.loop_headers.contains(&next_state.block);
        if is_loop_header {
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

        // Try direct accumulator fold pattern recognition FIRST
        let mut solved_acc_place = None;
        if let Some((acc_place, closed_form, iv_place)) = self.try_solve_accumulator_loop(anc, curr, n_term) {
            solved_state.set_value(acc_place.clone(), closed_form);
            solved_state.set_value(iv_place, n_term);
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
                            }
                        }
                    }
                }
            }

            if !this_solved && history.len() >= 4 {
                unsolved_places.push((place.clone(), history));
            }
        }

        if unsolved_places.len() >= 2 {
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
                        any_solved = true;
                        break;
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

    fn try_drive_interprocedural_call(
        &mut self,
        callee: &'a MirFunction,
        args: &[SymTermId],
    ) -> Option<SymTermId> {
        if callee.blocks.is_empty() || callee.params.len() != args.len() {
            return None;
        }

        let entry_id = callee.blocks[0].id.clone();
        let mut initial_state = SymbolicState::new(entry_id, MemoryVersionId::LIVE_ON_ENTRY);

        // Bind parameters to argument terms
        for ((param_name, _), &arg_term) in callee.params.iter().zip(args.iter()) {
            let p = Place {
                local: param_name.clone(),
                projections: vec![],
            };
            initial_state.set_value(p, arg_term);
        }

        let mut child_call_stack = self.call_stack.clone();
        child_call_stack.push(callee.name.clone());

        let mut child_driver = SupercompilerDriver::new(callee)
            .with_program_functions_map(self.program_funcs.clone())
            .with_call_stack(child_call_stack);

        // Seed child interner with current terms
        child_driver.interner = self.interner.clone();

        let child_tree = child_driver.run_with_initial_state(initial_state);

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
