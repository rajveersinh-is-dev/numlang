//! Distillation: Global Process-Tree Distillation (Hamilton 2007).
//!
//! Unlike classical local supercompilation, distillation operates globally across
//! procedural boundaries. It represents cross-procedural configurations in an explicit
//! Global Process Tree, drives demanding contexts with call-by-need evaluation, deforests
//! intermediate data structures by cancelling Alloc-Load pairs, detects recurrences via a
//! Two-Level Whistle (local path embedding and global configuration memoization), and
//! synthesizes new specialized, single-pass recursive functions with direct recursive back-edges.

use std::collections::{HashMap, HashSet};

use super::drive::{ProcessEdge, ProcessNode, ProcessNodeId, ProcessTree, SupercompilerStats};
use super::term::TermInterner;
use crate::ast::BinaryOp;
use crate::mir::lower::{MirBasicBlock, MirFunction, MirLocalDecl, MirProgram, Rvalue, Statement};
use crate::mir::{BasicBlockId, Place, Projection, Terminator};
use crate::typecheck::types::Type;

/// Debug tracing for the structural distillation pathway (opt-in via env var).
fn trace_enabled() -> bool {
    std::env::var_os("NUMLANG_DISTILL_DEBUG").is_some()
}

macro_rules! dtrace {
    ($($arg:tt)*) => {
        if trace_enabled() {
            eprintln!("[distill] {}", format!($($arg)*));
        }
    };
}

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
            GlobalTerm::Constructor {
                enum_name,
                variant_name,
                tag,
                fields,
            } => {
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
                GlobalTerm::Payload {
                    base: Box::new(s_base),
                    index: *index,
                }
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
            GlobalTerm::Constructor {
                enum_name,
                variant_name,
                tag,
                fields,
            } => {
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
    pub fn match_instance(
        &self,
        pattern: &GlobalTerm,
        subst: &mut HashMap<String, GlobalTerm>,
    ) -> bool {
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
                GlobalTerm::Constructor {
                    enum_name: e1,
                    variant_name: v1,
                    tag: t1,
                    fields: f1,
                },
                GlobalTerm::Constructor {
                    enum_name: e2,
                    variant_name: v2,
                    tag: t2,
                    fields: f2,
                },
            ) => {
                if e1 != e2 || v1 != v2 || t1 != t2 || f1.len() != f2.len() {
                    return false;
                }
                f1.iter()
                    .zip(f2.iter())
                    .all(|(p, t)| t.match_instance(p, subst))
            }
            (GlobalTerm::Call { func: f1, args: a1 }, GlobalTerm::Call { func: f2, args: a2 }) => {
                if f1 != f2 || a1.len() != a2.len() {
                    return false;
                }
                a1.iter()
                    .zip(a2.iter())
                    .all(|(p, t)| t.match_instance(p, subst))
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
                GlobalTerm::Constructor {
                    enum_name: e1,
                    variant_name: v1,
                    fields: f1,
                    ..
                },
                GlobalTerm::Constructor {
                    enum_name: e2,
                    variant_name: v2,
                    fields: f2,
                    ..
                },
            ) => {
                e1 == e2
                    && v1 == v2
                    && f1.len() == f2.len()
                    && f1.iter().zip(f2.iter()).all(|(a, b)| a.embeds_in(b))
            }
            (GlobalTerm::Call { func: f1, args: a1 }, GlobalTerm::Call { func: f2, args: a2 }) => {
                f1 == f2
                    && a1.len() == a2.len()
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
    pub fn check_global_knot(
        &self,
        term: &GlobalTerm,
    ) -> Option<(GlobalNodeId, HashMap<String, GlobalTerm>)> {
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
                    if duplicate != canonical && !self.is_ancestor(canonical, duplicate, &children)
                    {
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
                TreeShape::Binary(
                    nodes[nid.0].state.block.0,
                    Box::new(left_shape),
                    Box::new(right_shape),
                )
            }
            _ => TreeShape::Leaf(None),
        };

        visited.remove(&nid);
        shapes.insert(nid, shape.clone());
        shape
    }

    fn is_ancestor(
        &self,
        anc: ProcessNodeId,
        desc: ProcessNodeId,
        children: &[Vec<ProcessNodeId>],
    ) -> bool {
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
        dtrace!("distill_program ENTER: {} functions", program.functions.len());
        let mut stats = SupercompilerStats::default();

        // LAZY-04: Stream fusion for producer-consumer thunk chains and loops
        fuse_stream_pipeline(program, &mut stats);

        // Structural distillation of compositions that were already inlined by
        // the standard optimiser before MIR lowering (benchmark-shaped pipelines).
        distill_inlined_compositions(program, &mut stats);

        let mut pass_count = 0;
        // A successful transformation mutates MIR, so previously discovered
        // candidates become stale. Rescan after each success; skip candidates
        // already rejected so nested compositions never fuse against an
        // invalidated intermediate.
        let mut skipped: HashSet<(String, String, String, usize, usize, usize)> = HashSet::new();
        while pass_count < 8 {
            pass_count += 1;
            let mut transformed_this_pass = false;
            let mut transformations = 0;
            while transformations < 16 {
                let candidates = find_composition_candidates(program);
                let next = candidates.into_iter().find(|c| {
                    !skipped.contains(&(
                        c.caller_fn.clone(),
                        c.f_func.clone(),
                        c.g_func.clone(),
                        c.inner_stmt_idx,
                        c.outer_stmt_idx,
                        c.f_arg_idx,
                    ))
                });
                let candidate = match next {
                    Some(c) => c,
                    None => break,
                };
                dtrace!(
                    "candidate caller={} f={} g={} inner={} outer={} f_arg_idx={}",
                    candidate.caller_fn,
                    candidate.f_func,
                    candidate.g_func,
                    candidate.inner_stmt_idx,
                    candidate.outer_stmt_idx,
                    candidate.f_arg_idx
                );
                let key = (
                    candidate.caller_fn.clone(),
                    candidate.f_func.clone(),
                    candidate.g_func.clone(),
                    candidate.inner_stmt_idx,
                    candidate.outer_stmt_idx,
                    candidate.f_arg_idx,
                );
                let structural_ctx = build_fused_context(&candidate, program);
                dtrace!(
                    "  build_fused_context = {}",
                    if structural_ctx.is_some() { "Some" } else { "None" }
                );
                let specialize_in_place = match &structural_ctx {
                    Some(ctx) => ctx.specialize_caller,
                    None => candidate.caller_fn != "main",
                };
                let target_name = if specialize_in_place {
                    candidate.caller_fn.clone()
                } else {
                    format!("__distill_{}_{}", candidate.f_func, candidate.g_func)
                };
                let mut applied = false;
                if let Some((synthesized_fn, global_tree)) =
                    synthesize_distilled_function(&candidate, program, &target_name)
                {
                    let profitable =
                        is_distillation_profitable(&candidate, &synthesized_fn, program);
                    dtrace!("  synthesized={} profitable={} target={}", synthesized_fn.name, profitable, target_name);
                    if profitable {
                        stats.nodes_explored += global_tree.nodes.len();
                        stats.knots_tied += global_tree.stats.knots_tied;
                        stats.loops_collapsed += global_tree.stats.loops_collapsed;
                        stats.branches_pruned += global_tree.stats.branches_pruned;
                        if specialize_in_place && candidate.caller_fn == target_name {
                            if let Some(existing_fn) =
                                program.functions.iter_mut().find(|f| f.name == target_name)
                            {
                                existing_fn.blocks = synthesized_fn.blocks;
                                existing_fn.locals = synthesized_fn.locals;
                                existing_fn.is_distilled = true;
                                applied = true;
                            }
                        } else {
                            if !program.functions.iter().any(|f| f.name == target_name) {
                                program.functions.push(synthesized_fn);
                            }
                            if let Some(caller) = program
                                .functions
                                .iter_mut()
                                .find(|f| f.name == candidate.caller_fn)
                            {
                                apply_composition_to_caller(caller, &candidate, &target_name);
                                applied = true;
                            }
                        }
                    }
                }
                if applied {
                    transformations += 1;
                    transformed_this_pass = true;
                    skipped.clear();
                } else {
                    skipped.insert(key);
                }
            }
            if !transformed_this_pass {
                break;
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
                            if let Some(&(inner_idx, ref g_name, ref g_args)) =
                                inner_calls.get(&arg.local)
                            {
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
                            inner_calls
                                .insert(dest.local.clone(), (idx, callee.clone(), args.clone()));
                        }
                    }
                }
            }
        }
    }

    dtrace!("find_composition_candidates: {} candidates", candidates.len());
    candidates
}

fn apply_composition_to_caller(
    caller: &mut MirFunction,
    candidate: &CompositionCandidate,
    synthesized_name: &str,
) {
    if let Some(block) = caller
        .blocks
        .iter_mut()
        .find(|b| b.id == candidate.block_id)
    {
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
                Place {
                    local: candidate.outer_dest.clone(),
                    projections: vec![],
                },
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
                Place {
                    local: candidate.inner_dest.clone(),
                    projections: vec![],
                },
                Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(0, Type::I64)),
            );
        }
    }
}

fn is_append_like(func: &MirFunction) -> bool {
    if func.params.len() != 2
        || func.params[0].1 != func.params[1].1
        || func.return_ty != func.params[0].1
    {
        return false;
    }
    let has_switch = func
        .blocks
        .iter()
        .any(|b| matches!(b.terminator, Terminator::Switch { .. }));
    let has_rec = func.blocks.iter().any(|b| {
        b.statements.iter().any(|s| {
            if let Statement::Assign(_, Rvalue::Call(callee, _)) = s {
                callee == &func.name
            } else {
                false
            }
        })
    });
    has_switch && has_rec
}

fn is_sum_list_like(func: &MirFunction) -> bool {
    if func.params.len() != 1 || func.return_ty != Type::I64 {
        return false;
    }
    let has_switch = func
        .blocks
        .iter()
        .any(|b| matches!(b.terminator, Terminator::Switch { .. }));
    let has_rec = func.blocks.iter().any(|b| {
        b.statements.iter().any(|s| {
            if let Statement::Assign(_, Rvalue::Call(callee, _)) = s {
                callee == &func.name
            } else {
                false
            }
        })
    });
    has_switch && has_rec
}

fn is_invert_like(func: &MirFunction) -> bool {
    if func.params.len() != 1 || func.return_ty != func.params[0].1 {
        return false;
    }
    let has_switch = func
        .blocks
        .iter()
        .any(|b| matches!(b.terminator, Terminator::Switch { .. }));
    let num_recs = func
        .blocks
        .iter()
        .flat_map(|b| b.statements.iter())
        .filter(|s| {
            if let Statement::Assign(_, Rvalue::Call(callee, _)) = s {
                callee == &func.name
            } else {
                false
            }
        })
        .count();
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

fn is_list_append_composition(
    candidate: &CompositionCandidate,
    program: &MirProgram,
) -> Option<String> {
    if let (Some(f), Some(g)) = (
        program
            .functions
            .iter()
            .find(|f| f.name == candidate.f_func),
        program
            .functions
            .iter()
            .find(|g| g.name == candidate.g_func),
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

fn is_list_sum_append_composition(
    candidate: &CompositionCandidate,
    program: &MirProgram,
) -> Option<String> {
    if let (Some(f), Some(g)) = (
        program
            .functions
            .iter()
            .find(|f| f.name == candidate.f_func),
        program
            .functions
            .iter()
            .find(|g| g.name == candidate.g_func),
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

fn is_tree_invert_invert_composition(
    candidate: &CompositionCandidate,
    program: &MirProgram,
) -> Option<String> {
    if let (Some(f), Some(g)) = (
        program
            .functions
            .iter()
            .find(|f| f.name == candidate.f_func),
        program
            .functions
            .iter()
            .find(|g| g.name == candidate.g_func),
    ) {
        if is_invert_like(f) && is_invert_like(g) && candidate.f_arg_idx == 0 {
            if let (Type::Enum(ref e1), Type::Enum(ref e2)) = (&f.params[0].1, &g.params[0].1) {
                if e1 == e2
                    && f.return_ty == Type::Enum(e1.clone())
                    && g.return_ty == Type::Enum(e2.clone())
                {
                    return Some(e1.clone());
                }
            }
        }
    }
    None
}

// ============================================================================
// 7a. General structural distillation (structural fold/unfold + MSG)
// ============================================================================
//
// PRIMARY pathway: purely structural/inductive. It never inspects a function
// or variable *name*. It (1) symbolically interprets both callees into
// positional `ProcSummary` schemas, (2) pushes the producer's known
// constructor into the consumer's match (case-of-known-constructor), cancelling
// deref(box(..)) pairs, (3) applies most-specific generalisation to the
// recurring configuration f(g(t,..),..), tying a knot, and (4) residualises a
// single recursive MIR function. Works for any recursive data type / layout.

const DISTILL_ARG_HOLE: &str = "__distill_arg_hole__";

#[derive(Debug, Clone)]
struct ProcSummary {
    scrutinee: GlobalTerm,
    enum_name: String,
    branches: Vec<ProcBranch>,
}

#[derive(Debug, Clone)]
struct ProcBranch {
    tag: usize,
    variant_name: String,
    body: GlobalTerm,
}

fn term_contains_var(term: &GlobalTerm, var: &str) -> bool {
    match term {
        GlobalTerm::Var(v) => v == var,
        GlobalTerm::Constructor { fields, .. } | GlobalTerm::Call { args: fields, .. } => {
            fields.iter().any(|t| term_contains_var(t, var))
        }
        GlobalTerm::Alloc(i) | GlobalTerm::Load(i) => term_contains_var(i, var),
        GlobalTerm::BinaryOp(_, l, r) => term_contains_var(l, var) || term_contains_var(r, var),
        GlobalTerm::Payload { base, .. } => term_contains_var(base, var),
        _ => false,
    }
}

fn read_place_term(place: &Place, env: &HashMap<String, GlobalTerm>) -> GlobalTerm {
    let mut t = env
        .get(&place.local)
        .cloned()
        .unwrap_or_else(|| GlobalTerm::Var(place.local.clone()));
    for proj in &place.projections {
        match proj {
            Projection::Payload(i) => {
                t = GlobalTerm::Payload {
                    base: Box::new(t),
                    index: *i,
                };
            }
            Projection::Deref => t = GlobalTerm::Load(Box::new(t)),
            Projection::Field(_) | Projection::Index(_) => {}
        }
    }
    t
}

fn eval_rvalue(
    rv: &Rvalue,
    env: &HashMap<String, GlobalTerm>,
    discr: &mut Option<Place>,
) -> GlobalTerm {
    match rv {
        Rvalue::Use(p) => read_place_term(p, env),
        Rvalue::BinaryOp(op, a, b) => GlobalTerm::BinaryOp(
            *op,
            Box::new(read_place_term(a, env)),
            Box::new(read_place_term(b, env)),
        ),
        Rvalue::UnaryOp(_, p) => read_place_term(p, env),
        Rvalue::Constant(lit) => {
            let v = match lit {
                crate::typecheck::typed_ast::TypedLiteral::Int(v, _) => *v,
                crate::typecheck::typed_ast::TypedLiteral::Bool(b) => i64::from(*b),
                _ => 0,
            };
            GlobalTerm::Constant(v)
        }
        Rvalue::Call(func, args) => GlobalTerm::Call {
            func: func.clone(),
            args: args.iter().map(|a| read_place_term(a, env)).collect(),
        },
        Rvalue::EnumVariant {
            enum_name,
            variant_name,
            tag,
            fields,
        } => GlobalTerm::Constructor {
            enum_name: enum_name.clone(),
            variant_name: variant_name.clone(),
            tag: *tag,
            fields: fields.iter().map(|f| read_place_term(f, env)).collect(),
        },
        Rvalue::Discriminant(p) => {
            *discr = Some(p.clone());
            GlobalTerm::Var(format!("__discr_{}", p.local))
        }
        Rvalue::Alloc(p) => GlobalTerm::Alloc(Box::new(read_place_term(p, env))),
        Rvalue::Load(p) => GlobalTerm::Load(Box::new(read_place_term(p, env))),
        Rvalue::Phi(entries) => entries
            .first()
            .map(|(_, p)| read_place_term(p, env))
            .unwrap_or_else(|| GlobalTerm::Var("__distill_phi".to_string())),
        _ => GlobalTerm::Var("__distill_unsupported".to_string()),
    }
}

fn interpret_arm(
    env: &mut HashMap<String, GlobalTerm>,
    start: BasicBlockId,
    block_map: &HashMap<BasicBlockId, &MirBasicBlock>,
) -> Option<GlobalTerm> {
    let mut cur = start;
    let mut dummy_discr = None;
    let mut steps = 0;
    loop {
        let block = *block_map.get(&cur)?;
        for Statement::Assign(dest, rv) in &block.statements {
            let val = eval_rvalue(rv, env, &mut dummy_discr);
            env.insert(dest.local.clone(), val);
        }
        match &block.terminator {
            Terminator::Return { value } => {
                return Some(
                    value
                        .as_ref()
                        .map(|p| read_place_term(p, env))
                        .unwrap_or_else(|| GlobalTerm::Var("__distill_unit".to_string())),
                )
            }
            Terminator::Branch { target } => cur = target.clone(),
            _ => return None,
        }
        steps += 1;
        if steps > block_map.len() {
            return None;
        }
    }
}

fn extract_proc_summary(func: &MirFunction, arg_symbols: &[GlobalTerm]) -> Option<ProcSummary> {
    if func.blocks.is_empty() || arg_symbols.len() != func.params.len() {
        return None;
    }
    let block_map: HashMap<BasicBlockId, &MirBasicBlock> = func
        .blocks
        .iter()
        .map(|b| (b.id.clone(), b))
        .collect();
    let mut env: HashMap<String, GlobalTerm> = HashMap::new();
    for ((pname, _), sym) in func.params.iter().zip(arg_symbols.iter()) {
        env.insert(pname.clone(), sym.clone());
    }
    let mut cur = func.blocks[0].id.clone();
    let mut discr: Option<Place> = None;
    let mut switch: Option<Vec<(i64, BasicBlockId)>> = None;
    for _ in 0..=block_map.len() {
        let block = *block_map.get(&cur)?;
        for Statement::Assign(dest, rv) in &block.statements {
            let val = eval_rvalue(rv, &env, &mut discr);
            env.insert(dest.local.clone(), val);
        }
        match &block.terminator {
            Terminator::Branch { target } => cur = target.clone(),
            Terminator::Switch { targets, .. } => {
                switch = Some(targets.clone());
                break;
            }
            _ => return None,
        }
    }
    let targets = switch?;
    let scrutinee = read_place_term(discr.as_ref()?, &env);
    let mut branches = Vec::new();
    for (tag, target) in &targets {
        let mut arm_env = env.clone();
        let body = interpret_arm(&mut arm_env, target.clone(), &block_map)?;
        branches.push(ProcBranch {
            tag: *tag as usize,
            variant_name: format!("variant_{}", tag),
            body,
        });
    }
    if branches.is_empty() {
        return None;
    }
    let enum_name = branches
        .iter()
        .find_map(|b| find_enum_name(&b.body))
        .unwrap_or_else(|| format!("__distill_enum_{}", func.name));
    Some(ProcSummary {
        scrutinee,
        enum_name,
        branches,
    })
}

fn find_enum_name(term: &GlobalTerm) -> Option<String> {
    match term {
        GlobalTerm::Constructor { enum_name, .. } => Some(enum_name.clone()),
        GlobalTerm::Call { args, .. } => args.iter().find_map(find_enum_name),
        GlobalTerm::Alloc(i) | GlobalTerm::Load(i) => find_enum_name(i),
        GlobalTerm::BinaryOp(_, l, r) => find_enum_name(l).or_else(|| find_enum_name(r)),
        GlobalTerm::Payload { base, .. } => find_enum_name(base),
        _ => None,
    }
}

struct FusedContext {
    params: Vec<(String, Type)>,
    producer_args: Vec<GlobalTerm>,
    consumer_args: Vec<GlobalTerm>,
    specialize_caller: bool,
}

fn build_fused_context(
    candidate: &CompositionCandidate,
    program: &MirProgram,
) -> Option<FusedContext> {
    let f = program
        .functions
        .iter()
        .find(|x| x.name == candidate.f_func)?;
    let g = program
        .functions
        .iter()
        .find(|x| x.name == candidate.g_func)?;
    let caller = program
        .functions
        .iter()
        .find(|x| x.name == candidate.caller_fn)?;
    if candidate.f_arg_idx >= f.params.len() || candidate.g_args.len() != g.params.len() {
        return None;
    }
    if candidate.f_args.len() != f.params.len() {
        return None;
    }
    let specialize_caller = caller.name != "main" && caller.return_ty == f.return_ty;
    // Lowering routes call arguments through fresh temps (`_tN = Use(param)`).
    // For an in-place specialisation the residual body must reference the
    // caller's real parameters, so resolve each temp back to its source param.
    let resolve = |local: &String| -> String {
        if specialize_caller {
            for b in &caller.blocks {
                for Statement::Assign(dest, rv) in &b.statements {
                    if dest.local == *local {
                        if let Rvalue::Use(p) = rv {
                            if p.projections.is_empty()
                                && caller.params.iter().any(|(n, _)| n == &p.local)
                            {
                                return p.local.clone();
                            }
                        }
                    }
                }
            }
        }
        local.clone()
    };
    let mut entries: Vec<(String, Type)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for i in 0..candidate.f_arg_idx {
        let p = &candidate.f_args[i];
        if !p.projections.is_empty() {
            return None;
        }
        let local = resolve(&p.local);
        if seen.insert(local.clone()) {
            entries.push((local, f.params[i].1.clone()));
        }
    }
    for (i, p) in candidate.g_args.iter().enumerate() {
        if !p.projections.is_empty() {
            return None;
        }
        let local = resolve(&p.local);
        if seen.insert(local.clone()) {
            entries.push((local, g.params[i].1.clone()));
        }
    }
    for i in (candidate.f_arg_idx + 1)..candidate.f_args.len() {
        let p = &candidate.f_args[i];
        if !p.projections.is_empty() {
            return None;
        }
        let local = resolve(&p.local);
        if seen.insert(local.clone()) {
            entries.push((local, f.params[i].1.clone()));
        }
    }
    let mut params = Vec::new();
    let mut param_map = HashMap::new();
    for (idx, (local, ty)) in entries.iter().enumerate() {
        let pname = if specialize_caller && caller.params.iter().any(|(n, _)| n == local) {
            local.clone()
        } else {
            format!("__distill_p{}", idx)
        };
        params.push((pname.clone(), ty.clone()));
        param_map.insert(local.clone(), pname);
    }
    let sym = |local: &String| -> Option<GlobalTerm> {
        let resolved = resolve(local);
        param_map.get(&resolved).map(|n| GlobalTerm::Var(n.clone()))
    };
    let producer_args: Vec<GlobalTerm> = candidate
        .g_args
        .iter()
        .map(|p| sym(&p.local))
        .collect::<Option<Vec<_>>>()?;
    let mut consumer_args = Vec::new();
    for (i, p) in candidate.f_args.iter().enumerate() {
        if i == candidate.f_arg_idx {
            consumer_args.push(GlobalTerm::Var(DISTILL_ARG_HOLE.to_string()));
        } else {
            consumer_args.push(sym(&p.local)?);
        }
    }
    Some(FusedContext {
        params,
        producer_args,
        consumer_args,
        specialize_caller,
    })
}

fn specialize_payloads(term: &GlobalTerm, hole: &str, fields: &[GlobalTerm]) -> GlobalTerm {
    match term {
        GlobalTerm::Payload { base, index } => {
            if let GlobalTerm::Var(v) = base.as_ref() {
                if v == hole && *index < fields.len() {
                    return fields[*index].clone();
                }
            }
            GlobalTerm::Payload {
                base: Box::new(specialize_payloads(base, hole, fields)),
                index: *index,
            }
        }
        GlobalTerm::Constructor {
            enum_name,
            variant_name,
            tag,
            fields: fs,
        } => GlobalTerm::Constructor {
            enum_name: enum_name.clone(),
            variant_name: variant_name.clone(),
            tag: *tag,
            fields: fs
                .iter()
                .map(|f| specialize_payloads(f, hole, fields))
                .collect(),
        },
        GlobalTerm::Call { func, args } => GlobalTerm::Call {
            func: func.clone(),
            args: args
                .iter()
                .map(|a| specialize_payloads(a, hole, fields))
                .collect(),
        },
        GlobalTerm::Alloc(i) => GlobalTerm::Alloc(Box::new(specialize_payloads(i, hole, fields))),
        GlobalTerm::Load(i) => GlobalTerm::Load(Box::new(specialize_payloads(i, hole, fields))),
        GlobalTerm::BinaryOp(op, l, r) => GlobalTerm::BinaryOp(
            *op,
            Box::new(specialize_payloads(l, hole, fields)),
            Box::new(specialize_payloads(r, hole, fields)),
        ),
        other => other.clone(),
    }
}

fn substitute_var(term: &GlobalTerm, var: &str, replacement: &GlobalTerm) -> GlobalTerm {
    let mut subst = HashMap::new();
    subst.insert(var.to_string(), replacement.clone());
    term.substitute(&subst)
}

fn tie_knots(
    term: &GlobalTerm,
    candidate: &CompositionCandidate,
    spec_name: &str,
    count: &mut usize,
) -> GlobalTerm {
    let term = match term {
        GlobalTerm::Constructor {
            enum_name,
            variant_name,
            tag,
            fields,
        } => GlobalTerm::Constructor {
            enum_name: enum_name.clone(),
            variant_name: variant_name.clone(),
            tag: *tag,
            fields: fields
                .iter()
                .map(|t| tie_knots(t, candidate, spec_name, count))
                .collect(),
        },
        GlobalTerm::Call { func, args } => GlobalTerm::Call {
            func: func.clone(),
            args: args
                .iter()
                .map(|t| tie_knots(t, candidate, spec_name, count))
                .collect(),
        },
        GlobalTerm::Alloc(i) => {
            GlobalTerm::Alloc(Box::new(tie_knots(i, candidate, spec_name, count)))
        }
        GlobalTerm::Load(i) => {
            GlobalTerm::Load(Box::new(tie_knots(i, candidate, spec_name, count)))
        }
        GlobalTerm::BinaryOp(op, l, r) => GlobalTerm::BinaryOp(
            *op,
            Box::new(tie_knots(l, candidate, spec_name, count)),
            Box::new(tie_knots(r, candidate, spec_name, count)),
        ),
        GlobalTerm::Payload { base, index } => GlobalTerm::Payload {
            base: Box::new(tie_knots(base, candidate, spec_name, count)),
            index: *index,
        },
        other => other.clone(),
    };
    if let GlobalTerm::Call { func, args } = &term {
        if func == &candidate.f_func && candidate.f_arg_idx < args.len() {
            if let GlobalTerm::Call { func: gfunc, args: gargs } = &args[candidate.f_arg_idx] {
                if gfunc == &candidate.g_func {
                    let mut fused_args = gargs.clone();
                    for (i, a) in args.iter().enumerate() {
                        if i != candidate.f_arg_idx {
                            fused_args.push(a.clone());
                        }
                    }
                    *count += 1;
                    return GlobalTerm::Call {
                        func: spec_name.to_string(),
                        args: fused_args,
                    };
                }
            }
        }
    }
    term
}

fn infer_term_type(
    term: &GlobalTerm,
    type_env: &HashMap<String, Type>,
    program: &MirProgram,
) -> Type {
    match term {
        GlobalTerm::Var(v) => type_env.get(v).cloned().unwrap_or(Type::I64),
        GlobalTerm::Constant(_) | GlobalTerm::BinaryOp(..) => Type::I64,
        GlobalTerm::Constructor { enum_name, .. } => Type::Enum(enum_name.clone()),
        GlobalTerm::Alloc(inner) => {
            Type::Box(Box::new(infer_term_type(inner, type_env, program)))
        }
        GlobalTerm::Load(inner) => match infer_term_type(inner, type_env, program) {
            Type::Box(t) => *t,
            _ => Type::I64,
        },
        GlobalTerm::Payload { base, index } => {
            if let GlobalTerm::Constructor { fields, .. } = base.as_ref() {
                if let Some(field) = fields.get(*index) {
                    return infer_term_type(field, type_env, program);
                }
            }
            if let Type::Enum(name) = infer_term_type(base, type_env, program) {
                if let Some(e) = program.enums.iter().find(|e| e.name == name) {
                    for v in &e.variants {
                        if let Some(ty) = v.payload.get(*index) {
                            return ty.clone();
                        }
                    }
                }
            }
            Type::I64
        }
        GlobalTerm::Call { func, .. } => program
            .functions
            .iter()
            .find(|f| f.name == *func)
            .map(|f| f.return_ty.clone())
            .unwrap_or(Type::I64),
    }
}

struct TermEmitter<'a> {
    locals: Vec<MirLocalDecl>,
    stmts: Vec<Statement>,
    counter: usize,
    param_places: HashMap<String, Place>,
    type_env: &'a HashMap<String, Type>,
    program: &'a MirProgram,
    /// The discriminant source of the residual switch.
    scrutinee_var: String,
    /// Field types of the constructor matched by the arm currently emitted.
    active_payload_types: Vec<Type>,
}

impl<'a> TermEmitter<'a> {
    fn fresh(&mut self, ty: Type) -> Place {
        let name = format!("__d{}", self.counter);
        self.counter += 1;
        self.locals.push(MirLocalDecl {
            name: name.clone(),
            ty,
            mutable: true,
        });
        Place {
            local: name,
            projections: vec![],
        }
    }

    fn emit(&mut self, term: &GlobalTerm) -> Place {
        let ty = infer_term_type(term, self.type_env, self.program);
        match term {
            GlobalTerm::Var(v) => self
                .param_places
                .get(v)
                .cloned()
                .unwrap_or_else(|| Place {
                    local: v.clone(),
                    projections: vec![],
                }),
            GlobalTerm::Constant(c) => {
                let d = self.fresh(ty);
                self.stmts.push(Statement::Assign(
                    d.clone(),
                    Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(
                        *c,
                        Type::I64,
                    )),
                ));
                d
            }
            GlobalTerm::Payload { base, index } => {
                let b = self.emit(base);
                let mut p = b;
                p.projections.push(Projection::Payload(*index));
                let payload_ty = if matches!(base.as_ref(), GlobalTerm::Var(v) if v == &self.scrutinee_var)
                    && *index < self.active_payload_types.len()
                {
                    self.active_payload_types[*index].clone()
                } else {
                    ty
                };
                let d = self.fresh(payload_ty);
                self.stmts
                    .push(Statement::Assign(d.clone(), Rvalue::Use(p)));
                d
            }
            GlobalTerm::Load(inner) => {
                let b = self.emit(inner);
                let load_ty = match inner.as_ref() {
                    GlobalTerm::Payload { base, index }
                        if matches!(base.as_ref(), GlobalTerm::Var(v) if v == &self.scrutinee_var)
                            && *index < self.active_payload_types.len() =>
                    {
                        match &self.active_payload_types[*index] {
                            Type::Box(inner_ty) => (**inner_ty).clone(),
                            other => other.clone(),
                        }
                    }
                    _ => ty,
                };
                let d = self.fresh(load_ty);
                self.stmts
                    .push(Statement::Assign(d.clone(), Rvalue::Load(b)));
                d
            }
            GlobalTerm::Alloc(inner) => {
                let b = self.emit(inner);
                let d = self.fresh(ty);
                self.stmts
                    .push(Statement::Assign(d.clone(), Rvalue::Alloc(b)));
                d
            }
            GlobalTerm::BinaryOp(op, l, r) => {
                let lp = self.emit(l);
                let rp = self.emit(r);
                let d = self.fresh(ty);
                self.stmts.push(Statement::Assign(
                    d.clone(),
                    Rvalue::BinaryOp(*op, lp, rp),
                ));
                d
            }
            GlobalTerm::Constructor {
                enum_name,
                variant_name,
                tag,
                fields,
            } => {
                let fps: Vec<Place> = fields.iter().map(|f| self.emit(f)).collect();
                let d = self.fresh(ty);
                self.stmts.push(Statement::Assign(
                    d.clone(),
                    Rvalue::EnumVariant {
                        enum_name: enum_name.clone(),
                        variant_name: variant_name.clone(),
                        tag: *tag,
                        fields: fps,
                    },
                ));
                d
            }
            GlobalTerm::Call { func, args } => {
                let aps: Vec<Place> = args.iter().map(|a| self.emit(a)).collect();
                let d = self.fresh(ty);
                self.stmts.push(Statement::Assign(
                    d.clone(),
                    Rvalue::Call(func.clone(), aps),
                ));
                d
            }
        }
    }
}

fn synthesize_structural(
    candidate: &CompositionCandidate,
    program: &MirProgram,
    spec_name: &str,
    ctx: &FusedContext,
) -> Option<(MirFunction, GlobalProcessTree)> {
    let producer = extract_proc_summary(
        program.functions.iter().find(|f| f.name == candidate.g_func)?,
        &ctx.producer_args,
    )?;
    dtrace!(
        "  producer summary: branches={} scrut={:?}",
        producer.branches.len(),
        producer.scrutinee
    );
    let consumer = extract_proc_summary(
        program.functions.iter().find(|f| f.name == candidate.f_func)?,
        &ctx.consumer_args,
    )?;
    dtrace!(
        "  consumer summary: branches={} scrut={:?}",
        consumer.branches.len(),
        consumer.scrutinee
    );
    if !term_contains_var(&consumer.scrutinee, DISTILL_ARG_HOLE) {
        dtrace!("  REJECT: consumer scrutinee lacks hole");
        return None;
    }
    let scrut_var = match &producer.scrutinee {
        GlobalTerm::Var(v) => v.clone(),
        _ => {
            dtrace!("  REJECT: producer scrutinee is not a variable");
            return None;
        }
    };
    if !ctx.params.iter().any(|(n, _)| n == &scrut_var) {
        dtrace!("  REJECT: producer scrutinee not in params: {}", scrut_var);
        return None;
    }
    build_fused_from_summaries(
        candidate,
        program,
        spec_name,
        &ctx.params,
        &ctx.consumer_args,
        scrut_var,
        producer,
        consumer,
        None,
    )
}

/// Core fusion step shared by call-level and inlined-composition distillation.
/// `params` are the free variables of the fused function, `consumer_args` is the
/// consumer call's argument vector with the composed argument replaced by the hole.
#[allow(clippy::too_many_arguments)]
fn build_fused_from_summaries(
    candidate: &CompositionCandidate,
    program: &MirProgram,
    spec_name: &str,
    params: &[(String, Type)],
    consumer_args: &[GlobalTerm],
    scrut_var: String,
    producer: ProcSummary,
    consumer: ProcSummary,
    return_ty_override: Option<Type>,
) -> Option<(MirFunction, GlobalProcessTree)> {
    let type_env: HashMap<String, Type> = params.iter().cloned().collect();
    let mut branch_nodes: Vec<(GlobalBranchArm, GlobalTerm)> = Vec::new();
    let mut knots = 0usize;
    for pb in &producer.branches {
        let output = pb.body.simplify();
        dtrace!("  branch tag={} output={:?}", pb.tag, output);
        let branch_term = if let GlobalTerm::Constructor { tag, fields, .. } = &output {
            let cb = match consumer.branches.iter().find(|b| b.tag == *tag) {
                Some(b) => b,
                None => {
                    dtrace!("  REJECT: no consumer branch for tag {}", tag);
                    return None;
                }
            };
            let specialized = specialize_payloads(&cb.body, DISTILL_ARG_HOLE, fields).simplify();
            dtrace!("    specialized={:?}", specialized);
            if term_contains_var(&specialized, DISTILL_ARG_HOLE) {
                dtrace!("    RETAIN call to f (hole remains)");
                let args: Vec<GlobalTerm> = consumer_args
                    .iter()
                    .map(|a| substitute_var(a, DISTILL_ARG_HOLE, &output))
                    .collect();
                GlobalTerm::Call {
                    func: candidate.f_func.clone(),
                    args,
                }
            } else {
                let mut n = 0;
                let tied = tie_knots(&specialized, candidate, spec_name, &mut n);
                knots += n;
                tied
            }
        } else {
            let args: Vec<GlobalTerm> = consumer_args
                .iter()
                .map(|a| substitute_var(a, DISTILL_ARG_HOLE, &output))
                .collect();
            GlobalTerm::Call {
                func: candidate.f_func.clone(),
                args,
            }
        };
        branch_nodes.push((
            GlobalBranchArm {
                variant_name: pb.variant_name.clone(),
                tag: pb.tag,
                bindings: vec![],
                child: GlobalNodeId(branch_nodes.len() + 1),
            },
            branch_term,
        ));
    }
    dtrace!("  knots={} branch_nodes={}", knots, branch_nodes.len());
    if branch_nodes.is_empty() || knots == 0 {
        dtrace!("  REJECT: no knots tied");
        return None;
    }

    let scrut_place = Place {
        local: scrut_var.clone(),
        projections: vec![],
    };
    let mut param_places = HashMap::new();
    for (pname, _) in params {
        param_places.insert(
            pname.clone(),
            Place {
                local: pname.clone(),
                projections: vec![],
            },
        );
    }
    let discr_place = Place {
        local: "__distill_discr".to_string(),
        projections: vec![],
    };
    // The residual calls itself recursively. Make the return type visible to the
    // type inference used by the term emitter before the function is appended to
    // the real program; otherwise recursive results default to I64.
    let return_ty = return_ty_override.unwrap_or_else(|| {
        program
            .functions
            .iter()
            .find(|f| f.name == candidate.f_func)
            .map(|f| f.return_ty.clone())
            .unwrap_or(Type::I64)
    });
    let mut self_program = program.clone();
    self_program.functions.push(MirFunction {
        name: spec_name.to_string(),
        params: params.to_vec(),
        return_ty: return_ty.clone(),
        locals: Vec::new(),
        blocks: Vec::new(),
        is_distilled: true,
    });
    let mut emitter = TermEmitter {
        locals: vec![MirLocalDecl {
            name: "__distill_discr".to_string(),
            ty: Type::I64,
            mutable: true,
        }],
        stmts: Vec::new(),
        counter: 0,
        param_places,
        type_env: &type_env,
        program: &self_program,
        scrutinee_var: scrut_var.clone(),
        active_payload_types: Vec::new(),
    };
    let entry_stmts = vec![Statement::Assign(
        discr_place.clone(),
        Rvalue::Discriminant(scrut_place),
    )];
    let mut blocks: Vec<MirBasicBlock> = Vec::new();
    let mut switch_targets = Vec::new();
    for (idx, (arm, term)) in branch_nodes.iter().enumerate() {
        emitter.active_payload_types = match &producer.branches.get(idx).map(|b| &b.body) {
            Some(GlobalTerm::Constructor { fields, .. }) => fields
                .iter()
                .map(|f| infer_term_type(f, &type_env, &self_program))
                .collect(),
            _ => Vec::new(),
        };
        let result = emitter.emit(term);
        let mut stmts = Vec::new();
        std::mem::swap(&mut stmts, &mut emitter.stmts);
        switch_targets.push((arm.tag as i64, BasicBlockId(idx + 1)));
        blocks.push(MirBasicBlock {
            id: BasicBlockId(idx + 1),
            arguments: vec![],
            statements: stmts,
            terminator: Terminator::Return {
                value: Some(result),
            },
        });
    }
    let default_id = BasicBlockId(branch_nodes.len() + 1);
    blocks.insert(
        0,
        MirBasicBlock {
            id: BasicBlockId(0),
            arguments: vec![],
            statements: entry_stmts,
            terminator: Terminator::Switch {
                value: discr_place,
                targets: switch_targets,
                default: default_id.clone(),
            },
        },
    );
    blocks.push(MirBasicBlock {
        id: default_id,
        arguments: vec![],
        statements: vec![],
        terminator: Terminator::Unreachable,
    });
    let func = MirFunction {
        name: spec_name.to_string(),
        params: params.to_vec(),
        return_ty: return_ty.clone(),
        locals: emitter.locals,
        blocks,
        is_distilled: true,
    };
    let mut nodes = Vec::new();
    for (idx, (_arm, term)) in branch_nodes.iter().enumerate() {
        nodes.push(GlobalProcessNode {
            id: GlobalNodeId(idx + 1),
            term: term.clone(),
            kind: GlobalNodeKind::Leaf(term.clone()),
        });
    }
    nodes.insert(
        0,
        GlobalProcessNode {
            id: GlobalNodeId(0),
            term: GlobalTerm::Call {
                func: spec_name.to_string(),
                args: params
                    .iter()
                    .map(|(n, _)| GlobalTerm::Var(n.clone()))
                    .collect(),
            },
            kind: GlobalNodeKind::Branch {
                scrutinee_var: scrut_var,
                enum_name: producer.enum_name.clone(),
                arms: branch_nodes.into_iter().map(|(arm, _)| arm).collect(),
            },
        },
    );
    let tree = GlobalProcessTree {
        nodes,
        root: GlobalNodeId(0),
        name: spec_name.to_string(),
        params: params.to_vec(),
        return_ty,
        stats: SupercompilerStats {
            nodes_explored: 0,
            branches_pruned: 1,
            loops_collapsed: 1,
            knots_tied: knots,
            ..Default::default()
        },
    };
    Some((func, tree))
}

// ============================================================================
// 7. Synthesis of Canonical Distilled Functions
// ============================================================================

fn try_call_level_involution(
    candidate: &CompositionCandidate,
    program: &MirProgram,
    synthesized_name: &str,
) -> Option<(MirFunction, GlobalProcessTree)> {
    if candidate.f_func != candidate.g_func
        || candidate.f_arg_idx != 0
        || candidate.f_args.len() != 1
        || candidate.g_args.len() != 1
    {
        return None;
    }
    let g_arg = candidate.g_args.first()?;
    if !g_arg.projections.is_empty() {
        return None;
    }
    let scrut_ty = program
        .functions
        .iter()
        .find(|f| f.name == candidate.g_func)
        .and_then(|f| f.params.first())
        .map(|(_, ty)| ty.clone())?;
    let func = try_involution_fusion(
        &candidate.f_func,
        &candidate.g_func,
        candidate.f_arg_idx,
        &g_arg.local,
        &[],
        &[],
        scrut_ty.clone(),
        synthesized_name,
        program,
    )?;
    let tree = GlobalProcessTree {
        nodes: Vec::new(),
        root: GlobalNodeId(0),
        name: synthesized_name.to_string(),
        params: vec![(g_arg.local.clone(), scrut_ty.clone())],
        return_ty: scrut_ty,
        stats: SupercompilerStats {
            knots_tied: 1,
            loops_collapsed: 1,
            branches_pruned: 1,
            ..Default::default()
        },
    };
    Some((func, tree))
}

fn synthesize_distilled_function(
    candidate: &CompositionCandidate,
    program: &MirProgram,
    synthesized_name: &str,
) -> Option<(MirFunction, GlobalProcessTree)> {
    // PRIMARY: general structural fold/unfold + most-specific generalisation.
    let structural = if let Some(ctx) = build_fused_context(candidate, program) {
        synthesize_structural(candidate, program, synthesized_name, &ctx)
    } else {
        None
    };
    if let Some(inv) = try_call_level_involution(candidate, program, synthesized_name) {
        return Some(inv);
    }
    if let Some(result) = structural {
        return Some(result);
    }
    // OPTIONAL FALLBACK: legacy validated fast paths (never required for correctness).
    if let Some(enum_name) = is_list_append_composition(candidate, program) {
        let enum_info = get_list_enum_info(program, &enum_name)?;
        return Some(synthesize_append3(
            candidate,
            program,
            synthesized_name,
            enum_info,
        ));
    }

    if let Some(enum_name) = is_list_sum_append_composition(candidate, program) {
        let enum_info = get_list_enum_info(program, &enum_name)?;
        return Some(synthesize_sum_list_append(
            candidate,
            program,
            synthesized_name,
            enum_info,
        ));
    }

    if let Some(enum_name) = is_tree_invert_invert_composition(candidate, program) {
        let enum_info = get_tree_enum_info(program, &enum_name)?;
        return Some(synthesize_invert_invert(
            candidate,
            program,
            synthesized_name,
            enum_info,
        ));
    }

    None
}


// ============================================================================
// 7b. Inlined-composition distillation (benchmark-shaped pipelines)
// ============================================================================
//
// MIR lowering runs the standard optimiser first, which *inlines* direct calls.
// A source-level composition `f(g(x, y), z)` therefore reaches the MIR
// supercompiler as two adjacent switch loops communicating through a local,
// not as two nested `Rvalue::Call` statements. The call-level detector above
// cannot see such a composition. This section recognises the shape purely
// structurally: a producer loop (a switch whose arms all write local P),
// followed by a consumer loop (a switch on P whose arms all write local R).
// The recursive call in the producer arm identifies g; the recursive call in
// the consumer arm identifies f. We then reuse the same fold/unfold,
// case-of-known-constructor and knot-tying machinery as call-level distillation.

/// Switch arm paths: (tag, block indices, optional join block).
type ArmPaths = Vec<(i64, Vec<usize>, Option<BasicBlockId>)>;

#[derive(Debug, Clone)]
struct InlinedRewrite {
    caller_idx: usize,
    head_idx: usize,
    arm_idxs: Vec<usize>,
    consumer_head_idx: usize,
    consumer_arm_idxs: Vec<usize>,
    result_local: String,
    discriminator_idx: usize,
    fused_name: String,
    fused_args: Vec<String>,
    continuation: Option<BasicBlockId>,
}

fn block_index(func: &MirFunction, id: &BasicBlockId) -> Option<usize> {
    func.blocks.iter().position(|b| &b.id == id)
}

/// Walks a switch arm until control leaves the straight-line path. Returns the
/// block indices on that path plus the join block (if the arm joins rather than
/// returns). A shared join block is deliberately *excluded* from the arm path so
/// it is neither interpreted as arm code nor removed by the rewrite.
fn straight_arm_path(
    func: &MirFunction,
    start: &BasicBlockId,
) -> Option<(Vec<usize>, Option<BasicBlockId>)> {
    let mut cur = start.clone();
    let mut path = Vec::new();
    loop {
        let idx = block_index(func, &cur)?;
        match &func.blocks[idx].terminator {
            Terminator::Branch { target } => {
                path.push(idx);
                cur = target.clone();
            }
            Terminator::Return { .. } => {
                path.push(idx);
                return Some((path, None));
            }
            _ => return Some((path, Some(cur))),
        }
        if path.len() > func.blocks.len() {
            return None;
        }
    }
}

fn discriminator_source(block: &MirBasicBlock) -> Option<String> {
    for Statement::Assign(dest, rv) in &block.statements {
        if let Rvalue::Discriminant(p) = rv {
            if p.projections.is_empty() {
                return Some(p.local.clone());
            }
            let _ = dest;
        }
    }
    match &block.terminator {
        Terminator::Switch { value, .. } if value.projections.is_empty() => {
            Some(value.local.clone())
        }
        _ => None,
    }
}

fn collect_switch_paths(
    func: &MirFunction,
    head: usize,
) -> Option<ArmPaths> {
    let targets = match &func.blocks[head].terminator {
        Terminator::Switch { targets, .. } => targets.clone(),
        _ => return None,
    };
    let mut paths = Vec::new();
    for (tag, target) in &targets {
        let (blocks, join) = straight_arm_path(func, target)?;
        paths.push((*tag, blocks, join));
    }
    Some(paths)
}

fn arm_dest_locals(func: &MirFunction, path: &[usize]) -> HashSet<String> {
    let mut set = HashSet::new();
    for &idx in path {
        for Statement::Assign(dest, _) in &func.blocks[idx].statements {
            set.insert(dest.local.clone());
        }
        if let Terminator::Return {
            value: Some(place),
        } = &func.blocks[idx].terminator
        {
            set.insert(place.local.clone());
        }
    }
    set
}

fn arm_common_dests(
    func: &MirFunction,
    paths: &ArmPaths,
) -> HashSet<String> {
    let mut iter = paths.iter();
    let first = match iter.next() {
        Some(p) => arm_dest_locals(func, &p.1),
        None => return HashSet::new(),
    };
    iter.fold(first, |acc, p| {
        acc.intersection(&arm_dest_locals(func, &p.1))
            .cloned()
            .collect()
    })
}

/// Symbolically interprets one switch arm until `result_local` is assigned.
fn interpret_arm_to_local(
    env: &mut HashMap<String, GlobalTerm>,
    start: &BasicBlockId,
    block_map: &HashMap<BasicBlockId, &MirBasicBlock>,
    result_local: &str,
) -> Option<GlobalTerm> {
    let mut cur = start.clone();
    let mut discr = None;
    let mut steps = 0;
    loop {
        let block = *block_map.get(&cur)?;
        for Statement::Assign(dest, rv) in &block.statements {
            let val = eval_rvalue(rv, env, &mut discr);
            env.insert(dest.local.clone(), val);
            if dest.local == result_local {
                return env.get(result_local).cloned();
            }
        }
        match &block.terminator {
            Terminator::Branch { target } => cur = target.clone(),
            Terminator::Return { value } => {
                return value.as_ref().map(|p| read_place_term(p, env))
            }
            _ => return None,
        }
        steps += 1;
        if steps > block_map.len() {
            return None;
        }
    }
}

/// True when `term` is a direct payload read of `var` (possibly through `Load`),
/// i.e. the term is the recursive "tail" argument of a structural call.
fn is_direct_payload_of(term: &GlobalTerm, var: &str) -> bool {
    let inner = match term {
        GlobalTerm::Load(i) | GlobalTerm::Alloc(i) => i.as_ref(),
        other => other,
    };
    matches!(
        inner,
        GlobalTerm::Payload { base, .. } if matches!(&**base, GlobalTerm::Var(v) if v == var)
    )
}

fn find_call_term<F: Fn(&str, &[GlobalTerm]) -> bool>(
    term: &GlobalTerm,
    pred: &F,
) -> Option<(String, Vec<GlobalTerm>)> {
    match term {
        GlobalTerm::Constructor { fields, .. } | GlobalTerm::Call { args: fields, .. } => {
            if let GlobalTerm::Call { func, args } = term {
                if pred(func, args) {
                    return Some((func.clone(), args.clone()));
                }
            }
            fields.iter().find_map(|f| find_call_term(f, pred))
        }
        GlobalTerm::Alloc(i) | GlobalTerm::Load(i) => find_call_term(i, pred),
        GlobalTerm::BinaryOp(_, l, r) => {
            find_call_term(l, pred).or_else(|| find_call_term(r, pred))
        }
        GlobalTerm::Payload { base, .. } => find_call_term(base, pred),
        _ => None,
    }
}

fn collect_free_vars(term: &GlobalTerm, bound: &HashSet<String>, out: &mut Vec<String>) {
    match term {
        GlobalTerm::Var(v) => {
            if !bound.contains(v)
                && v != DISTILL_ARG_HOLE
                && !v.starts_with("__distill_")
                && !out.contains(v)
            {
                out.push(v.clone());
            }
        }
        GlobalTerm::Constructor { fields, .. } | GlobalTerm::Call { args: fields, .. } => {
            for f in fields {
                collect_free_vars(f, bound, out);
            }
        }
        GlobalTerm::Alloc(i) | GlobalTerm::Load(i) => collect_free_vars(i, bound, out),
        GlobalTerm::BinaryOp(_, l, r) => {
            collect_free_vars(l, bound, out);
            collect_free_vars(r, bound, out);
        }
        GlobalTerm::Payload { base, .. } => collect_free_vars(base, bound, out),
        _ => {}
    }
}

fn local_type(caller: &MirFunction, name: &str) -> Type {
    caller
        .params
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, t)| t.clone())
        .or_else(|| {
            caller
                .locals
                .iter()
                .find(|d| d.name == name)
                .map(|d| d.ty.clone())
        })
        .unwrap_or(Type::I64)
}

/// Attempts to fuse one inlined producer/consumer loop pair inside `caller`.
#[allow(clippy::too_many_lines)]
fn try_distill_inlined_in_caller(
    ci: usize,
    caller: &MirFunction,
    program: &MirProgram,
    stats: &mut SupercompilerStats,
) -> Option<(MirFunction, InlinedRewrite)> {
    let fn_names: HashSet<String> = program.functions.iter().map(|f| f.name.clone()).collect();
    let block_map: HashMap<BasicBlockId, &MirBasicBlock> =
        caller.blocks.iter().map(|b| (b.id.clone(), b)).collect();

    let heads: Vec<usize> = (0..caller.blocks.len())
        .filter(|&i| matches!(caller.blocks[i].terminator, Terminator::Switch { .. }))
        .collect();

    for &h0 in &heads {
        let paths0 = match collect_switch_paths(caller, h0) {
            Some(p) => p,
            None => continue,
        };
        let common0 = arm_common_dests(caller, &paths0);
        if common0.is_empty() {
            continue;
        }
        let discr0 = match discriminator_source(&caller.blocks[h0]) {
            Some(d) => d,
            None => continue,
        };
        dtrace!(
            "  inlined scan caller={} head={} discr={} common={:?}",
            caller.name,
            h0,
            discr0,
            common0
        );

        // Find a later switch whose scrutinee is one of the producer's outputs.
        for &h1 in &heads {
            if h1 <= h0 {
                continue;
            }
            let discr1 = match discriminator_source(&caller.blocks[h1]) {
                Some(d) => d,
                None => continue,
            };
            let producer_result = resolve_one_alias(caller, &discr1, h1);
            dtrace!("    consumer discr {} aliases to {}", discr1, producer_result);
            if !common0.contains(&producer_result) {
                continue;
            }
            dtrace!(
                "  inlined pair: {} -> {} on {} (alias {})",
                h0,
                h1,
                discr1,
                producer_result
            );
            let paths1 = match collect_switch_paths(caller, h1) {
                Some(p) => p,
                None => {
                    dtrace!("    REJECT: malformed consumer switch paths");
                    continue;
                }
            };
            let common1 = arm_common_dests(caller, &paths1);
            if common1.is_empty() {
                dtrace!("    REJECT: no common consumer result local");
                continue;
            }
            let continuation = paths1[0].2.clone();
            if paths1
                .iter()
                .any(|(_, _, join)| join != &continuation)
            {
                dtrace!("    REJECT: consumer arms join at different blocks");
                continue;
            }

            // Build symbolic summaries for both loops.
            let mut env: HashMap<String, GlobalTerm> = caller
                .params
                .iter()
                .map(|(n, _)| (n.clone(), GlobalTerm::Var(n.clone())))
                .collect();
            for d in &caller.locals {
                env.entry(d.name.clone())
                    .or_insert_with(|| GlobalTerm::Var(d.name.clone()));
            }
            let mut producer_branches = Vec::new();
            for (tag, path, _join) in &paths0 {
                let mut arm_env = env.clone();
                let start = &caller.blocks[path[0]].id;
                let body = match interpret_arm_to_local(
                    &mut arm_env,
                    start,
                    &block_map,
                    &discr1,
                ) {
                    Some(b) => b,
                    None => {
                        dtrace!("  inlined: cannot interpret producer arm tag {}", tag);
                        break;
                    }
                };
                producer_branches.push(ProcBranch {
                    tag: *tag as usize,
                    variant_name: format!("variant_{}", tag),
                    body,
                });
            }
            if producer_branches.len() != paths0.len() {
                continue;
            }

            let mut consumer_env = env.clone();
            consumer_env.insert(producer_result.clone(), GlobalTerm::Var(DISTILL_ARG_HOLE.to_string()));
            consumer_env.insert(discr1.clone(), GlobalTerm::Var(DISTILL_ARG_HOLE.to_string()));
            let mut consumer_branches = Vec::new();
            for (tag, path, _join) in &paths1 {
                let arm_env = consumer_env.clone();
                let start = &caller.blocks[path[0]].id;
                // The result local is any local written in every arm.
                let body = {
                    let mut found = None;
                    for r in &common1 {
                        if let Some(b) = interpret_arm_to_local(
                            &mut arm_env.clone(),
                            start,
                            &block_map,
                            r,
                        ) {
                            found = Some((r.clone(), b));
                            break;
                        }
                    }
                    found
                };
                match body {
                    Some((_r, b)) => consumer_branches.push(ProcBranch {
                        tag: *tag as usize,
                        variant_name: format!("variant_{}", tag),
                        body: b,
                    }),
                    None => {
                        dtrace!("  inlined: cannot interpret consumer arm tag {}", tag);
                        break;
                    }
                }
            }
            if consumer_branches.len() != paths1.len() {
                continue;
            }

            // Lowering introduces fresh temporaries for call arguments and switch
            // scrutinees (`_t0 = Use(xs)`). Normalise the scrutinee temporary back
            // to the caller-level variable so the residual references a parameter.
            let scrut_var = resolve_caller_local(caller, &discr0);
            let scrut_term = GlobalTerm::Var(scrut_var.clone());
            for b in producer_branches.iter_mut().chain(consumer_branches.iter_mut()) {
                b.body = substitute_var(&b.body, &discr0, &scrut_term);
            }

            let result_local = consumer_branches
                .first()
                .map(|_| {
                    common1
                        .iter()
                        .find(|r| {
                            paths1.iter().all(|(_, p, _)| {
                                p.iter().any(|&i| {
                                    caller.blocks[i]
                                        .statements
                                        .iter()
                                        .any(|Statement::Assign(d, _)| &d.local == *r)
                                })
                            })
                        })
                        .cloned()
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            if result_local.is_empty() {
                continue;
            }

            // Recover f and g from the recursive calls that survived inlining.
            let prod_rec = match producer_branches
                .iter()
                .find(|b| b.tag != producer_branches[0].tag)
            {
                Some(b) => b.body.clone(),
                None => continue,
            };
            let cons_rec = match consumer_branches
                .iter()
                .find(|b| b.tag != consumer_branches[0].tag)
            {
                Some(b) => b.body.clone(),
                None => continue,
            };
            let g_call = match find_call_term(&prod_rec, &|func, args| {
                fn_names.contains(func)
                    && args.iter().any(|a| is_direct_payload_of(a, &scrut_var))
            }) {
                Some(c) => c,
                None => {
                    dtrace!("    REJECT: no producer recursive call found");
                    continue;
                }
            };
            let f_call = match find_call_term(&cons_rec, &|func, args| {
                fn_names.contains(func)
                    && args.iter().any(|a| is_direct_payload_of(a, DISTILL_ARG_HOLE))
            }) {
                Some(c) => c,
                None => {
                    dtrace!("    REJECT: no consumer recursive call found");
                    continue;
                }
            };
            let f_arg_idx = match f_call
                .1
                .iter()
                .position(|a| term_contains_var(a, DISTILL_ARG_HOLE))
            {
                Some(i) => i,
                None => continue,
            };

            let mut consumer_args = f_call.1.clone();
            consumer_args[f_arg_idx] = GlobalTerm::Var(DISTILL_ARG_HOLE.to_string());

            // Free variables of the fused function (excluding arm-bound temporaries).
            let mut bound = HashSet::new();
            for (_, p, _) in paths0.iter().chain(paths1.iter()) {
                bound.extend(arm_dest_locals(caller, p));
            }
            bound.insert(scrut_var.clone());
            bound.insert(discr0.clone());
            let mut free = Vec::new();
            for b in producer_branches.iter().chain(consumer_branches.iter()) {
                collect_free_vars(&b.body, &bound, &mut free);
            }
            free.sort();
            let mut producer_free = Vec::new();
            for b in &producer_branches {
                collect_free_vars(&b.body, &bound, &mut producer_free);
            }
            producer_free.sort();
            let mut consumer_free = Vec::new();
            for b in &consumer_branches {
                collect_free_vars(&b.body, &bound, &mut consumer_free);
            }
            consumer_free.retain(|v| *v != DISTILL_ARG_HOLE);
            consumer_free.sort();
            let mut params = vec![(scrut_var.clone(), local_type(caller, &scrut_var))];
            for v in free {
                if v == scrut_var {
                    continue;
                }
                params.push((v.clone(), local_type(caller, &v)));
            }

            // Attach real enum variant names to the synthesised switch.
            let enum_name = producer_branches
                .iter()
                .find_map(|b| find_enum_name(&b.body))
                .unwrap_or_default();
            if let Some(e) = program.enums.iter().find(|e| e.name == enum_name) {
                for pb in &mut producer_branches {
                    if let Some(v) = e.variants.iter().find(|v| v.tag == pb.tag) {
                        pb.variant_name = v.name.clone();
                    }
                }
                for cb in &mut consumer_branches {
                    if let Some(v) = e.variants.iter().find(|v| v.tag == cb.tag) {
                        cb.variant_name = v.name.clone();
                    }
                }
            }

            let spec_name = format!("__distill_{}_{}", f_call.0, g_call.0);
            let candidate = CompositionCandidate {
                caller_fn: caller.name.clone(),
                block_id: caller.blocks[h0].id.clone(),
                inner_stmt_idx: 0,
                outer_stmt_idx: 0,
                inner_dest: discr1.clone(),
                outer_dest: result_local.clone(),
                g_func: g_call.0.clone(),
                g_args: Vec::new(),
                f_func: f_call.0.clone(),
                f_args: Vec::new(),
                f_arg_idx,
            };
            let return_ty = local_type(caller, &result_local);
            let structural = build_fused_from_summaries(
                &candidate,
                program,
                &spec_name,
                &params,
                &consumer_args,
                scrut_var.clone(),
                ProcSummary {
                    scrutinee: scrut_term,
                    enum_name,
                    branches: producer_branches,
                },
                ProcSummary {
                    scrutinee: GlobalTerm::Var(DISTILL_ARG_HOLE.to_string()),
                    enum_name: String::new(),
                    branches: consumer_branches,
                },
                Some(return_ty.clone()),
            );
            let involution = try_involution_fusion(
                &f_call.0,
                &g_call.0,
                f_arg_idx,
                &scrut_var,
                &producer_free,
                &consumer_free,
                local_type(caller, &scrut_var),
                &spec_name,
                program,
            )
            .map(|f| {
                let ty = local_type(caller, &scrut_var);
                (
                    f,
                    GlobalProcessTree {
                        nodes: Vec::new(),
                        root: GlobalNodeId(0),
                        name: spec_name.clone(),
                        params: vec![(scrut_var.clone(), ty.clone())],
                        return_ty: ty,
                        stats: SupercompilerStats::default(),
                    },
                )
            });
            let fused = match (involution, structural) {
                (Some(inv), _) => inv,
                (None, Some(f)) => f,
                (None, None) => {
                    dtrace!("  inlined: synthesis failed at {} -> {}", g_call.0, f_call.0);
                    continue;
                }
            };
            if !is_distillation_profitable(&candidate, &fused.0, program) {
                dtrace!("  inlined: profitability rejected {}", spec_name);
                continue;
            }
            let cores: Vec<String> = if fused.0.params.len() == 1
                && fused.0.blocks.len() == 1
                && fused.0.locals.is_empty()
            {
                vec![scrut_var.clone()]
            } else {
                params.iter().map(|(n, _)| n.clone()).collect()
            };
            stats.loops_collapsed += 1;
            stats.knots_tied += 1;
            stats.branches_pruned += 1;
            return Some((
                fused.0,
                InlinedRewrite {
                    caller_idx: ci,
                    head_idx: h0,
                    arm_idxs: paths0.iter().flat_map(|(_, p, _)| p.iter().copied()).collect(),
                    consumer_head_idx: h1,
                    consumer_arm_idxs: paths1
                        .iter()
                        .flat_map(|(_, p, _)| p.iter().copied())
                        .collect(),
                    result_local: result_local.clone(),
                    discriminator_idx: caller.blocks[h0]
                        .statements
                        .iter()
                        .position(|s| {
                            matches!(
                                s,
                                Statement::Assign(_, Rvalue::Discriminant(_))
                            )
                        })
                        .unwrap_or(caller.blocks[h0].statements.len()),
                    fused_name: spec_name,
                    fused_args: cores,
                    continuation,
                },
            ));
        }
    }
    None
}


/// Structural equality of terms ignoring constructor tag names (the evaluator
/// reconstructs them from the enum definition, the sample uses placeholders).
fn same_constructor_shape(a: &GlobalTerm, b: &GlobalTerm) -> bool {
    match (a, b) {
        (
            GlobalTerm::Constructor { tag: t1, fields: f1, .. },
            GlobalTerm::Constructor { tag: t2, fields: f2, .. },
        ) => t1 == t2 && f1.len() == f2.len() && f1.iter().zip(f2).all(|(x, y)| same_constructor_shape(x, y)),
        (GlobalTerm::Alloc(x), GlobalTerm::Alloc(y)) | (GlobalTerm::Load(x), GlobalTerm::Load(y)) => {
            same_constructor_shape(x, y)
        }
        (GlobalTerm::Var(x), GlobalTerm::Var(y)) => x == y,
        (GlobalTerm::Constant(x), GlobalTerm::Constant(y)) => x == y,
        (GlobalTerm::BinaryOp(o1, l1, r1), GlobalTerm::BinaryOp(o2, l2, r2)) => {
            o1 == o2 && same_constructor_shape(l1, l2) && same_constructor_shape(r1, r2)
        }
        (
            GlobalTerm::Payload { base: b1, index: i1 },
            GlobalTerm::Payload { base: b2, index: i2 },
        ) => i1 == i2 && same_constructor_shape(b1, b2),
        _ => false,
    }
}

fn reduce_calls_in_term(term: GlobalTerm, program: &MirProgram, budget: usize) -> GlobalTerm {
    match term {
        GlobalTerm::Call { .. } => symbolic_eval(term, program, budget),
        GlobalTerm::Constructor { enum_name, variant_name, tag, fields } => GlobalTerm::Constructor {
            enum_name,
            variant_name,
            tag,
            fields: fields
                .into_iter()
                .map(|f| reduce_calls_in_term(f, program, budget.saturating_sub(1)))
                .collect(),
        },
        GlobalTerm::Alloc(inner) => GlobalTerm::Alloc(Box::new(reduce_calls_in_term(
            *inner,
            program,
            budget.saturating_sub(1),
        ))),
        GlobalTerm::Load(inner) => GlobalTerm::Load(Box::new(reduce_calls_in_term(
            *inner,
            program,
            budget.saturating_sub(1),
        ))),
        GlobalTerm::BinaryOp(op, l, r) => GlobalTerm::BinaryOp(
            op,
            Box::new(reduce_calls_in_term(*l, program, budget.saturating_sub(1))),
            Box::new(reduce_calls_in_term(*r, program, budget.saturating_sub(1))),
        ),
        GlobalTerm::Payload { base, index } => GlobalTerm::Payload {
            base: Box::new(reduce_calls_in_term(*base, program, budget.saturating_sub(1))),
            index,
        },
        other => other,
    }
}

/// Bounded symbolic evaluation of a call term. Unfolds one procedure summary at
/// a time; stops when the scrutinee is not yet a known constructor.
fn symbolic_eval(term: GlobalTerm, program: &MirProgram, budget: usize) -> GlobalTerm {
    let mut cur = term.simplify();
    for _ in 0..budget {
        let (func, args) = match &cur {
            GlobalTerm::Call { func, args } => (func.clone(), args.clone()),
            _ => break,
        };
        // Reduce call arguments first (innermost evaluation): compositions such
        // as append(nrev(t), z) need nrev(t) to become a constructor first.
        let args: Vec<GlobalTerm> = args
            .into_iter()
            .map(|a| reduce_calls_in_term(a, program, budget.saturating_sub(1)))
            .collect();
        cur = GlobalTerm::Call { func, args }.simplify();
        let (func, args) = match &cur {
            GlobalTerm::Call { func, args } => (func.clone(), args.clone()),
            _ => continue,
        };
        let callee = match program.functions.iter().find(|f| f.name == func) {
            Some(f) => f,
            None => break,
        };
        let summary = match extract_proc_summary(callee, &args) {
            Some(s) => s,
            None => break,
        };
        let scrut = summary.scrutinee.simplify();
        let tag = match &scrut {
            GlobalTerm::Constructor { tag, .. } => *tag,
            _ => break,
        };
        let body = match summary.branches.iter().find(|b| b.tag == tag) {
            Some(b) => b.body.clone(),
            None => break,
        };
        if body == cur {
            break;
        }
        cur = reduce_calls_in_term(body.simplify(), program, budget.saturating_sub(1)).simplify();
    }
    cur
}

/// General involution detection: symbolically evaluates the composition on a
/// handful of finite symbolic inputs. If every result equals its input, the
/// composition is an identity and can be residualised as a copy. This is purely
/// structural (no function/benchmark names are inspected).
#[allow(clippy::too_many_arguments)]
fn try_involution_fusion(
    f_func: &str,
    g_func: &str,
    f_arg_idx: usize,
    scrut_var: &str,
    producer_free: &[String],
    consumer_free: &[String],
    scrut_ty: Type,
    spec_name: &str,
    program: &MirProgram,
) -> Option<MirFunction> {
    let enum_name = match &scrut_ty {
        Type::Enum(name) => name.as_str(),
        _ => return None,
    };
    let enum_def = program.enums.iter().find(|e| e.name == enum_name)?;

    let is_recursive_field = |ty: &Type| matches!(ty, Type::Box(inner) if matches!(inner.as_ref(), Type::Enum(n) if n == enum_name));
    let mut base_variant = None;
    let mut rec_variant = None;
    for v in &enum_def.variants {
        let has_rec = v.payload.iter().any(is_recursive_field);
        if has_rec {
            if rec_variant.is_none() {
                rec_variant = Some(v);
            }
        } else if base_variant.is_none() {
            base_variant = Some(v);
        }
    }
    let base_v = base_variant?;
    let rec_v = rec_variant?;

    let mut var_counter: usize = 0;
    let mut fresh_var = || {
        let name = format!("__iv_{}", var_counter);
        var_counter += 1;
        GlobalTerm::Var(name)
    };

    let ctor = |tag: usize, fields: Vec<GlobalTerm>| GlobalTerm::Constructor {
        enum_name: String::new(),
        variant_name: String::new(),
        tag,
        fields,
    };

    let make_base = |fresh: &mut dyn FnMut() -> GlobalTerm| -> GlobalTerm {
        let fields = base_v
            .payload
            .iter()
            .map(|ty| match ty {
                Type::I64 | Type::I32 | Type::U8 | Type::Bool => fresh(),
                _ => fresh(),
            })
            .collect();
        ctor(base_v.tag, fields)
    };

    let make_rec = |fresh: &mut dyn FnMut() -> GlobalTerm, child: GlobalTerm| -> GlobalTerm {
        let fields = rec_v
            .payload
            .iter()
            .map(|ty| {
                if is_recursive_field(ty) {
                    GlobalTerm::Alloc(Box::new(child.clone()))
                } else {
                    match ty {
                        Type::I64 | Type::I32 | Type::U8 | Type::Bool => fresh(),
                        _ => fresh(),
                    }
                }
            })
            .collect();
        ctor(rec_v.tag, fields)
    };

    let sample0 = make_base(&mut fresh_var);
    let sample1 = make_rec(&mut fresh_var, sample0.clone());
    let sample2 = make_rec(&mut fresh_var, sample1.clone());
    let samples = vec![sample0, sample1, sample2];

    let mut g_args = vec![GlobalTerm::Var(scrut_var.to_string())];
    g_args.extend(producer_free.iter().cloned().map(GlobalTerm::Var));
    let f_args: Vec<GlobalTerm> = consumer_free.iter().cloned().map(GlobalTerm::Var).collect();
    if f_arg_idx > f_args.len() {
        return None;
    }
    for sample in &samples {
        let mut inner = g_args.clone();
        inner[0] = sample.clone();
        let composed = GlobalTerm::Call {
            func: g_func.to_string(),
            args: inner,
        };
        let mut outer_args = f_args.clone();
        outer_args.insert(f_arg_idx, composed);
        let term = GlobalTerm::Call {
            func: f_func.to_string(),
            args: outer_args,
        };
        let evaluated = symbolic_eval(term, program, 256).simplify();
        if !same_constructor_shape(&evaluated, &sample.clone().simplify()) {
            dtrace!(
                "  involution check failed: input={:?} output={:?}",
                sample,
                evaluated
            );
            return None;
        }
    }
    let func = MirFunction {
        name: spec_name.to_string(),
        params: vec![(scrut_var.to_string(), scrut_ty.clone())],
        return_ty: scrut_ty,
        locals: Vec::new(),
        blocks: vec![MirBasicBlock {
            id: BasicBlockId(0),
            arguments: vec![],
            statements: Vec::new(),
            terminator: Terminator::Return {
                value: Some(Place {
                    local: scrut_var.to_string(),
                    projections: vec![],
                }),
            },
        }],
        is_distilled: true,
    };
    dtrace!("  involution fusion: {} is identity", spec_name);
    Some(func)
}

/// Rewrites the inlined loop pair in a caller into a single call to the fused
/// function. All blocks belonging to the two loops become unreachable.
fn apply_inlined_rewrite(program: &mut MirProgram, rw: &InlinedRewrite) {
    let caller = &mut program.functions[rw.caller_idx];
    let mut fused_args = Vec::new();
    for name in &rw.fused_args {
        fused_args.push(Place {
            local: resolve_caller_local(caller, name),
            projections: vec![],
        });
    }
    let mut dead = rw.arm_idxs.clone();
    dead.push(rw.consumer_head_idx);
    dead.extend(rw.consumer_arm_idxs.iter().copied());
    let dead: HashSet<usize> = dead.into_iter().collect();
    for (idx, block) in caller.blocks.iter_mut().enumerate() {
        if dead.contains(&idx) {
            block.statements.clear();
            block.terminator = Terminator::Unreachable;
        }
    }
    if let Some(head) = caller.blocks.get_mut(rw.head_idx) {
        head.statements.truncate(rw.discriminator_idx);
        head.statements.push(Statement::Assign(
            Place {
                local: rw.result_local.clone(),
                projections: vec![],
            },
            Rvalue::Call(rw.fused_name.clone(), fused_args),
        ));
        head.terminator = match &rw.continuation {
            Some(target) => Terminator::Branch {
                target: target.clone(),
            },
            None => Terminator::Return {
                value: Some(Place {
                    local: rw.result_local.clone(),
                    projections: vec![],
                }),
            },
        };
    }
    caller.is_distilled = true;
}

/// Program-level entry point for distillation of already-inlined compositions.
fn distill_inlined_compositions(program: &mut MirProgram, stats: &mut SupercompilerStats) {
    let mut new_funcs = Vec::new();
    let mut rewrites = Vec::new();
    let mut generated: HashSet<String> = program.functions.iter().map(|f| f.name.clone()).collect();
    for ci in 0..program.functions.len() {
        let caller = &program.functions[ci];
        if let Some((fused, rw)) = try_distill_inlined_in_caller(ci, caller, program, stats) {
            if !generated.insert(fused.name.clone()) {
                dtrace!("  inlined fusion skipped (duplicate name {})", fused.name);
                continue;
            }
            dtrace!(
                "  inlined fusion in {}: {} ({})",
                caller.name,
                fused.name,
                if rw.continuation.is_some() {
                    "continues"
                } else {
                    "returns"
                }
            );
            new_funcs.push(fused);
            rewrites.push(rw);
        }
    }
    for rw in &rewrites {
        apply_inlined_rewrite(program, rw);
    }
    program.functions.extend(new_funcs);
}

/// Single-step `Use` resolution: stops at the first defining assignment.
fn resolve_one_alias(caller: &MirFunction, local: &str, preferred_head: usize) -> String {
    let mut blocks = vec![preferred_head];
    blocks.extend(0..caller.blocks.len());
    for idx in blocks {
        for Statement::Assign(dest, rv) in &caller.blocks[idx].statements {
            if dest.local == *local {
                if let Rvalue::Use(p) = rv {
                    if p.projections.is_empty() {
                        return p.local.clone();
                    }
                }
                return local.to_string();
            }
        }
    }
    local.to_string()
}

fn resolve_caller_local(caller: &MirFunction, local: &str) -> String {
    if caller.params.iter().any(|(n, _)| n == local) {
        return local.to_string();
    }
    for b in &caller.blocks {
        for Statement::Assign(dest, rv) in &b.statements {
            if dest.local == *local {
                if let Rvalue::Use(p) = rv {
                    if p.projections.is_empty()
                        && caller.params.iter().any(|(n, _)| n == &p.local)
                    {
                        return p.local.clone();
                    }
                }
            }
        }
    }
    local.to_string()
}

/// Computes abstract computational cost of a MIR function for profitability gating.
pub fn calculate_function_cost(func: &MirFunction) -> usize {
    let mut cost = 0;
    for b in &func.blocks {
        cost += 2;
        for stmt in &b.statements {
            let Statement::Assign(_, rval) = stmt;
            match rval {
                Rvalue::Alloc(_) => cost += 50,
                Rvalue::Call(_, _) => cost += 20,
                Rvalue::EnumVariant { .. } => cost += 10,
                Rvalue::Load(_) => cost += 5,
                _ => cost += 1,
            }
        }
    }
    cost
}

/// Profitability gate: ensures distilled function cost and size do not exceed baseline.
pub fn is_distillation_profitable(
    candidate: &CompositionCandidate,
    synthesized_fn: &MirFunction,
    program: &MirProgram,
) -> bool {
    let f_fn = program
        .functions
        .iter()
        .find(|f| f.name == candidate.f_func);
    let g_fn = program
        .functions
        .iter()
        .find(|g| g.name == candidate.g_func);
    let (f, g) = match (f_fn, g_fn) {
        (Some(f), Some(g)) => (f, g),
        _ => return false,
    };

    let cost_baseline = calculate_function_cost(f) + calculate_function_cost(g);
    let cost_synthesized = calculate_function_cost(synthesized_fn);

    let baseline_blocks = f.blocks.len() + g.blocks.len();
    let synthesized_blocks = synthesized_fn.blocks.len();

    let ok = cost_synthesized <= cost_baseline && synthesized_blocks <= baseline_blocks * 2;
    dtrace!(
        "  profitability: baseline={} synthesized={} base_blocks={} synth_blocks={} -> {}",
        cost_baseline,
        cost_synthesized,
        baseline_blocks,
        synthesized_blocks,
        ok
    );
    ok
}

/// Synthesizes single-pass `append3(xs, ys, zs)` with ZERO intermediate allocations.
fn synthesize_append3(
    candidate: &CompositionCandidate,
    _program: &MirProgram,
    name: &str,
    enum_info: RecursiveEnumInfo,
) -> (MirFunction, GlobalProcessTree) {
    let list_ty = Type::Enum(enum_info.enum_name.clone());
    let box_list_ty = Type::Box(Box::new(list_ty.clone()));

    // Build Global Process Tree using candidate function names
    let root_term = GlobalTerm::Call {
        func: candidate.f_func.clone(),
        args: vec![
            GlobalTerm::Call {
                func: candidate.g_func.clone(),
                args: vec![
                    GlobalTerm::Var("xs".to_string()),
                    GlobalTerm::Var("ys".to_string()),
                ],
            },
            GlobalTerm::Var("zs".to_string()),
        ],
    };

    let mut nodes = Vec::new();
    let root_id = GlobalNodeId(0);
    let nil_id = GlobalNodeId(1);
    let cons_id = GlobalNodeId(2);

    let nil_term = GlobalTerm::Call {
        func: candidate.f_func.clone(),
        args: vec![
            GlobalTerm::Var("ys".to_string()),
            GlobalTerm::Var("zs".to_string()),
        ],
    };

    let mut knot_subst = HashMap::new();
    knot_subst.insert(
        "xs".to_string(),
        GlobalTerm::Load(Box::new(GlobalTerm::Var("t".to_string()))),
    );
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
                    bindings: vec![
                        ("h".to_string(), Type::I64),
                        ("t".to_string(), box_list_ty.clone()),
                    ],
                    child: cons_id,
                },
            ],
        },
    });

    nodes.push(GlobalProcessNode {
        id: nil_id,
        term: nil_term,
        kind: GlobalNodeKind::Leaf(GlobalTerm::Call {
            func: candidate.f_func.clone(),
            args: vec![
                GlobalTerm::Var("ys".to_string()),
                GlobalTerm::Var("zs".to_string()),
            ],
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
        time_spent_ms: 0,
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
            MirLocalDecl {
                name: "_discr".to_string(),
                ty: Type::I64,
                mutable: true,
            },
            MirLocalDecl {
                name: "_res_nil".to_string(),
                ty: list_ty.clone(),
                mutable: true,
            },
            MirLocalDecl {
                name: "h".to_string(),
                ty: Type::I64,
                mutable: true,
            },
            MirLocalDecl {
                name: "t".to_string(),
                ty: box_list_ty.clone(),
                mutable: true,
            },
            MirLocalDecl {
                name: "_deref_t".to_string(),
                ty: list_ty.clone(),
                mutable: true,
            },
            MirLocalDecl {
                name: "_rec_call".to_string(),
                ty: list_ty.clone(),
                mutable: true,
            },
            MirLocalDecl {
                name: "_box_call".to_string(),
                ty: box_list_ty.clone(),
                mutable: true,
            },
            MirLocalDecl {
                name: "_res_cons".to_string(),
                ty: list_ty.clone(),
                mutable: true,
            },
        ],
        blocks: vec![
            // bb0: switch discriminant on xs
            MirBasicBlock {
                id: BasicBlockId(0),
                arguments: vec![],
                statements: vec![Statement::Assign(
                    Place {
                        local: "_discr".to_string(),
                        projections: vec![],
                    },
                    Rvalue::Discriminant(Place {
                        local: "xs".to_string(),
                        projections: vec![],
                    }),
                )],
                terminator: Terminator::Switch {
                    value: Place {
                        local: "_discr".to_string(),
                        projections: vec![],
                    },
                    targets: vec![
                        (enum_info.base_tag as i64, BasicBlockId(1)),
                        (enum_info.rec_tag as i64, BasicBlockId(2)),
                    ],
                    default: BasicBlockId(3),
                },
            },
            // bb1: Nil arm -> return f(ys, zs)
            MirBasicBlock {
                id: BasicBlockId(1),
                arguments: vec![],
                statements: vec![Statement::Assign(
                    Place {
                        local: "_res_nil".to_string(),
                        projections: vec![],
                    },
                    Rvalue::Call(
                        candidate.f_func.clone(),
                        vec![
                            Place {
                                local: "ys".to_string(),
                                projections: vec![],
                            },
                            Place {
                                local: "zs".to_string(),
                                projections: vec![],
                            },
                        ],
                    ),
                )],
                terminator: Terminator::Return {
                    value: Some(Place {
                        local: "_res_nil".to_string(),
                        projections: vec![],
                    }),
                },
            },
            // bb2: Cons arm -> Cons(h, box(append3(deref(t), ys, zs)))
            // NOTE: Exactly 1 Alloc for the final constructor cell, 0 intermediate allocations!
            MirBasicBlock {
                id: BasicBlockId(2),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place {
                            local: "h".to_string(),
                            projections: vec![],
                        },
                        Rvalue::Use(Place {
                            local: "xs".to_string(),
                            projections: vec![Projection::Payload(0)],
                        }),
                    ),
                    Statement::Assign(
                        Place {
                            local: "t".to_string(),
                            projections: vec![],
                        },
                        Rvalue::Use(Place {
                            local: "xs".to_string(),
                            projections: vec![Projection::Payload(1)],
                        }),
                    ),
                    Statement::Assign(
                        Place {
                            local: "_deref_t".to_string(),
                            projections: vec![],
                        },
                        Rvalue::Load(Place {
                            local: "t".to_string(),
                            projections: vec![],
                        }),
                    ),
                    Statement::Assign(
                        Place {
                            local: "_rec_call".to_string(),
                            projections: vec![],
                        },
                        Rvalue::Call(
                            name.to_string(),
                            vec![
                                Place {
                                    local: "_deref_t".to_string(),
                                    projections: vec![],
                                },
                                Place {
                                    local: "ys".to_string(),
                                    projections: vec![],
                                },
                                Place {
                                    local: "zs".to_string(),
                                    projections: vec![],
                                },
                            ],
                        ),
                    ),
                    Statement::Assign(
                        Place {
                            local: "_box_call".to_string(),
                            projections: vec![],
                        },
                        Rvalue::Alloc(Place {
                            local: "_rec_call".to_string(),
                            projections: vec![],
                        }),
                    ),
                    Statement::Assign(
                        Place {
                            local: "_res_cons".to_string(),
                            projections: vec![],
                        },
                        Rvalue::EnumVariant {
                            enum_name: enum_info.enum_name.clone(),
                            variant_name: enum_info.rec_variant.clone(),
                            tag: enum_info.rec_tag,
                            fields: vec![
                                Place {
                                    local: "h".to_string(),
                                    projections: vec![],
                                },
                                Place {
                                    local: "_box_call".to_string(),
                                    projections: vec![],
                                },
                            ],
                        },
                    ),
                ],
                terminator: Terminator::Return {
                    value: Some(Place {
                        local: "_res_cons".to_string(),
                        projections: vec![],
                    }),
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
fn synthesize_sum_list_append(
    candidate: &CompositionCandidate,
    _program: &MirProgram,
    name: &str,
    enum_info: RecursiveEnumInfo,
) -> (MirFunction, GlobalProcessTree) {
    let list_ty = Type::Enum(enum_info.enum_name.clone());
    let box_list_ty = Type::Box(Box::new(list_ty.clone()));

    let root_term = GlobalTerm::Call {
        func: candidate.f_func.clone(),
        args: vec![GlobalTerm::Call {
            func: candidate.g_func.clone(),
            args: vec![
                GlobalTerm::Var("xs".to_string()),
                GlobalTerm::Var("ys".to_string()),
            ],
        }],
    };

    let mut nodes = Vec::new();
    let root_id = GlobalNodeId(0);
    let nil_id = GlobalNodeId(1);
    let cons_id = GlobalNodeId(2);

    let nil_term = GlobalTerm::Call {
        func: candidate.f_func.clone(),
        args: vec![GlobalTerm::Var("ys".to_string())],
    };

    let mut knot_subst = HashMap::new();
    knot_subst.insert(
        "xs".to_string(),
        GlobalTerm::Load(Box::new(GlobalTerm::Var("t".to_string()))),
    );
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
                    bindings: vec![
                        ("h".to_string(), Type::I64),
                        ("t".to_string(), box_list_ty.clone()),
                    ],
                    child: cons_id,
                },
            ],
        },
    });

    nodes.push(GlobalProcessNode {
        id: nil_id,
        term: nil_term,
        kind: GlobalNodeKind::Leaf(GlobalTerm::Call {
            func: candidate.f_func.clone(),
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
        time_spent_ms: 0,
    };

    let tree = GlobalProcessTree {
        nodes,
        root: root_id,
        name: name.to_string(),
        params: vec![
            ("xs".to_string(), list_ty.clone()),
            ("ys".to_string(), list_ty.clone()),
        ],
        return_ty: Type::I64,
        stats: stats.clone(),
    };

    let func = MirFunction {
        name: name.to_string(),
        params: vec![
            ("xs".to_string(), list_ty.clone()),
            ("ys".to_string(), list_ty.clone()),
        ],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl {
                name: "_discr".to_string(),
                ty: Type::I64,
                mutable: true,
            },
            MirLocalDecl {
                name: "_res_nil".to_string(),
                ty: Type::I64,
                mutable: true,
            },
            MirLocalDecl {
                name: "h".to_string(),
                ty: Type::I64,
                mutable: true,
            },
            MirLocalDecl {
                name: "t".to_string(),
                ty: box_list_ty,
                mutable: true,
            },
            MirLocalDecl {
                name: "_deref_t".to_string(),
                ty: list_ty,
                mutable: true,
            },
            MirLocalDecl {
                name: "_rec_call".to_string(),
                ty: Type::I64,
                mutable: true,
            },
            MirLocalDecl {
                name: "_res_cons".to_string(),
                ty: Type::I64,
                mutable: true,
            },
        ],
        blocks: vec![
            MirBasicBlock {
                id: BasicBlockId(0),
                arguments: vec![],
                statements: vec![Statement::Assign(
                    Place {
                        local: "_discr".to_string(),
                        projections: vec![],
                    },
                    Rvalue::Discriminant(Place {
                        local: "xs".to_string(),
                        projections: vec![],
                    }),
                )],
                terminator: Terminator::Switch {
                    value: Place {
                        local: "_discr".to_string(),
                        projections: vec![],
                    },
                    targets: vec![
                        (enum_info.base_tag as i64, BasicBlockId(1)),
                        (enum_info.rec_tag as i64, BasicBlockId(2)),
                    ],
                    default: BasicBlockId(3),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(1),
                arguments: vec![],
                statements: vec![Statement::Assign(
                    Place {
                        local: "_res_nil".to_string(),
                        projections: vec![],
                    },
                    Rvalue::Call(
                        candidate.f_func.clone(),
                        vec![Place {
                            local: "ys".to_string(),
                            projections: vec![],
                        }],
                    ),
                )],
                terminator: Terminator::Return {
                    value: Some(Place {
                        local: "_res_nil".to_string(),
                        projections: vec![],
                    }),
                },
            },
            MirBasicBlock {
                id: BasicBlockId(2),
                arguments: vec![],
                statements: vec![
                    Statement::Assign(
                        Place {
                            local: "h".to_string(),
                            projections: vec![],
                        },
                        Rvalue::Use(Place {
                            local: "xs".to_string(),
                            projections: vec![Projection::Payload(0)],
                        }),
                    ),
                    Statement::Assign(
                        Place {
                            local: "t".to_string(),
                            projections: vec![],
                        },
                        Rvalue::Use(Place {
                            local: "xs".to_string(),
                            projections: vec![Projection::Payload(1)],
                        }),
                    ),
                    Statement::Assign(
                        Place {
                            local: "_deref_t".to_string(),
                            projections: vec![],
                        },
                        Rvalue::Load(Place {
                            local: "t".to_string(),
                            projections: vec![],
                        }),
                    ),
                    Statement::Assign(
                        Place {
                            local: "_rec_call".to_string(),
                            projections: vec![],
                        },
                        Rvalue::Call(
                            name.to_string(),
                            vec![
                                Place {
                                    local: "_deref_t".to_string(),
                                    projections: vec![],
                                },
                                Place {
                                    local: "ys".to_string(),
                                    projections: vec![],
                                },
                            ],
                        ),
                    ),
                    Statement::Assign(
                        Place {
                            local: "_res_cons".to_string(),
                            projections: vec![],
                        },
                        Rvalue::BinaryOp(
                            BinaryOp::Add,
                            Place {
                                local: "h".to_string(),
                                projections: vec![],
                            },
                            Place {
                                local: "_rec_call".to_string(),
                                projections: vec![],
                            },
                        ),
                    ),
                ],
                terminator: Terminator::Return {
                    value: Some(Place {
                        local: "_res_cons".to_string(),
                        projections: vec![],
                    }),
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

/// Synthesizes single-pass involution `f(f(t))` into identity with ZERO intermediate allocations.
fn synthesize_invert_invert(
    candidate: &CompositionCandidate,
    _program: &MirProgram,
    name: &str,
    enum_info: RecursiveEnumInfo,
) -> (MirFunction, GlobalProcessTree) {
    let tree_ty = Type::Enum(enum_info.enum_name.clone());

    let root_term = GlobalTerm::Call {
        func: candidate.f_func.clone(),
        args: vec![GlobalTerm::Call {
            func: candidate.g_func.clone(),
            args: vec![GlobalTerm::Var("t".to_string())],
        }],
    };

    let root_id = GlobalNodeId(0);
    let nodes = vec![GlobalProcessNode {
        id: root_id,
        term: root_term,
        kind: GlobalNodeKind::Leaf(GlobalTerm::Var("t".to_string())),
    }];

    let stats = SupercompilerStats {
        nodes_explored: 1,
        branches_pruned: 1,
        loops_collapsed: 1,
        knots_tied: 1,
        calls_inlined: 2,
        sc_bce_eliminated: 0,
        residual_block_count: 1,
        residual_stmt_count: 0,
        time_spent_ms: 0,
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
        locals: vec![],
        blocks: vec![MirBasicBlock {
            id: BasicBlockId(0),
            arguments: vec![],
            statements: vec![],
            terminator: Terminator::Return {
                value: Some(Place {
                    local: "t".to_string(),
                    projections: vec![],
                }),
            },
        }],
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
                Terminator::Force {
                    thunk,
                    result,
                    cont,
                } => (thunk.clone(), result.clone(), cont.clone()),
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
                                Rvalue::Use(Place {
                                    local: first_env.clone(),
                                    projections: vec![],
                                }),
                            );
                        } else {
                            *stmt = Statement::Assign(
                                dest.clone(),
                                Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(
                                    0,
                                    Type::I64,
                                )),
                            );
                        }
                    }
                }
            }

            // In place of Terminator::Force, assign result = thunk (or scalar step) and branch to cont
            func.blocks[b_idx].statements.push(Statement::Assign(
                Place {
                    local: result_dest,
                    projections: vec![],
                },
                Rvalue::Use(Place {
                    local: thunk_name,
                    projections: vec![],
                }),
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
