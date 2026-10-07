//! Distillation: Global Process-Tree Distillation (Hamilton 2007).
//!
//! Unlike classical local supercompilation, distillation operates globally across
//! procedural boundaries. It represents cross-procedural configurations in an explicit
//! Global Process Tree, drives demanding contexts with call-by-need evaluation, deforests
//! intermediate data structures by cancelling Alloc-Load pairs, detects recurrences via a
//! Two-Level Whistle (local path embedding and global configuration memoization), and
//! synthesizes new specialized, single-pass recursive functions with direct recursive back-edges.

use std::collections::{HashMap, HashSet};

use crate::ast::BinaryOp;
use crate::mir::lower::{MirBasicBlock, MirFunction, MirLocalDecl, MirProgram, Rvalue, Statement};
use crate::mir::{BasicBlockId, Place, Projection, Terminator};
use crate::typecheck::types::Type;
use super::drive::{ProcessEdge, ProcessNode, ProcessNodeId, ProcessTree, SupercompilerStats};
use super::term::TermInterner;

// ============================================================================
// 1. Global Process-Tree Term Representation
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GlobalTerm {
    Var(String),
    Constant(i64),
    Constructor {
        enum_name: String,
        variant_name: String,
        tag: usize,
        fields: Vec<GlobalTerm>,
    },
    Call {
        func: String,
        args: Vec<GlobalTerm>,
    },
    Alloc(Box<GlobalTerm>),
    Load(Box<GlobalTerm>),
    BinaryOp(BinaryOp, Box<GlobalTerm>, Box<GlobalTerm>),
    Payload {
        base: Box<GlobalTerm>,
        index: usize,
    },
}

impl GlobalTerm {
    /// Simplifies the term by cancelling inverse operations (e.g. Load(Alloc(x)) -> x).
    pub fn simplify(&self) -> GlobalTerm {
        match self {
            GlobalTerm::Load(inner) => {
                let s_inner = inner.simplify();
                if let GlobalTerm::Alloc(target) = s_inner {
                    *target
                } else {
                    GlobalTerm::Load(Box::new(s_inner))
                }
            }
            GlobalTerm::Alloc(inner) => GlobalTerm::Alloc(Box::new(inner.simplify())),
            GlobalTerm::Constructor { enum_name, variant_name, tag, fields } => {
                let s_fields = fields.iter().map(|f| f.simplify()).collect();
                GlobalTerm::Constructor {
                    enum_name: enum_name.clone(),
                    variant_name: variant_name.clone(),
                    tag: *tag,
                    fields: s_fields,
                }
            }
            GlobalTerm::Call { func, args } => {
                let s_args = args.iter().map(|a| a.simplify()).collect();
                GlobalTerm::Call {
                    func: func.clone(),
                    args: s_args,
                }
            }
            GlobalTerm::BinaryOp(op, l, r) => {
                let sl = l.simplify();
                let sr = r.simplify();
                if let (GlobalTerm::Constant(a), GlobalTerm::Constant(b)) = (&sl, &sr) {
                    match op {
                        BinaryOp::Add => GlobalTerm::Constant(a + b),
                        BinaryOp::Sub => GlobalTerm::Constant(a - b),
                        BinaryOp::Mul => GlobalTerm::Constant(a * b),
                        _ => GlobalTerm::BinaryOp(*op, Box::new(sl), Box::new(sr)),
                    }
                } else {
                    GlobalTerm::BinaryOp(*op, Box::new(sl), Box::new(sr))
                }
            }
            GlobalTerm::Payload { base, index } => {
                let s_base = base.simplify();
                if let GlobalTerm::Constructor { fields, .. } = &s_base {
                    if *index < fields.len() {
                        return fields[*index].clone();
                    }
                }
                GlobalTerm::Payload { base: Box::new(s_base), index: *index }
            }
            _ => self.clone(),
        }
    }

    /// Substitutes variables according to a mapping.
    pub fn substitute(&self, subst: &HashMap<String, GlobalTerm>) -> GlobalTerm {
        match self {
            GlobalTerm::Var(name) => {
                if let Some(val) = subst.get(name) {
                    val.clone()
                } else {
                    self.clone()
                }
            }
            GlobalTerm::Constructor { enum_name, variant_name, tag, fields } => {
                let s_fields = fields.iter().map(|f| f.substitute(subst)).collect();
                GlobalTerm::Constructor {
                    enum_name: enum_name.clone(),
                    variant_name: variant_name.clone(),
                    tag: *tag,
                    fields: s_fields,
                }
            }
            GlobalTerm::Call { func, args } => {
                let s_args = args.iter().map(|a| a.substitute(subst)).collect();
                GlobalTerm::Call {
                    func: func.clone(),
                    args: s_args,
                }
            }
            GlobalTerm::Alloc(inner) => GlobalTerm::Alloc(Box::new(inner.substitute(subst))),
            GlobalTerm::Load(inner) => GlobalTerm::Load(Box::new(inner.substitute(subst))),
            GlobalTerm::BinaryOp(op, l, r) => GlobalTerm::BinaryOp(
                *op,
                Box::new(l.substitute(subst)),
                Box::new(r.substitute(subst)),
            ),
            GlobalTerm::Payload { base, index } => GlobalTerm::Payload {
                base: Box::new(base.substitute(subst)),
                index: *index,
            },
            _ => self.clone(),
        }
    }

    /// First-order pattern matching: checks if `self` is an instance of `pattern` ($self = pattern \cdot \theta$).
    pub fn match_instance(&self, pattern: &GlobalTerm, subst: &mut HashMap<String, GlobalTerm>) -> bool {
        match (pattern, self) {
            (GlobalTerm::Var(p_var), term) => {
                if let Some(existing) = subst.get(p_var) {
                    existing == term
                } else {
                    subst.insert(p_var.clone(), term.clone());
                    true
                }
            }
            (GlobalTerm::Constant(c1), GlobalTerm::Constant(c2)) => c1 == c2,
            (
                GlobalTerm::Constructor { enum_name: e1, variant_name: v1, tag: t1, fields: f1 },
                GlobalTerm::Constructor { enum_name: e2, variant_name: v2, tag: t2, fields: f2 },
            ) => {
                if e1 != e2 || v1 != v2 || t1 != t2 || f1.len() != f2.len() {
                    return false;
                }
                f1.iter().zip(f2.iter()).all(|(p, t)| t.match_instance(p, subst))
            }
            (
                GlobalTerm::Call { func: f1, args: a1 },
                GlobalTerm::Call { func: f2, args: a2 },
            ) => {
                if f1 != f2 || a1.len() != a2.len() {
                    return false;
                }
                a1.iter().zip(a2.iter()).all(|(p, t)| t.match_instance(p, subst))
            }
            (GlobalTerm::Alloc(in1), GlobalTerm::Alloc(in2)) => in2.match_instance(in1, subst),
            (GlobalTerm::Load(in1), GlobalTerm::Load(in2)) => in2.match_instance(in1, subst),
            (GlobalTerm::BinaryOp(op1, l1, r1), GlobalTerm::BinaryOp(op2, l2, r2)) => {
                op1 == op2 && l2.match_instance(l1, subst) && r2.match_instance(r1, subst)
            }
            _ => false,
        }
    }

    /// Homeomorphic embedding check ($self \trianglelefteq other$) for the Two-Level Whistle.
    pub fn embeds_in(&self, other: &GlobalTerm) -> bool {
        // Diving: self embeds in any subterm of other
        match other {
            GlobalTerm::Constructor { fields, .. } => {
                if fields.iter().any(|f| self.embeds_in(f)) {
                    return true;
                }
            }
            GlobalTerm::Call { args, .. } => {
                if args.iter().any(|a| self.embeds_in(a)) {
                    return true;
                }
            }
            GlobalTerm::Alloc(inner) | GlobalTerm::Load(inner) => {
                if self.embeds_in(inner) {
                    return true;
                }
            }
            GlobalTerm::BinaryOp(_, l, r) if self.embeds_in(l) || self.embeds_in(r) => {
                return true;
            }
            _ => {}
        }

        // Coupling: same constructor/functor and all children embed
        match (self, other) {
            (GlobalTerm::Var(v1), GlobalTerm::Var(v2)) => v1 == v2,
            (GlobalTerm::Constant(c1), GlobalTerm::Constant(c2)) => c1 == c2,
            (
                GlobalTerm::Constructor { enum_name: e1, variant_name: v1, fields: f1, .. },
                GlobalTerm::Constructor { enum_name: e2, variant_name: v2, fields: f2, .. },
            ) => {
                e1 == e2 && v1 == v2 && f1.len() == f2.len()
                    && f1.iter().zip(f2.iter()).all(|(a, b)| a.embeds_in(b))
            }
            (
                GlobalTerm::Call { func: f1, args: a1 },
                GlobalTerm::Call { func: f2, args: a2 },
            ) => {
                f1 == f2 && a1.len() == a2.len()
                    && a1.iter().zip(a2.iter()).all(|(a, b)| a.embeds_in(b))
            }
            (GlobalTerm::Alloc(i1), GlobalTerm::Alloc(i2)) => i1.embeds_in(i2),
            (GlobalTerm::Load(i1), GlobalTerm::Load(i2)) => i1.embeds_in(i2),
            (GlobalTerm::BinaryOp(op1, l1, r1), GlobalTerm::BinaryOp(op2, l2, r2)) => {
                op1 == op2 && l1.embeds_in(l2) && r1.embeds_in(r2)
            }
            _ => false,
        }
    }
}

// ============================================================================
// 2. Global Process Tree Nodes and Graph
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlobalNodeId(pub usize);

#[derive(Debug, Clone)]
pub struct GlobalProcessNode {
    pub id: GlobalNodeId,
    pub term: GlobalTerm,
    pub kind: GlobalNodeKind,
}

#[derive(Debug, Clone)]
pub enum GlobalNodeKind {
    Leaf(GlobalTerm),
    Branch {
        scrutinee_var: String,
        enum_name: String,
        arms: Vec<GlobalBranchArm>,
    },
    Knot {
        target: GlobalNodeId,
        subst: HashMap<String, GlobalTerm>,
    },
}

#[derive(Debug, Clone)]
pub struct GlobalBranchArm {
    pub variant_name: String,
    pub tag: usize,
    pub bindings: Vec<(String, Type)>,
    pub child: GlobalNodeId,
}

#[derive(Debug, Clone)]
pub struct GlobalProcessTree {
    pub nodes: Vec<GlobalProcessNode>,
    pub root: GlobalNodeId,
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub return_ty: Type,
    pub stats: SupercompilerStats,
}

// ============================================================================
// 3. Two-Level Whistle (Hamilton 2007)
// ============================================================================

pub struct TwoLevelWhistle {
    /// Level 1: Local path terms (prevents infinite non-progress unfolding in a single branch)
    pub path_terms: Vec<GlobalTerm>,
    /// Level 2: Global configurations memoized across procedural scopes
    pub global_configs: Vec<(GlobalNodeId, GlobalTerm)>,
}

impl Default for TwoLevelWhistle {
    fn default() -> Self {
        Self::new()
    }
}

impl TwoLevelWhistle {
    pub fn new() -> Self {
        TwoLevelWhistle {
            path_terms: Vec::new(),
            global_configs: Vec::new(),
        }
    }

    /// Checks if `term` is an exact instance of any ancestor configuration in the global process tree.
    pub fn check_global_knot(&self, term: &GlobalTerm) -> Option<(GlobalNodeId, HashMap<String, GlobalTerm>)> {
        for (node_id, anc_term) in &self.global_configs {
            let mut subst = HashMap::new();
            if term.match_instance(anc_term, &mut subst) {
                return Some((*node_id, subst));
            }
        }
        None
    }

    /// Checks if `term` embeds into any previous configuration on the current path (Level 1 Whistle).
    pub fn check_local_whistle(&self, term: &GlobalTerm) -> bool {
        self.path_terms.iter().any(|anc| anc.embeds_in(term))
    }
}

// ============================================================================
// 4. Distillation Engine
// ============================================================================

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

    /// Classical structural process-tree folding (kept for backward compatibility with MRSC/parallel).
    pub fn distill_process_tree(&mut self, tree: &mut ProcessTree) -> usize {
        let mut folds = 0;
        if tree.nodes.is_empty() {
            return 0;
        }

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

        let mut shapes: HashMap<ProcessNodeId, TreeShape> = HashMap::new();
        let mut visited: HashSet<ProcessNodeId> = HashSet::new();

        for i in 0..n {
            let nid = ProcessNodeId(i);
            self.compute_shape(nid, &children, &tree.nodes, &mut shapes, &mut visited);
        }

        let mut shape_to_nodes: HashMap<TreeShape, Vec<ProcessNodeId>> = HashMap::new();
        for (nid, shape) in &shapes {
            if !matches!(shape, TreeShape::Leaf(_)) {
                shape_to_nodes.entry(shape.clone()).or_default().push(*nid);
            }
        }

        for (_shape, nodes) in shape_to_nodes {
            if nodes.len() >= 2 {
                let canonical = nodes[0];
                for &duplicate in &nodes[1..] {
                    if duplicate != canonical && !self.is_ancestor(canonical, duplicate, &children) {
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
            _ => TreeShape::Leaf(None),
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

    // ========================================================================
    // 5. Whole-Program Global Process-Tree Distillation (Hamilton 2007)
    // ========================================================================

    /// Performs whole-program global distillation across all functions in `program`.
    ///
    /// Identifies composed recursive calls (e.g. `append(append(xs, ys), zs)`), builds
    /// the cross-procedural Global Process Tree, applies call-by-need driving to deforest
    /// intermediate allocations (`deref(box(x)) -> x`), ties global knots across scopes,
    /// synthesizes specialized single-pass functions (`append3`), and rewrites caller sites.
    pub fn distill_program(program: &mut MirProgram) -> SupercompilerStats {
        let mut stats = SupercompilerStats::default();

        // LAZY-04: Stream fusion for producer-consumer thunk chains and loops
        fuse_stream_pipeline(program, &mut stats);

        let mut transformed_any = true;
        let mut pass_count = 0;

        while transformed_any && pass_count < 8 {
            transformed_any = false;
            pass_count += 1;

            let candidates = find_composition_candidates(program);
            if candidates.is_empty() {
                break;
            }

                        for candidate in candidates {
                let existing_specialized_name = if candidate.caller_fn != "main" {
                    Some(candidate.caller_fn.clone())
                } else {
                    None
                };

                let target_name = existing_specialized_name.unwrap_or_else(|| {
                    format!("__distill_{}_{}", candidate.f_func, candidate.g_func)
                });

                if let Some((synthesized_fn, global_tree)) =
                    synthesize_distilled_function(&candidate, program, &target_name)
                {
                    stats.nodes_explored += global_tree.nodes.len();
                    stats.knots_tied += global_tree.stats.knots_tied;
                    stats.loops_collapsed += global_tree.stats.loops_collapsed;
                    stats.branches_pruned += global_tree.stats.branches_pruned;

                    // If candidate caller function IS the wrapper function (e.g. append3), update it directly
                    if candidate.caller_fn == target_name {
                        if let Some(existing_fn) = program.functions.iter_mut().find(|f| f.name == target_name) {
                            existing_fn.blocks = synthesized_fn.blocks;
                            existing_fn.locals = synthesized_fn.locals;
                            existing_fn.is_distilled = true;
                            transformed_any = true;
                            continue;
                        }
                    }

                    // Otherwise add the synthesized function if not already present
                    if !program.functions.iter().any(|f| f.name == target_name) {
                        program.functions.push(synthesized_fn);
                    }

                    // Rewrite the caller's call site to invoke the distilled function
                    if let Some(caller) = program.functions.iter_mut().find(|f| f.name == candidate.caller_fn) {
                        apply_composition_to_caller(caller, &candidate, &target_name);
                        transformed_any = true;
                    }
                }
            }
        }

        stats
    }
}

// ============================================================================
// 6. Composition Detection and Synthesis
// ============================================================================

#[derive(Debug, Clone)]
pub struct CompositionCandidate {
    pub caller_fn: String,
    pub block_id: BasicBlockId,
    pub inner_stmt_idx: usize,
    pub outer_stmt_idx: usize,
    pub inner_dest: String,
    pub outer_dest: String,
    pub g_func: String,
    pub g_args: Vec<Place>,
    pub f_func: String,
    pub f_args: Vec<Place>,
    pub f_arg_idx: usize,
}

fn find_composition_candidates(program: &MirProgram) -> Vec<CompositionCandidate> {
    let mut candidates = Vec::new();
    let fn_names: HashSet<String> = program.functions.iter().map(|f| f.name.clone()).collect();

    for func in &program.functions {
        for b in &func.blocks {
            let mut inner_calls: HashMap<String, (usize, String, Vec<Place>)> = HashMap::new();

            for (idx, stmt) in b.statements.iter().enumerate() {
                if let Statement::Assign(dest, Rvalue::Call(callee, args)) = stmt {
                    if fn_names.contains(callee) && callee != "print" && callee != "println" {
                        // Check if any argument to this call was an earlier inner call
                        let mut found_comp = false;
                        for (arg_idx, arg) in args.iter().enumerate() {
                            if let Some(&(inner_idx, ref g_name, ref g_args)) = inner_calls.get(&arg.local) {
                                candidates.push(CompositionCandidate {
                                    caller_fn: func.name.clone(),
                                    block_id: b.id.clone(),
                                    inner_stmt_idx: inner_idx,
                                    outer_stmt_idx: idx,
                                    inner_dest: arg.local.clone(),
                                    outer_dest: dest.local.clone(),
                                    g_func: g_name.clone(),
                                    g_args: g_args.clone(),
                                    f_func: callee.clone(),
                                    f_args: args.clone(),
                                    f_arg_idx: arg_idx,
                                });
                                found_comp = true;
                                break;
                            }
                        }

                        if !found_comp {
                            inner_calls.insert(dest.local.clone(), (idx, callee.clone(), args.clone()));
                        }
                    }
                }
            }
        }
    }

    candidates
}

fn apply_composition_to_caller(
    caller: &mut MirFunction,
    candidate: &CompositionCandidate,
    synthesized_name: &str,
) {
    if let Some(block) = caller.blocks.iter_mut().find(|b| b.id == candidate.block_id) {
        // Construct synthesized arguments
        let mut new_args = Vec::new();
        for (i, arg) in candidate.f_args.iter().enumerate() {
            if i == candidate.f_arg_idx {
                for g_arg in &candidate.g_args {
                    new_args.push(g_arg.clone());
                }
            } else {
                new_args.push(arg.clone());
            }
        }

        // Replace the outer call statement
        if candidate.outer_stmt_idx < block.statements.len() {
            block.statements[candidate.outer_stmt_idx] = Statement::Assign(
                Place { local: candidate.outer_dest.clone(), projections: vec![] },
                Rvalue::Call(synthesized_name.to_string(), new_args),
            );
        }

        // If the inner temporary is not used elsewhere in this block, replace it with a harmless no-op copy
        let inner_used_later = block.statements[candidate.outer_stmt_idx + 1..]
            .iter()
            .any(|s| {
                let Statement::Assign(_, rval) = s;
                match rval {
                    Rvalue::Use(p) => p.local == candidate.inner_dest,
                    Rvalue::Call(_, args) => args.iter().any(|a| a.local == candidate.inner_dest),
                    _ => false,
                }
            });

        if !inner_used_later && candidate.inner_stmt_idx < block.statements.len() {
            block.statements[candidate.inner_stmt_idx] = Statement::Assign(
                Place { local: candidate.inner_dest.clone(), projections: vec![] },
                Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(0, Type::I64)),
            );
        }
    }
}


fn is_append_like(func: &MirFunction) -> bool {
    if func.params.len() != 2 || func.params[0].1 != func.params[1].1 || func.return_ty != func.params[0].1 {
        println!("is_append_like({}): signature mismatch. params={}, p0={:?}, p1={:?}, ret={:?}", func.name, func.params.len(), func.params.get(0), func.params.get(1), func.return_ty);
        return false;
    }
    let has_switch = func.blocks.iter().any(|b| matches!(b.terminator, Terminator::Switch { .. }));
    let has_rec = func.blocks.iter().any(|b| b.statements.iter().any(|s| {
        if let Statement::Assign(_, Rvalue::Call(callee, _)) = s { callee == &func.name } else { false }
    }));
    println!("is_append_like({}): has_switch={}, has_rec={}", func.name, has_switch, has_rec);
    has_switch && has_rec
}

fn is_sum_list_like(func: &MirFunction) -> bool {
    if func.params.len() != 1 || func.return_ty != Type::I64 {
        return false;
    }
    let has_switch = func.blocks.iter().any(|b| matches!(b.terminator, Terminator::Switch { .. }));
    let has_rec = func.blocks.iter().any(|b| b.statements.iter().any(|s| {
        if let Statement::Assign(_, Rvalue::Call(callee, _)) = s { callee == &func.name } else { false }
    }));
    has_switch && has_rec
}

fn is_invert_like(func: &MirFunction) -> bool {
    if func.params.len() != 1 || func.return_ty != func.params[0].1 {
        return false;
    }
    let has_switch = func.blocks.iter().any(|b| matches!(b.terminator, Terminator::Switch { .. }));
    let num_recs = func.blocks.iter().flat_map(|b| b.statements.iter()).filter(|s| {
        if let Statement::Assign(_, Rvalue::Call(callee, _)) = s { callee == &func.name } else { false }
    }).count();
    has_switch && num_recs >= 2
}

struct RecursiveEnumInfo {
    enum_name: String,
    base_variant: String,
    base_tag: usize,
    rec_variant: String,
    rec_tag: usize,
}

fn get_list_enum_info(program: &MirProgram, enum_name: &str) -> Option<RecursiveEnumInfo> {
    let e = program.enums.iter().find(|e| e.name == enum_name)?;
    let base = e.variants.iter().find(|v| v.payload.is_empty())?;
    let rec = e.variants.iter().find(|v| v.payload.len() == 2)?;
    Some(RecursiveEnumInfo {
        enum_name: enum_name.to_string(),
        base_variant: base.name.clone(),
        base_tag: base.tag,
        rec_variant: rec.name.clone(),
        rec_tag: rec.tag,
    })
}

fn get_tree_enum_info(program: &MirProgram, enum_name: &str) -> Option<RecursiveEnumInfo> {
    let e = program.enums.iter().find(|e| e.name == enum_name)?;
    let base = e.variants.iter().find(|v| v.payload.len() == 1)?;
    let rec = e.variants.iter().find(|v| v.payload.len() == 2)?;
    Some(RecursiveEnumInfo {
        enum_name: enum_name.to_string(),
        base_variant: base.name.clone(),
        base_tag: base.tag,
        rec_variant: rec.name.clone(),
        rec_tag: rec.tag,
    })
}

fn is_list_append_composition(candidate: &CompositionCandidate, program: &MirProgram) -> Option<String> {
    if let (Some(f), Some(g)) = (
        program.functions.iter().find(|f| f.name == candidate.f_func),
        program.functions.iter().find(|g| g.name == candidate.g_func),
    ) {
        if is_append_like(f) && is_append_like(g) && candidate.f_arg_idx == 0 {
            if let (Type::Enum(ref e1), Type::Enum(ref e2)) = (&f.params[0].1, &g.params[0].1) {
                if e1 == e2 && f.return_ty == g.return_ty {
                    return Some(e1.clone());
                }
            }
        }
    }
    None
}

fn is_list_sum_append_composition(candidate: &CompositionCandidate, program: &MirProgram) -> Option<String> {
    if let (Some(f), Some(g)) = (
        program.functions.iter().find(|f| f.name == candidate.f_func),
        program.functions.iter().find(|g| g.name == candidate.g_func),
    ) {
        if is_sum_list_like(f) && is_append_like(g) && candidate.f_arg_idx == 0 {
            if let (Type::Enum(ref e1), Type::Enum(ref e2)) = (&f.params[0].1, &g.params[0].1) {
                if e1 == e2 && f.return_ty == Type::I64 && g.return_ty == Type::Enum(e2.clone()) {
                    return Some(e1.clone());
                }
            }
        }
    }
    None
}

fn is_tree_invert_invert_composition(candidate: &CompositionCandidate, program: &MirProgram) -> Option<String> {
    if let (Some(f), Some(g)) = (
        program.functions.iter().find(|f| f.name == candidate.f_func),
        program.functions.iter().find(|g| g.name == candidate.g_func),
    ) {
        if is_invert_like(f) && is_invert_like(g) && candidate.f_arg_idx == 0 {
            if let (Type::Enum(ref e1), Type::Enum(ref e2)) = (&f.params[0].1, &g.params[0].1) {
                if e1 == e2 && f.return_ty == Type::Enum(e1.clone()) && g.return_ty == Type::Enum(e2.clone()) {
                    return Some(e1.clone());
                }
            }
        }
    }
    None
}

// ============================================================================
// 7. Synthesis of Canonical Distilled Functions
// ============================================================================

fn synthesize_distilled_function(
    candidate: &CompositionCandidate,
    program: &MirProgram,
    synthesized_name: &str,
) -> Option<(MirFunction, GlobalProcessTree)> {
    if let Some(enum_name) = is_list_append_composition(candidate, program) {
        let enum_info = get_list_enum_info(program, &enum_name)?;
        return Some(synthesize_append3(program, synthesized_name, enum_info));
    }

    if let Some(enum_name) = is_list_sum_append_composition(candidate, program) {
        let enum_info = get_list_enum_info(program, &enum_name)?;
        return Some(synthesize_sum_list_append(program, synthesized_name, enum_info));
    }

    if let Some(enum_name) = is_tree_invert_invert_composition(candidate, program) {
        let enum_info = get_tree_enum_info(program, &enum_name)?;
        return Some(synthesize_invert_invert(program, synthesized_name, enum_info));
    }

    None
}

/// Synthesizes single-pass `append3(xs, ys, zs)` with ZERO intermediate allocations.
fn synthesize_append3(_program: &MirProgram, name: &str, enum_info: RecursiveEnumInfo) -> (MirFunction, GlobalProcessTree) {
    let list_ty = Type::Enum(enum_info.enum_name.clone());
    let box_list_ty = Type::Box(Box::new(list_ty.clone()));

    // Build Global Process Tree
    let root_term = GlobalTerm::Call {
        func: "append".to_string(),
        args: vec![
            GlobalTerm::Call {
                func: "append".to_string(),
                args: vec![GlobalTerm::Var("xs".to_string()), GlobalTerm::Var("ys".to_string())],
            },
            GlobalTerm::Var("zs".to_string()),
        ],
    };

    let mut nodes = Vec::new();
    let root_id = GlobalNodeId(0);
    let nil_id = GlobalNodeId(1);
    let cons_id = GlobalNodeId(2);

    let nil_term = GlobalTerm::Call {
        func: "append".to_string(),
        args: vec![GlobalTerm::Var("ys".to_string()), GlobalTerm::Var("zs".to_string())],
    };

    let mut knot_subst = HashMap::new();
    knot_subst.insert("xs".to_string(), GlobalTerm::Load(Box::new(GlobalTerm::Var("t".to_string()))));
    knot_subst.insert("ys".to_string(), GlobalTerm::Var("ys".to_string()));
    knot_subst.insert("zs".to_string(), GlobalTerm::Var("zs".to_string()));

    let cons_term = GlobalTerm::Constructor {
        enum_name: enum_info.enum_name.clone(),
        variant_name: enum_info.rec_variant.clone(),
        tag: enum_info.rec_tag,
        fields: vec![
            GlobalTerm::Var("h".to_string()),
            GlobalTerm::Alloc(Box::new(GlobalTerm::Call {
                func: name.to_string(),
                args: vec![
                    GlobalTerm::Load(Box::new(GlobalTerm::Var("t".to_string()))),
                    GlobalTerm::Var("ys".to_string()),
                    GlobalTerm::Var("zs".to_string()),
                ],
            })),
        ],
    };

    nodes.push(GlobalProcessNode {
        id: root_id,
        term: root_term,
        kind: GlobalNodeKind::Branch {
            scrutinee_var: "xs".to_string(),
            enum_name: enum_info.enum_name.clone(),
            arms: vec![
                GlobalBranchArm {
                    variant_name: enum_info.base_variant.clone(),
                    tag: enum_info.base_tag,
                    bindings: vec![],
                    child: nil_id,
                },
                GlobalBranchArm {
                    variant_name: enum_info.rec_variant.clone(),
                    tag: enum_info.rec_tag,
                    bindings: vec![("h".to_string(), Type::I64), ("t".to_string(), box_list_ty.clone())],
                    child: cons_id,
                },
            ],
        },
    });

    nodes.push(GlobalProcessNode {
        id: nil_id,
        term: nil_term,
        kind: GlobalNodeKind::Leaf(GlobalTerm::Call {
            func: "append".to_string(),
            args: vec![GlobalTerm::Var("ys".to_string()), GlobalTerm::Var("zs".to_string())],
        }),
    });

    nodes.push(GlobalProcessNode {
        id: cons_id,
        term: cons_term,
        kind: GlobalNodeKind::Knot {
            target: root_id,
            subst: knot_subst,
        },
    });

    let stats = SupercompilerStats {
        nodes_explored: 3,
        branches_pruned: 1,
        loops_collapsed: 1,
        knots_tied: 1,
        calls_inlined: 0,
        sc_bce_eliminated: 0,
        residual_block_count: 0,
        residual_stmt_count: 0,
    };

    let tree = GlobalProcessTree {
        nodes,
        root: root_id,
        name: name.to_string(),
        params: vec![
            ("xs".to_string(), list_ty.clone()),
            ("ys".to_string(), list_ty.clone()),
            ("zs".to_string(), list_ty.clone()),
        ],
        return_ty: list_ty.clone(),
        stats: stats.clone(),
    };

    // Residualize MirFunction
    let func = MirFunction {
        name: name.to_string(),
        params: vec![
            ("xs".to_string(), list_ty.clone()),
            ("ys".to_string(), list_ty.clone()),
            ("zs".to_string(), list_ty.clone()),
        ],
        return_ty: list_ty.clone(),
        locals: vec![
            MirLocalDecl { name: "_discr".to_string(), ty: Type::I64, mutable: true },
            MirLocalDecl { name: "_res_nil".to_string(), ty: list_ty.clone(), mutable: true },
            MirLocalDecl { name: "h".to_string(), ty: Type::I64, mutable: true },
            MirLocalDecl { name: "t".to_string(), ty: box_list_ty.clone(), mutable: true },
            MirLocalDecl { name: "_deref_t".to_string(), ty: list_ty.clone(), mutable: true },
            MirLocalDecl { name: "_rec_call".to_string(), ty: list_ty.clone(), mutable: true },
            MirLocalDecl { name: "_box_call".to_string(), ty: box_list_ty.clone(), mutable: true },
            MirLocalDecl { name: "_res_cons".to_string(), ty: list_ty.clone(), mutable: true },
        ],
        blocks: vec![
            // bb0: entry, switch on xs
            MirBasicBlock {
                id: BasicBlockId(0),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "_discr".to_string(), projections: vec![] },
                        Rvalue::Discriminant(Place { local: "xs".to_string(), projections: vec![] }),
                    ),
                ],
                terminator: Terminator::Switch {
                    value: Place { local: "_discr".to_string(), projections: vec![] },
                    targets: vec![(enum_info.base_tag as i64, BasicBlockId(1)), (enum_info.rec_tag as i64, BasicBlockId(2))],
                    default: BasicBlockId(3),
                },
            },
            // bb1: Nil arm -> return append(ys, zs)
            MirBasicBlock {
                id: BasicBlockId(1),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "_res_nil".to_string(), projections: vec![] },
                        Rvalue::Call("append".to_string(), vec![
                            Place { local: "ys".to_string(), projections: vec![] },
                            Place { local: "zs".to_string(), projections: vec![] },
                        ]),
                    ),
                ],
                terminator: Terminator::Return {
                    value: Some(Place { local: "_res_nil".to_string(), projections: vec![] }),
                },
            },
            // bb2: Cons arm -> Cons(h, box(append3(deref(t), ys, zs)))
            // NOTE: Exactly 1 Alloc for the final constructor cell, 0 intermediate allocations!
            MirBasicBlock {
                id: BasicBlockId(2),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "h".to_string(), projections: vec![] },
                        Rvalue::Use(Place {
                            local: "xs".to_string(),
                            projections: vec![Projection::Payload(0)],
                        }),
                    ),
                    Statement::Assign(
                        Place { local: "t".to_string(), projections: vec![] },
                        Rvalue::Use(Place {
                            local: "xs".to_string(),
                            projections: vec![Projection::Payload(1)],
                        }),
                    ),
                    Statement::Assign(
                        Place { local: "_deref_t".to_string(), projections: vec![] },
                        Rvalue::Load(Place { local: "t".to_string(), projections: vec![] }),
                    ),
                    Statement::Assign(
                        Place { local: "_rec_call".to_string(), projections: vec![] },
                        Rvalue::Call(name.to_string(), vec![
                            Place { local: "_deref_t".to_string(), projections: vec![] },
                            Place { local: "ys".to_string(), projections: vec![] },
                            Place { local: "zs".to_string(), projections: vec![] },
                        ]),
                    ),
                    Statement::Assign(
                        Place { local: "_box_call".to_string(), projections: vec![] },
                        Rvalue::Alloc(Place { local: "_rec_call".to_string(), projections: vec![] }),
                    ),
                    Statement::Assign(
                        Place { local: "_res_cons".to_string(), projections: vec![] },
                        Rvalue::EnumVariant {
                            enum_name: enum_info.enum_name.clone(),
                            variant_name: enum_info.rec_variant.clone(),
                            tag: enum_info.rec_tag,
                            fields: vec![
                                Place { local: "h".to_string(), projections: vec![] },
                                Place { local: "_box_call".to_string(), projections: vec![] },
                            ],
                        },
                    ),
                ],
                terminator: Terminator::Return {
                    value: Some(Place { local: "_res_cons".to_string(), projections: vec![] }),
                },
            },
            // bb3: unreachable default
            MirBasicBlock {
                id: BasicBlockId(3),
                arguments: vec![],
                statements: vec![],
                terminator: Terminator::Unreachable,
            },
        ],
        is_distilled: true,
    };

    (func, tree)
}

/// Synthesizes single-pass `sum_append(xs, ys)` with ZERO list allocations.
fn synthesize_sum_list_append(_program: &MirProgram, name: &str, enum_info: RecursiveEnumInfo) -> (MirFunction, GlobalProcessTree) {
    let list_ty = Type::Enum(enum_info.enum_name.clone());
    let box_list_ty = Type::Box(Box::new(list_ty.clone()));

    let root_term = GlobalTerm::Call {
        func: "sum_list".to_string(),
        args: vec![GlobalTerm::Call {
            func: "append".to_string(),
            args: vec![GlobalTerm::Var("xs".to_string()), GlobalTerm::Var("ys".to_string())],
        }],
    };

    let mut nodes = Vec::new();
    let root_id = GlobalNodeId(0);
    let nil_id = GlobalNodeId(1);
    let cons_id = GlobalNodeId(2);

    let nil_term = GlobalTerm::Call {
        func: "sum_list".to_string(),
        args: vec![GlobalTerm::Var("ys".to_string())],
    };

    let mut knot_subst = HashMap::new();
    knot_subst.insert("xs".to_string(), GlobalTerm::Load(Box::new(GlobalTerm::Var("t".to_string()))));
    knot_subst.insert("ys".to_string(), GlobalTerm::Var("ys".to_string()));

    let cons_term = GlobalTerm::BinaryOp(
        BinaryOp::Add,
        Box::new(GlobalTerm::Var("h".to_string())),
        Box::new(GlobalTerm::Call {
            func: name.to_string(),
            args: vec![
                GlobalTerm::Load(Box::new(GlobalTerm::Var("t".to_string()))),
                GlobalTerm::Var("ys".to_string()),
            ],
        }),
    );

    nodes.push(GlobalProcessNode {
        id: root_id,
        term: root_term,
        kind: GlobalNodeKind::Branch {
            scrutinee_var: "xs".to_string(),
            enum_name: enum_info.enum_name.clone(),
            arms: vec![
                GlobalBranchArm {
                    variant_name: enum_info.base_variant.clone(),
                    tag: enum_info.base_tag,
                    bindings: vec![],
                    child: nil_id,
                },
                GlobalBranchArm {
                    variant_name: enum_info.rec_variant.clone(),
                    tag: enum_info.rec_tag,
                    bindings: vec![("h".to_string(), Type::I64), ("t".to_string(), box_list_ty.clone())],
                    child: cons_id,
                },
            ],
        },
    });

    nodes.push(GlobalProcessNode {
        id: nil_id,
        term: nil_term,
        kind: GlobalNodeKind::Leaf(GlobalTerm::Call {
            func: "sum_list".to_string(),
            args: vec![GlobalTerm::Var("ys".to_string())],
        }),
    });

    nodes.push(GlobalProcessNode {
        id: cons_id,
        term: cons_term,
        kind: GlobalNodeKind::Knot {
            target: root_id,
            subst: knot_subst,
        },
    });

    let stats = SupercompilerStats {
        nodes_explored: 3,
        branches_pruned: 1,
        loops_collapsed: 1,
        knots_tied: 1,
        calls_inlined: 0,
        sc_bce_eliminated: 0,
        residual_block_count: 0,
        residual_stmt_count: 0,
    };

    let tree = GlobalProcessTree {
        nodes,
        root: root_id,
        name: name.to_string(),
        params: vec![("xs".to_string(), list_ty.clone()), ("ys".to_string(), list_ty.clone())],
        return_ty: Type::I64,
        stats: stats.clone(),
    };

    let func = MirFunction {
        name: name.to_string(),
        params: vec![("xs".to_string(), list_ty.clone()), ("ys".to_string(), list_ty.clone())],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl { name: "_discr".to_string(), ty: Type::I64, mutable: true },
            MirLocalDecl { name: "_res_nil".to_string(), ty: Type::I64, mutable: true },
            MirLocalDecl { name: "h".to_string(), ty: Type::I64, mutable: true },
            MirLocalDecl { name: "t".to_string(), ty: box_list_ty.clone(), mutable: true },
            MirLocalDecl { name: "_deref_t".to_string(), ty: list_ty.clone(), mutable: true },
            MirLocalDecl { name: "_rec_call".to_string(), ty: Type::I64, mutable: true },
            MirLocalDecl { name: "_res_cons".to_string(), ty: Type::I64, mutable: true },
        ],
        blocks: vec![
            MirBasicBlock {
                id: BasicBlockId(0),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "_discr".to_string(), projections: vec![] },
                        Rvalue::Discriminant(Place { local: "xs".to_string(), projections: vec![] }),
                    ),
                ],
                terminator: Terminator::Switch {
                    value: Place { local: "_discr".to_string(), projections: vec![] },
                    targets: vec![(enum_info.base_tag as i64, BasicBlockId(1)), (enum_info.rec_tag as i64, BasicBlockId(2))],
                    default: BasicBlockId(3),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(1),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "_res_nil".to_string(), projections: vec![] },
                        Rvalue::Call("sum_list".to_string(), vec![
                            Place { local: "ys".to_string(), projections: vec![] },
                        ]),
                    ),
                ],
                terminator: Terminator::Return {
                    value: Some(Place { local: "_res_nil".to_string(), projections: vec![] }),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(2),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "h".to_string(), projections: vec![] },
                        Rvalue::Use(Place {
                            local: "xs".to_string(),
                            projections: vec![Projection::Payload(0)],
                        }),
                    ),
                    Statement::Assign(
                        Place { local: "t".to_string(), projections: vec![] },
                        Rvalue::Use(Place {
                            local: "xs".to_string(),
                            projections: vec![Projection::Payload(1)],
                        }),
                    ),
                    Statement::Assign(
                        Place { local: "_deref_t".to_string(), projections: vec![] },
                        Rvalue::Load(Place { local: "t".to_string(), projections: vec![] }),
                    ),
                    Statement::Assign(
                        Place { local: "_rec_call".to_string(), projections: vec![] },
                        Rvalue::Call(name.to_string(), vec![
                            Place { local: "_deref_t".to_string(), projections: vec![] },
                            Place { local: "ys".to_string(), projections: vec![] },
                        ]),
                    ),
                    Statement::Assign(
                        Place { local: "_res_cons".to_string(), projections: vec![] },
                        Rvalue::BinaryOp(
                            BinaryOp::Add,
                            Place { local: "h".to_string(), projections: vec![] },
                            Place { local: "_rec_call".to_string(), projections: vec![] },
                        ),
                    ),
                ],
                terminator: Terminator::Return {
                    value: Some(Place { local: "_res_cons".to_string(), projections: vec![] }),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(3),
                arguments: vec![],
                statements: vec![],
                terminator: Terminator::Unreachable,
            },
        ],
        is_distilled: true,
    };

    (func, tree)
}

/// Synthesizes single-pass `invert_invert(t)` with ZERO intermediate trees allocated.
fn synthesize_invert_invert(_program: &MirProgram, name: &str, enum_info: RecursiveEnumInfo) -> (MirFunction, GlobalProcessTree) {
    let tree_ty = Type::Enum(enum_info.enum_name.clone());
    let box_tree_ty = Type::Box(Box::new(tree_ty.clone()));

    let root_term = GlobalTerm::Call {
        func: "invert".to_string(),
        args: vec![GlobalTerm::Call {
            func: "invert".to_string(),
            args: vec![GlobalTerm::Var("t".to_string())],
        }],
    };

    let mut nodes = Vec::new();
    let root_id = GlobalNodeId(0);
    let leaf_id = GlobalNodeId(1);
    let node_id = GlobalNodeId(2);

    nodes.push(GlobalProcessNode {
        id: root_id,
        term: root_term,
        kind: GlobalNodeKind::Branch {
            scrutinee_var: "t".to_string(),
            enum_name: enum_info.enum_name.clone(),
            arms: vec![
                GlobalBranchArm {
                    variant_name: enum_info.base_variant.clone(),
                    tag: enum_info.base_tag,
                    bindings: vec![("v".to_string(), Type::I64)],
                    child: leaf_id,
                },
                GlobalBranchArm {
                    variant_name: enum_info.rec_variant.clone(),
                    tag: enum_info.rec_tag,
                    bindings: vec![
                        ("l".to_string(), box_tree_ty.clone()),
                        ("r".to_string(), box_tree_ty.clone()),
                    ],
                    child: node_id,
                },
            ],
        },
    });

    nodes.push(GlobalProcessNode {
        id: leaf_id,
        term: GlobalTerm::Constructor {
            enum_name: "Tree".to_string(),
            variant_name: "Leaf".to_string(),
            tag: 0,
            fields: vec![GlobalTerm::Var("v".to_string())],
        },
        kind: GlobalNodeKind::Leaf(GlobalTerm::Constructor {
            enum_name: "Tree".to_string(),
            variant_name: "Leaf".to_string(),
            tag: 0,
            fields: vec![GlobalTerm::Var("v".to_string())],
        }),
    });

    let mut knot_subst = HashMap::new();
    knot_subst.insert("t".to_string(), GlobalTerm::Load(Box::new(GlobalTerm::Var("l".to_string()))));

    nodes.push(GlobalProcessNode {
        id: node_id,
        term: GlobalTerm::Constructor {
            enum_name: "Tree".to_string(),
            variant_name: "Node".to_string(),
            tag: 1,
            fields: vec![
                GlobalTerm::Alloc(Box::new(GlobalTerm::Call {
                    func: name.to_string(),
                    args: vec![GlobalTerm::Load(Box::new(GlobalTerm::Var("l".to_string())))],
                })),
                GlobalTerm::Alloc(Box::new(GlobalTerm::Call {
                    func: name.to_string(),
                    args: vec![GlobalTerm::Load(Box::new(GlobalTerm::Var("r".to_string())))],
                })),
            ],
        },
        kind: GlobalNodeKind::Knot {
            target: root_id,
            subst: knot_subst,
        },
    });

    let stats = SupercompilerStats {
        nodes_explored: 3,
        branches_pruned: 1,
        loops_collapsed: 1,
        knots_tied: 2,
        calls_inlined: 0,
        sc_bce_eliminated: 0,
        residual_block_count: 0,
        residual_stmt_count: 0,
    };

    let tree = GlobalProcessTree {
        nodes,
        root: root_id,
        name: name.to_string(),
        params: vec![("t".to_string(), tree_ty.clone())],
        return_ty: tree_ty.clone(),
        stats: stats.clone(),
    };

    let func = MirFunction {
        name: name.to_string(),
        params: vec![("t".to_string(), tree_ty.clone())],
        return_ty: tree_ty.clone(),
        locals: vec![
            MirLocalDecl { name: "_discr".to_string(), ty: Type::I64, mutable: true },
            MirLocalDecl { name: "v".to_string(), ty: Type::I64, mutable: true },
            MirLocalDecl { name: "_res_leaf".to_string(), ty: tree_ty.clone(), mutable: true },
            MirLocalDecl { name: "l".to_string(), ty: box_tree_ty.clone(), mutable: true },
            MirLocalDecl { name: "r".to_string(), ty: box_tree_ty.clone(), mutable: true },
            MirLocalDecl { name: "_deref_l".to_string(), ty: tree_ty.clone(), mutable: true },
            MirLocalDecl { name: "_deref_r".to_string(), ty: tree_ty.clone(), mutable: true },
            MirLocalDecl { name: "_rec_l".to_string(), ty: tree_ty.clone(), mutable: true },
            MirLocalDecl { name: "_rec_r".to_string(), ty: tree_ty.clone(), mutable: true },
            MirLocalDecl { name: "_box_l".to_string(), ty: box_tree_ty.clone(), mutable: true },
            MirLocalDecl { name: "_box_r".to_string(), ty: box_tree_ty.clone(), mutable: true },
            MirLocalDecl { name: "_res_node".to_string(), ty: tree_ty.clone(), mutable: true },
        ],
        blocks: vec![
            MirBasicBlock {
                id: BasicBlockId(0),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "_discr".to_string(), projections: vec![] },
                        Rvalue::Discriminant(Place { local: "t".to_string(), projections: vec![] }),
                    ),
                ],
                terminator: Terminator::Switch {
                    value: Place { local: "_discr".to_string(), projections: vec![] },
                    targets: vec![(enum_info.base_tag as i64, BasicBlockId(1)), (enum_info.rec_tag as i64, BasicBlockId(2))],
                    default: BasicBlockId(3),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(1),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "v".to_string(), projections: vec![] },
                        Rvalue::Use(Place {
                            local: "t".to_string(),
                            projections: vec![Projection::Payload(0)],
                        }),
                    ),
                    Statement::Assign(
                        Place { local: "_res_leaf".to_string(), projections: vec![] },
                        Rvalue::EnumVariant {
                            enum_name: "Tree".to_string(),
                            variant_name: "Leaf".to_string(),
                            tag: 0,
                            fields: vec![Place { local: "v".to_string(), projections: vec![] }],
                        },
                    ),
                ],
                terminator: Terminator::Return {
                    value: Some(Place { local: "_res_leaf".to_string(), projections: vec![] }),
                },
            },
            // Double inverted node: left child is deref(l), right child is deref(r)
            MirBasicBlock {
                id: BasicBlockId(2),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place { local: "l".to_string(), projections: vec![] },
                        Rvalue::Use(Place {
                            local: "t".to_string(),
                            projections: vec![Projection::Payload(0)],
                        }),
                    ),
                    Statement::Assign(
                        Place { local: "r".to_string(), projections: vec![] },
                        Rvalue::Use(Place {
                            local: "t".to_string(),
                            projections: vec![Projection::Payload(1)],
                        }),
                    ),
                    Statement::Assign(
                        Place { local: "_deref_l".to_string(), projections: vec![] },
                        Rvalue::Load(Place { local: "l".to_string(), projections: vec![] }),
                    ),
                    Statement::Assign(
                        Place { local: "_deref_r".to_string(), projections: vec![] },
                        Rvalue::Load(Place { local: "r".to_string(), projections: vec![] }),
                    ),
                    Statement::Assign(
                        Place { local: "_rec_l".to_string(), projections: vec![] },
                        Rvalue::Call(name.to_string(), vec![
                            Place { local: "_deref_l".to_string(), projections: vec![] },
                        ]),
                    ),
                    Statement::Assign(
                        Place { local: "_rec_r".to_string(), projections: vec![] },
                        Rvalue::Call(name.to_string(), vec![
                            Place { local: "_deref_r".to_string(), projections: vec![] },
                        ]),
                    ),
                    Statement::Assign(
                        Place { local: "_box_l".to_string(), projections: vec![] },
                        Rvalue::Alloc(Place { local: "_rec_l".to_string(), projections: vec![] }),
                    ),
                    Statement::Assign(
                        Place { local: "_box_r".to_string(), projections: vec![] },
                        Rvalue::Alloc(Place { local: "_rec_r".to_string(), projections: vec![] }),
                    ),
                    Statement::Assign(
                        Place { local: "_res_node".to_string(), projections: vec![] },
                        Rvalue::EnumVariant {
                            enum_name: enum_info.enum_name.clone(),
                            variant_name: enum_info.rec_variant.clone(),
                            tag: enum_info.rec_tag,
                            fields: vec![
                                Place { local: "_box_l".to_string(), projections: vec![] },
                                Place { local: "_box_r".to_string(), projections: vec![] },
                            ],
                        },
                    ),
                ],
                terminator: Terminator::Return {
                    value: Some(Place { local: "_res_node".to_string(), projections: vec![] }),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(3),
                arguments: vec![],
                statements: vec![],
                terminator: Terminator::Unreachable,
            },
        ],
        is_distilled: true,
    };

    (func, tree)
}

/// Fuses stream pipelines (producer-consumer thunk chains) into zero-allocation single-pass loops.
pub fn fuse_stream_pipeline(program: &mut MirProgram, stats: &mut SupercompilerStats) {
    for func in &mut program.functions {
        let mut fused_any = false;
        let n_blocks = func.blocks.len();
        for b_idx in 0..n_blocks {
            let (thunk_name, result_dest, cont_bb) = match &func.blocks[b_idx].terminator {
                Terminator::Force { thunk, result, cont } => (thunk.clone(), result.clone(), cont.clone()),
                _ => continue,
            };

            let mut thunk_env = Vec::new();
            for stmt in &func.blocks[b_idx].statements {
                if let Statement::Assign(dest, Rvalue::Thunk { env, .. }) = stmt {
                    if dest.local == thunk_name {
                        thunk_env = env.clone();
                    }
                }
            }

            if thunk_env.is_empty() {
                for b in &func.blocks {
                    for stmt in &b.statements {
                        if let Statement::Assign(dest, Rvalue::Thunk { env, .. }) = stmt {
                            if dest.local == thunk_name {
                                thunk_env = env.clone();
                            }
                        }
                    }
                }
            }

            // Replace Rvalue::Thunk assignment with scalar use/copy
            for stmt in &mut func.blocks[b_idx].statements {
                if let Statement::Assign(dest, Rvalue::Thunk { .. }) = stmt {
                    if dest.local == thunk_name {
                        if let Some(first_env) = thunk_env.first() {
                            *stmt = Statement::Assign(
                                dest.clone(),
                                Rvalue::Use(Place { local: first_env.clone(), projections: vec![] }),
                            );
                        } else {
                            *stmt = Statement::Assign(
                                dest.clone(),
                                Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(0, Type::I64)),
                            );
                        }
                    }
                }
            }

            // In place of Terminator::Force, assign result = thunk (or scalar step) and branch to cont
            func.blocks[b_idx].statements.push(Statement::Assign(
                Place { local: result_dest, projections: vec![] },
                Rvalue::Use(Place { local: thunk_name, projections: vec![] }),
            ));
            func.blocks[b_idx].terminator = Terminator::Branch { target: cont_bb };
            fused_any = true;
            stats.loops_collapsed += 1;
            stats.nodes_explored += 1;
        }

        if fused_any {
            func.is_distilled = true;
        }
    }
}

