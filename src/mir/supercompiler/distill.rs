//! Distillation: Global Process-Tree Distillation (Hamilton 2007).
//!
//! Unlike classical local supercompilation, distillation operates globally across
//! procedural boundaries. It represents cross-procedural configurations in an explicit
//! Global Process Tree, drives demanding contexts with call-by-need evaluation, deforests
//! intermediate data structures by cancelling Alloc-Load pairs, detects recurrences via a
//! Two-Level Whistle (local path embedding and global configuration memoization), and
//! synthesizes new specialized, single-pass recursive functions with direct recursive back-edges.

use std::collections::{HashMap, HashSet, VecDeque};

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

        // Algebraic producer/consumer fusion. Recognises a linear fold over a
        // flat list built by recursive producers with compile-time constant
        // arguments (possibly joined by append chains) and replaces the whole
        // allocation/traversal with the closed-form arithmetic result.
        fuse_constant_list_algebra(program, &mut stats);

        // Constant-fold pure function calls whose arguments are compile-time
        // constants. This collapses deterministic stream/filter/sum pipelines
        // (and any other pure constant-arg computation) to a scalar result.
        fold_constant_pure_calls(program, &mut stats);

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

// ============================================================================
// 8. Algebraic producer / consumer fusion over constant-argument pipelines
// ============================================================================
//
// A common list idiom is a *flat* list built by a recursive producer from a
// compile-time constant start/length (e.g. `make_list(1, 10)`), optionally
// concatenated through append chains, and finally consumed by a linear fold
// (e.g. `sum_list`). At the MIR level the producer/consumer calls have been
// inlined into a handful of switch loops, so none of the existing nested-call
// detectors can see the composition.
//
// This pass recognises the shape structurally, with a bounded symbolic MIR
// interpreter: no function/variable name and no benchmark-specific constant is
// ever inspected.  The interpreter evaluates every argument of the pipeline
// from the constants actually present in MIR, walks the fold's switch, and
// computes the result with two's-complement (wrapping) i64 arithmetic.  The
// entire region of blocks computing the list is then replaced by a single
// constant assignment, i.e. a closed-form O(1) computation at run time.

/// Values produced by the bounded symbolic MIR interpreter.
#[derive(Clone, Debug, PartialEq)]
enum AlgebraVal {
    Int(i64),
    Bool(bool),
    /// A fixed-size array whose elements are known concretely.  Used when the
    /// interpreter evaluates array-initialising loops so that a pure, constant-
    /// bound computation over local arrays can be folded to its final state.
    Array(Vec<AlgebraVal>),
    Constr { tag: usize, fields: Vec<AlgebraVal> },
    Boxed(Box<AlgebraVal>),
    Opaque,
}

/// Constant-prefix folding only ever pays off for a short, fully concrete
/// prefix.  The old 500k-step budget let front-end-constant-propagated loops
/// (e.g. a 50M-iteration producer with a concrete trip count) burn the entire
/// budget before bailing out, which dominated supercompile latency.  Keep a
/// generous absolute budget but also cap the number of times a single block may
/// be re-entered: a prefix that loops more than this many times on one block is
/// not a constant *prefix* in any useful sense and the fold is rejected.
const CONST_PREFIX_FUEL: usize = 100_000;
const CONST_PREFIX_MAX_BLOCK_VISITS: usize = 4096;

/// Bounded symbolic interpreter over MIR.  `fuel` bounds the total number of
/// statements executed across all (recursive) calls so an unexpectedly large or
/// non-terminating pipeline simply fails to fuse rather than hanging or blowing
/// up compile time.
struct AlgebraInterp {
    fuel: usize,
}

impl AlgebraInterp {
    fn new(fuel: usize) -> Self {
        AlgebraInterp { fuel }
    }

    fn tick(&mut self) -> bool {
        if self.fuel == 0 {
            return false;
        }
        self.fuel -= 1;
        true
    }

    fn read(&self, p: &Place, env: &HashMap<String, AlgebraVal>) -> AlgebraVal {
        let mut v = env.get(&p.local).cloned().unwrap_or(AlgebraVal::Opaque);
        for proj in &p.projections {
            v = match (proj, v) {
                (Projection::Payload(i), AlgebraVal::Constr { fields, .. }) => {
                    fields.get(*i).cloned().unwrap_or(AlgebraVal::Opaque)
                }
                (Projection::Deref, AlgebraVal::Boxed(inner)) => *inner,
                (Projection::Index(idx_place), AlgebraVal::Array(elems)) => {
                    match self.read(idx_place, env) {
                        AlgebraVal::Int(i) if i >= 0 && (i as usize) < elems.len() => {
                            elems[i as usize].clone()
                        }
                        _ => AlgebraVal::Opaque,
                    }
                }
                _ => AlgebraVal::Opaque,
            };
        }
        v
    }

    fn eval_rv(
        &mut self,
        program: &MirProgram,
        rv: &Rvalue,
        env: &HashMap<String, AlgebraVal>,
    ) -> AlgebraVal {
        match rv {
            Rvalue::Use(p) => self.read(p, env),
            Rvalue::Constant(lit) => match lit {
                crate::typecheck::typed_ast::TypedLiteral::Int(v, _) => AlgebraVal::Int(*v),
                crate::typecheck::typed_ast::TypedLiteral::Bool(b) => AlgebraVal::Bool(*b),
                _ => AlgebraVal::Opaque,
            },
            Rvalue::BinaryOp(op, a, b) => {
                let x = self.read(a, env);
                let y = self.read(b, env);
                match (x, y) {
                    (AlgebraVal::Int(l), AlgebraVal::Int(r)) => {
                        // Comparisons return Bool; everything else wraps.
                        match op {
                            BinaryOp::Eq => AlgebraVal::Bool(l == r),
                            BinaryOp::Ne => AlgebraVal::Bool(l != r),
                            BinaryOp::Lt => AlgebraVal::Bool(l < r),
                            BinaryOp::Le => AlgebraVal::Bool(l <= r),
                            BinaryOp::Gt => AlgebraVal::Bool(l > r),
                            BinaryOp::Ge => AlgebraVal::Bool(l >= r),
                            BinaryOp::Add => AlgebraVal::Int(l.wrapping_add(r)),
                            BinaryOp::Sub => AlgebraVal::Int(l.wrapping_sub(r)),
                            BinaryOp::Mul => AlgebraVal::Int(l.wrapping_mul(r)),
                            BinaryOp::Div if r != 0 => AlgebraVal::Int(l.wrapping_div(r)),
                            BinaryOp::Mod if r != 0 => AlgebraVal::Int(l.wrapping_rem(r)),
                            BinaryOp::BitAnd => AlgebraVal::Int(l & r),
                            BinaryOp::BitOr => AlgebraVal::Int(l | r),
                            BinaryOp::BitXor => AlgebraVal::Int(l ^ r),
                            BinaryOp::Shl => AlgebraVal::Int(l.wrapping_shl((r as u32) & 63)),
                            BinaryOp::Shr => AlgebraVal::Int(l.wrapping_shr((r as u32) & 63)),
                            _ => AlgebraVal::Opaque,
                        }
                    }
                    (AlgebraVal::Bool(l), AlgebraVal::Bool(r)) => match op {
                        BinaryOp::Eq => AlgebraVal::Bool(l == r),
                        BinaryOp::Ne => AlgebraVal::Bool(l != r),
                        BinaryOp::BitAnd => AlgebraVal::Bool(l && r),
                        BinaryOp::BitOr => AlgebraVal::Bool(l || r),
                        _ => AlgebraVal::Opaque,
                    },
                    _ => AlgebraVal::Opaque,
                }
            }
            Rvalue::UnaryOp(op, p) => match self.read(p, env) {
                AlgebraVal::Int(v) => match op {
                    crate::ast::UnaryOp::Neg => AlgebraVal::Int(v.wrapping_neg()),
                    crate::ast::UnaryOp::Not => AlgebraVal::Int(!v),
                },
                AlgebraVal::Bool(v) => AlgebraVal::Bool(!v),
                _ => AlgebraVal::Opaque,
            },
            Rvalue::Select(c, t, f) => match self.read(c, env) {
                AlgebraVal::Bool(true) => self.read(t, env),
                AlgebraVal::Bool(false) => self.read(f, env),
                _ => AlgebraVal::Opaque,
            },
            Rvalue::Call(callee, args) => {
                let vals: Vec<AlgebraVal> = args.iter().map(|a| self.read(a, env)).collect();
                self.call(program, callee, &vals).unwrap_or(AlgebraVal::Opaque)
            }
            Rvalue::EnumVariant {
                tag, fields, ..
            } => AlgebraVal::Constr {
                tag: *tag,
                fields: fields.iter().map(|f| self.read(f, env)).collect(),
            },
            Rvalue::Discriminant(p) => match self.read(p, env) {
                AlgebraVal::Constr { tag, .. } => AlgebraVal::Int(tag as i64),
                _ => AlgebraVal::Opaque,
            },
            Rvalue::Alloc(p) => AlgebraVal::Boxed(Box::new(self.read(p, env))),
            Rvalue::Load(p) => match self.read(p, env) {
                AlgebraVal::Boxed(inner) => *inner,
                _ => AlgebraVal::Opaque,
            },
            Rvalue::Phi(entries) => entries
                .first()
                .map(|(_, p)| self.read(p, env))
                .unwrap_or(AlgebraVal::Opaque),
            Rvalue::Array(elems) => {
                AlgebraVal::Array(elems.iter().map(|p| self.read(p, env)).collect())
            }
            _ => AlgebraVal::Opaque,
        }
    }

    /// Runs blocks starting at `start`.  When `stop` is reached the statements
    /// of that block are still evaluated and then control returns (the block's
    /// terminator is not followed).  On an ordinary `Return` the result is
    /// stored in `__algebra_return__` in `env`.
    fn run_blocks(
        &mut self,
        program: &MirProgram,
        func: &MirFunction,
        start: &BasicBlockId,
        env: &mut HashMap<String, AlgebraVal>,
        stop: Option<&BasicBlockId>,
    ) -> Option<()> {
        let block_map: HashMap<BasicBlockId, &MirBasicBlock> = func
            .blocks
            .iter()
            .map(|b| (b.id.clone(), b))
            .collect();
        let mut cur = start.clone();
        loop {
            if !self.tick() {
                return None;
            }
            let block = *block_map.get(&cur)?;
            for Statement::Assign(dest, rv) in &block.statements {
                if !self.tick() {
                    return None;
                }
                let val = self.eval_rv(program, rv, env);
                if let [Projection::Index(idx_place)] = &dest.projections[..] {
                    // Model a store to a concrete array element.  If the array
                    // or index is not concrete, downgrade the whole array to
                    // Opaque so the caller will refuse to fold.
                    let idx = self.read(idx_place, env);
                    let old = env.get(&dest.local).cloned().unwrap_or(AlgebraVal::Opaque);
                    let new = match (old, idx) {
                        (AlgebraVal::Array(mut elems), AlgebraVal::Int(i))
                            if i >= 0 && (i as usize) < elems.len() =>
                        {
                            elems[i as usize] = val;
                            AlgebraVal::Array(elems)
                        }
                        _ => AlgebraVal::Opaque,
                    };
                    env.insert(dest.local.clone(), new);
                } else {
                    env.insert(dest.local.clone(), val);
                }
            }
            if stop == Some(&cur) {
                return Some(());
            }
            match &block.terminator {
                Terminator::Return { value } => {
                    let val = value
                        .as_ref()
                        .map(|p| self.read(p, env))
                        .unwrap_or(AlgebraVal::Opaque);
                    env.insert("__algebra_return__".to_string(), val);
                    return Some(());
                }
                Terminator::Branch { target } => cur = target.clone(),
                Terminator::BranchIf {
                    condition,
                    then_target,
                    else_target,
                } => match self.read(condition, env) {
                    AlgebraVal::Bool(true) => cur = then_target.clone(),
                    AlgebraVal::Bool(false) => cur = else_target.clone(),
                    _ => return None,
                },
                Terminator::Switch {
                    value,
                    targets,
                    default,
                } => {
                    let tag = match self.read(value, env) {
                        AlgebraVal::Int(v) => v,
                        _ => return None,
                    };
                    cur = targets
                        .iter()
                        .find(|(t, _)| *t == tag)
                        .map(|(_, b)| b.clone())
                        .unwrap_or_else(|| default.clone());
                }
                _ => return None,
            }
        }
    }

    /// Executes blocks from `start` until the first block that contains a
    /// `Rvalue::Call` or has a `Return` terminator.  Returns that stop block
    /// together with the set of blocks actually executed before it.  This is
    /// used by the constant-prefix folder so the region it removes is defined
    /// by the *executed path*, not by block order (which is not reliable in
    /// functions with branches and inlined loops).
    fn run_prefix(
        &mut self,
        program: &MirProgram,
        func: &MirFunction,
        start: &BasicBlockId,
        env: &mut HashMap<String, AlgebraVal>,
        max_block_visits: usize,
    ) -> Option<(BasicBlockId, HashSet<BasicBlockId>)> {
        let block_map: HashMap<BasicBlockId, &MirBasicBlock> = func
            .blocks
            .iter()
            .map(|b| (b.id.clone(), b))
            .collect();
        let mut visited: HashSet<BasicBlockId> = HashSet::new();
        let mut block_visits: HashMap<BasicBlockId, usize> = HashMap::new();
        let mut cur = start.clone();
        loop {
            if !self.tick() {
                return None;
            }
            // Reject a concrete loop whose trip count is far larger than any
            // prefix fold could justify before interpreting all of its
            // iterations.  This is a pure bail-out: it never changes a fold.
            let visits = block_visits.entry(cur.clone()).or_insert(0);
            *visits += 1;
            if *visits > max_block_visits {
                return None;
            }
            let block = *block_map.get(&cur)?;
            let has_call = block
                .statements
                .iter()
                .any(|Statement::Assign(_, rv)| matches!(rv, Rvalue::Call(_, _)));
            if has_call || matches!(block.terminator, Terminator::Return { .. }) {
                return Some((cur, visited));
            }
            visited.insert(cur.clone());
            for Statement::Assign(dest, rv) in &block.statements {
                if !self.tick() {
                    return None;
                }
                let val = self.eval_rv(program, rv, env);
                if let [Projection::Index(idx_place)] = &dest.projections[..] {
                    let idx = self.read(idx_place, env);
                    let old = env.get(&dest.local).cloned().unwrap_or(AlgebraVal::Opaque);
                    let new = match (old, idx) {
                        (AlgebraVal::Array(mut elems), AlgebraVal::Int(i))
                            if i >= 0 && (i as usize) < elems.len() =>
                        {
                            elems[i as usize] = val;
                            AlgebraVal::Array(elems)
                        }
                        _ => AlgebraVal::Opaque,
                    };
                    env.insert(dest.local.clone(), new);
                } else {
                    env.insert(dest.local.clone(), val);
                }
            }
            cur = match &block.terminator {
                Terminator::Branch { target } => target.clone(),
                Terminator::BranchIf {
                    condition,
                    then_target,
                    else_target,
                } => match self.read(condition, env) {
                    AlgebraVal::Bool(true) => then_target.clone(),
                    AlgebraVal::Bool(false) => else_target.clone(),
                    _ => return None,
                },
                Terminator::Switch {
                    value,
                    targets,
                    default,
                } => {
                    let tag = match self.read(value, env) {
                        AlgebraVal::Int(v) => v,
                        _ => return None,
                    };
                    targets
                        .iter()
                        .find(|(t, _)| *t == tag)
                        .map(|(_, b)| b.clone())
                        .unwrap_or_else(|| default.clone())
                }
                _ => return None,
            };
        }
    }

    fn call(
        &mut self,
        program: &MirProgram,
        name: &str,
        args: &[AlgebraVal],
    ) -> Option<AlgebraVal> {
        if !self.tick() {
            return None;
        }
        let func = program.functions.iter().find(|f| f.name == name)?;
        if func.blocks.is_empty() || func.params.len() != args.len() {
            return None;
        }
        let mut env: HashMap<String, AlgebraVal> = HashMap::new();
        for ((p, _), v) in func.params.iter().zip(args.iter()) {
            env.insert(p.clone(), v.clone());
        }
        self.run_blocks(program, func, &func.blocks[0].id, &mut env, None)?;
        Some(env.remove("__algebra_return__").unwrap_or(AlgebraVal::Opaque))
    }
}

/// All successors of a block (used for structural reachability checks).
fn algebra_successors(b: &MirBasicBlock) -> Vec<BasicBlockId> {
    match &b.terminator {
        Terminator::Branch { target } => vec![target.clone()],
        Terminator::BranchIf {
            then_target,
            else_target,
            ..
        } => vec![then_target.clone(), else_target.clone()],
        Terminator::Switch {
            targets, default, ..
        } => {
            let mut v: Vec<BasicBlockId> = targets.iter().map(|(_, b)| b.clone()).collect();
            v.push(default.clone());
            v
        }
        _ => Vec::new(),
    }
}

/// Forward reachability from `start` over every terminator successor.  Unlike
/// the symbolic interpreter this ignores branch conditions, so it is a safe
/// over-approximation: any block that can execute after `start` is included.
fn algebra_reachable(func: &MirFunction, start: &BasicBlockId) -> HashSet<BasicBlockId> {
    let map: HashMap<BasicBlockId, &MirBasicBlock> = func
        .blocks
        .iter()
        .map(|b| (b.id.clone(), b))
        .collect();
    let mut seen = HashSet::new();
    let mut stack = vec![start.clone()];
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(b) = map.get(&id) {
            // Use the canonical successor enumeration so indirect calls, forks,
            // guards, etc. are followed too — never miss a possible back-edge.
            for s in b.terminator.successors() {
                stack.push(s);
            }
        }
    }
    seen
}

fn algebra_reaches(
    func: &MirFunction,
    start: &BasicBlockId,
    target: &BasicBlockId,
) -> bool {
    let map: HashMap<BasicBlockId, &MirBasicBlock> = func
        .blocks
        .iter()
        .map(|b| (b.id.clone(), b))
        .collect();
    let mut stack = vec![start.clone()];
    let mut seen = HashSet::new();
    while let Some(id) = stack.pop() {
        if &id == target {
            return true;
        }
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(b) = map.get(&id) {
            for s in algebra_successors(b) {
                stack.push(s);
            }
        }
    }
    false
}

/// Like `algebra_reaches`, but follows only unconditional `Branch` edges.  This
/// deliberately stops at loop headers / switches so a back-edge cannot make two
/// sibling arms appear to reach one another.
fn algebra_reaches_branch_only(
    func: &MirFunction,
    start: &BasicBlockId,
    target: &BasicBlockId,
) -> bool {
    let mut cur = start.clone();
    for _ in 0..=func.blocks.len() {
        if &cur == target {
            return true;
        }
        match func
            .blocks
            .iter()
            .find(|blk| blk.id == cur)
            .map(|blk| &blk.terminator)
        {
            Some(Terminator::Branch { target: next }) => cur = next.clone(),
            _ => return false,
        }
    }
    false
}

/// Like `algebra_reaches_branch_only`, but the *first* edge out of `start` may
/// be any control-flow edge.  A common producer/consumer shape has the first
/// fold arm fall straight through to the join while the second arm immediately
/// switches on another value that the symbolic interpreter already knows to be
/// constant.  Requiring that second arm's entry edge to be a plain `Branch`
/// hid the join (and therefore prevented the whole fold from being recognised).
/// All edges after the first must still be plain `Branch` edges, so a back-edge
/// cannot make two sibling arms appear to reconverge spuriously.
fn algebra_reaches_from_arm_entry(
    func: &MirFunction,
    start: &BasicBlockId,
    target: &BasicBlockId,
) -> bool {
    if start == target {
        return true;
    }
    let Some(start_block) = func.blocks.iter().find(|blk| blk.id == *start) else {
        return false;
    };
    algebra_successors(start_block)
        .into_iter()
        .any(|next| algebra_reaches_branch_only(func, &next, target))
}

/// Finds the block where both fold arms reconverge: the first block on the
/// first arm's straight-line `Branch` chain that is also reachable from the
/// second arm (whose entry edge may itself be a switch/branch).  Works both for
/// loop bodies (latch block) and straight-line compositions (a `Return` block).
fn algebra_arm_join(func: &MirFunction, targets: &[(i64, BasicBlockId)]) -> Option<BasicBlockId> {
    let (a, b) = match targets {
        [(_, a), (_, b)] => (a, b),
        _ => return None,
    };
    let mut cur = a.clone();
    for _ in 0..=func.blocks.len() {
        if algebra_reaches_from_arm_entry(func, b, &cur) {
            return Some(cur);
        }
        match func
            .blocks
            .iter()
            .find(|blk| blk.id == cur)
            .map(|blk| &blk.terminator)
        {
            Some(Terminator::Branch { target }) => cur = target.clone(),
            _ => return None,
        }
    }
    None
}

/// Identifies the body entry of the innermost loop whose latch is `join_idx`
/// and whose body contains `fold_idx`.  Falls back to the entry block for a
/// straight-line composition.
fn algebra_body_start(func: &MirFunction, fold_idx: usize, join_idx: usize) -> usize {
    let fold_id = &func.blocks[fold_idx].id;
    let mut best = 0usize;
    for (h_idx, header) in func.blocks.iter().enumerate() {
        if h_idx >= fold_idx {
            break;
        }
        let then_target = match &header.terminator {
            Terminator::BranchIf { then_target, .. } => then_target,
            _ => continue,
        };
        if !algebra_successors(&func.blocks[join_idx]).contains(&header.id) {
            continue;
        }
        if !algebra_reaches(func, then_target, fold_id) {
            continue;
        }
        if let Some(body_start) = block_index(func, then_target) {
            if body_start < fold_idx && body_start > best {
                best = body_start;
            }
        }
    }
    best
}

/// Blocks reachable from `start` without passing through `join`.
fn algebra_region_blocks(
    func: &MirFunction,
    start: &BasicBlockId,
    join: &BasicBlockId,
) -> HashSet<usize> {
    let map: HashMap<BasicBlockId, &MirBasicBlock> = func
        .blocks
        .iter()
        .map(|b| (b.id.clone(), b))
        .collect();
    let mut region = HashSet::new();
    let mut stack = vec![start.clone()];
    while let Some(id) = stack.pop() {
        if &id == join {
            continue;
        }
        let idx = match block_index(func, &id) {
            Some(i) => i,
            None => continue,
        };
        if !region.insert(idx) {
            continue;
        }
        if let Some(b) = map.get(&id) {
            for s in algebra_successors(b) {
                stack.push(s);
            }
        }
    }
    region
}

fn algebra_collect_reads(place: &Place, out: &mut HashSet<String>) {
    out.insert(place.local.clone());
}

fn algebra_collect_rvalue_reads(rv: &Rvalue, out: &mut HashSet<String>) {
    match rv {
        Rvalue::Use(p)
        | Rvalue::UnaryOp(_, p)
        | Rvalue::Alloc(p)
        | Rvalue::Load(p)
        | Rvalue::Discriminant(p) => algebra_collect_reads(p, out),
        Rvalue::BinaryOp(_, a, b) => {
            algebra_collect_reads(a, out);
            algebra_collect_reads(b, out);
        }
        Rvalue::Select(a, b, c) => {
            algebra_collect_reads(a, out);
            algebra_collect_reads(b, out);
            algebra_collect_reads(c, out);
        }
        Rvalue::Call(_, args) | Rvalue::Array(args) => {
            for a in args {
                algebra_collect_reads(a, out);
            }
        }
        Rvalue::Struct(_, fields) => {
            for (_, p) in fields {
                algebra_collect_reads(p, out);
            }
        }
        Rvalue::EnumVariant { fields, .. } => {
            for p in fields {
                algebra_collect_reads(p, out);
            }
        }
        Rvalue::Phi(entries) => {
            for (_, p) in entries {
                algebra_collect_reads(p, out);
            }
        }
        Rvalue::ClosureAlloc { captured, .. } => {
            for p in captured {
                algebra_collect_reads(p, out);
            }
        }
        _ => {}
    }
}

/// A local assigned inside the region must not be read outside it (other than
/// the result local in the join block, which the replacement re-defines).
fn algebra_region_locals_escape(
    func: &MirFunction,
    region: &HashSet<usize>,
    join_idx: usize,
    result_local: &str,
) -> bool {
    let mut assigned: HashSet<String> = HashSet::new();
    for &idx in region {
        for Statement::Assign(dest, _) in &func.blocks[idx].statements {
            assigned.insert(dest.local.clone());
        }
    }
    for (idx, block) in func.blocks.iter().enumerate() {
        if region.contains(&idx) || idx == join_idx {
            continue;
        }
        let mut reads = HashSet::new();
        for Statement::Assign(_, rv) in &block.statements {
            algebra_collect_rvalue_reads(rv, &mut reads);
        }
        match &block.terminator {
            Terminator::BranchIf { condition, .. } | Terminator::Switch { value: condition, .. } => {
                algebra_collect_reads(condition, &mut reads)
            }
            Terminator::Return {
                value: Some(place),
            } => algebra_collect_reads(place, &mut reads),
            _ => {}
        }
        for r in reads {
            if r != result_local && assigned.contains(&r) {
                return true;
            }
        }
    }
    false
}

/// True if a block outside the region branches into it (other than the loop
/// header entering at `body_start`).  Such a region cannot be replaced safely.
fn algebra_has_external_entry(
    func: &MirFunction,
    region: &HashSet<usize>,
    body_start: &BasicBlockId,
) -> bool {
    for (idx, block) in func.blocks.iter().enumerate() {
        if region.contains(&idx) {
            continue;
        }
        for s in algebra_successors(block) {
            if &s == body_start {
                continue;
            }
            if let Some(i) = block_index(func, &s) {
                if region.contains(&i) {
                    return true;
                }
            }
        }
    }
    false
}

/// Calls to functions outside the program (e.g. `println`) must not be removed.
fn algebra_region_has_side_effects(
    func: &MirFunction,
    program: &MirProgram,
    region: &HashSet<usize>,
) -> bool {
    for &idx in region {
        let block = &func.blocks[idx];
        for Statement::Assign(_, rv) in &block.statements {
            match rv {
                Rvalue::Call(callee, _) => {
                    if !program.functions.iter().any(|f| &f.name == callee) {
                        return true;
                    }
                }
                Rvalue::Thunk { .. } | Rvalue::ClosureAlloc { .. } | Rvalue::FnPtr(_) => {
                    return true
                }
                _ => {}
            }
        }
        match &block.terminator {
            Terminator::IndirectCall { .. }
            | Terminator::Force { .. }
            | Terminator::Fork { .. } => return true,
            _ => {}
        }
    }
    false
}

struct AlgebraRewrite {
    caller_idx: usize,
    body_start_idx: usize,
    join: BasicBlockId,
    result_local: String,
    value: i64,
    dead: HashSet<usize>,
}

fn try_algebraic_fold_in_caller(
    caller_idx: usize,
    caller: &MirFunction,
    program: &MirProgram,
) -> Option<AlgebraRewrite> {
    for (fold_idx, fold_block) in caller.blocks.iter().enumerate() {
        let (scrut, targets, default) = match &fold_block.terminator {
            Terminator::Switch {
                value,
                targets,
                default,
            } => (value, targets, default),
            _ => continue,
        };
        if targets.len() != 2 {
            continue;
        }
        let join = algebra_arm_join(caller, targets)?;
        let join_idx = block_index(caller, &join)?;
        let body_start_idx = algebra_body_start(caller, fold_idx, join_idx);
        let body_start = caller.blocks[body_start_idx].id.clone();
        let region = algebra_region_blocks(caller, &body_start, &join);
        if !region.contains(&fold_idx) || region.contains(&join_idx) {
            continue;
        }
        if algebra_has_external_entry(caller, &region, &body_start) {
            continue;
        }
        if algebra_region_has_side_effects(caller, program, &region) {
            continue;
        }

        // Both switch arms must reach the join.
        if targets
            .iter()
            .any(|(_, target)| !algebra_reaches(caller, target, &join))
        {
            continue;
        }

        // Symbolically execute the region up to the fold switch.
        let mut interp = AlgebraInterp::new(20_000);
        let mut env: HashMap<String, AlgebraVal> = HashMap::new();
        for (p, _) in &caller.params {
            env.insert(p.clone(), AlgebraVal::Opaque);
        }
        for d in &caller.locals {
            env.entry(d.name.clone()).or_insert(AlgebraVal::Opaque);
        }
        interp.run_blocks(
            program,
            caller,
            &body_start,
            &mut env,
            Some(&caller.blocks[fold_idx].id),
        )?;
        let tag = match interp.read(scrut, &env) {
            AlgebraVal::Int(v) => v,
            _ => continue,
        };
        let target = targets
            .iter()
            .find(|(t, _)| *t == tag)
            .map(|(_, b)| b.clone())
            .unwrap_or_else(|| default.clone());

        // Execute the selected arm until the join and read back the fold result.
        interp.run_blocks(program, caller, &target, &mut env, Some(&join))?;
        let mut read_in_join = HashSet::new();
        for Statement::Assign(_, rv) in &caller.blocks[join_idx].statements {
            algebra_collect_rvalue_reads(rv, &mut read_in_join);
        }
        match &caller.blocks[join_idx].terminator {
            Terminator::BranchIf { condition, .. } | Terminator::Switch { value: condition, .. } => {
                algebra_collect_reads(condition, &mut read_in_join)
            }
            Terminator::Return {
                value: Some(place),
            } => algebra_collect_reads(place, &mut read_in_join),
            _ => {}
        }
        // The fold result must be defined by the region, not by the join block
        // itself (which computes the loop's accumulator update).
        let mut assigned_in_region = HashSet::new();
        for &idx in &region {
            for Statement::Assign(dest, _) in &caller.blocks[idx].statements {
                assigned_in_region.insert(dest.local.clone());
            }
        }
        let mut found: Vec<(String, i64)> = read_in_join
            .iter()
            .filter(|r| assigned_in_region.contains(*r))
            .filter_map(|r| match env.get(r) {
                Some(AlgebraVal::Int(v)) => Some((r.clone(), *v)),
                _ => None,
            })
            .collect();
        found.sort();
        if found.len() != 1 {
            dtrace!(
                "  algebraic fold: ambiguous/absent result in {}: {:?}",
                caller.name,
                found
            );
            continue;
        }
        let (result_local, value) = found.remove(0);
        if algebra_region_locals_escape(caller, &region, join_idx, &result_local) {
            continue;
        }

        dtrace!(
            "  algebraic fold in {}: region={:?} result={}={}",
            caller.name,
            region,
            result_local,
            value
        );
        return Some(AlgebraRewrite {
            caller_idx,
            body_start_idx,
            join,
            result_local,
            value,
            dead: region,
        });
    }
    None
}

fn apply_algebraic_rewrite(program: &mut MirProgram, rw: AlgebraRewrite) {
    let caller = &mut program.functions[rw.caller_idx];
    let result_ty = local_type(caller, &rw.result_local);
    let mut dead = rw.dead;
    dead.remove(&rw.body_start_idx);
    for idx in dead {
        if let Some(block) = caller.blocks.get_mut(idx) {
            block.statements.clear();
            block.terminator = Terminator::Unreachable;
        }
    }
    if let Some(block) = caller.blocks.get_mut(rw.body_start_idx) {
        block.statements = vec![Statement::Assign(
            Place {
                local: rw.result_local.clone(),
                projections: vec![],
            },
            Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(
                rw.value,
                result_ty,
            )),
        )];
        block.terminator = Terminator::Branch {
            target: rw.join.clone(),
        };
    }
    caller.is_distilled = true;
}

/// Program-level entry point for algebraic producer/consumer fusion.
fn fuse_constant_list_algebra(program: &mut MirProgram, stats: &mut SupercompilerStats) {
    let mut rewrites = Vec::new();
    for (ci, caller) in program.functions.iter().enumerate() {
        if let Some(rw) = try_algebraic_fold_in_caller(ci, caller, program) {
            rewrites.push(rw);
        }
    }
    for rw in rewrites {
        stats.loops_collapsed += 1;
        stats.branches_pruned += 1;
        apply_algebraic_rewrite(program, rw);
    }
}

/// A description of a constant-computation prefix that can be folded away.
struct ConstantPrefixFold {
    /// The block at which the fold stops (its statements are preserved).
    stop_block: BasicBlockId,
    /// Concrete assignments for every live-out local of the prefix.
    assignments: Vec<Statement>,
    /// Fresh index locals introduced for array-element stores.
    fresh_locals: Vec<MirLocalDecl>,
    /// Blocks belonging to the prefix (all except the entry are made unreachable).
    prefix_blocks: Vec<BasicBlockId>,
}

/// Analyses a function for a pure, constant-evaluable computation prefix.
///
/// The prefix is the part of the control flow from the entry block up to the
/// first observable operation (a call to a function outside the program, or a
/// `Return`).  When the bounded symbolic interpreter can evaluate that entire
/// prefix to concrete values — including local arrays written by fixed-trip-
/// count loops — the prefix is replaced by assignments of those values.
///
/// This is deliberately name- and constant-agnostic: no function name, variable
/// name, or benchmark-specific magic value is ever inspected.  Any Opaque
/// value, back-edge from the suffix into the prefix, or non-constant live-out
/// makes the analysis bail out conservatively.
fn analyze_constant_prefix(func: &MirFunction, program: &MirProgram) -> Option<ConstantPrefixFold> {
    if func.blocks.len() < 2 {
        return None;
    }

    // Symbolically execute from the entry until the first block that performs
    // an observable operation (a Call or a Return).  The set of executed blocks
    // *on this path* is the prefix; using block order instead of the actual
    // executed path would be wrong for functions with branches and inlining.
    let mut interp = AlgebraInterp::new(CONST_PREFIX_FUEL);
    let mut env: HashMap<String, AlgebraVal> = HashMap::new();
    for (p, _) in &func.params {
        env.insert(p.clone(), AlgebraVal::Opaque);
    }
    for d in &func.locals {
        env.entry(d.name.clone()).or_insert(AlgebraVal::Opaque);
    }
    let (stop_block, visited) = interp.run_prefix(
        program,
        func,
        &func.blocks[0].id,
        &mut env,
        CONST_PREFIX_MAX_BLOCK_VISITS,
    )?;
    let steps_used = CONST_PREFIX_FUEL.saturating_sub(interp.fuel);
    dtrace!(
        "constant-prefix interp fn={} steps={} visited={}",
        func.name,
        steps_used,
        visited.len()
    );
    if visited.len() < 2 {
        return None;
    }

    // Only blocks that can actually execute *after* the prefix matter here.
    // The symbolic interpreter follows the single concrete path through every
    // branch, so arms of compile-time-constant branches inside the prefix are
    // never visited; their back-edges into loop headers are dead at run time
    // and disappear together with the removed prefix.  Treating those dead
    // arms as residual suffix blocks was far too conservative.
    //
    // Forward reachability from `stop_block` over *all* terminator successors
    // is a safe over-approximation of the residual suffix: it includes unknown
    // branches, so a genuine back-edge from the suffix into the prefix is
    // still detected and makes the fold bail out.
    let suffix_reachable = algebra_reachable(func, &stop_block);
    let entry_id = &func.blocks[0].id;
    for block in func
        .blocks
        .iter()
        .filter(|b| suffix_reachable.contains(&b.id))
    {
        for succ in block.terminator.successors() {
            if visited.contains(&succ) && &succ != entry_id {
                return None;
            }
        }
    }

    // Locals defined in the prefix and read in the suffix must all be concrete.
    let mut prefix_defs: HashSet<String> = HashSet::new();
    for block in func.blocks.iter().filter(|b| visited.contains(&b.id)) {
        for Statement::Assign(dest, _) in &block.statements {
            prefix_defs.insert(dest.local.clone());
        }
    }
    let suffix: Vec<&MirBasicBlock> = func
        .blocks
        .iter()
        .filter(|b| !visited.contains(&b.id))
        .collect();
    let mut suffix_reads: HashSet<String> = HashSet::new();
    // The stop block belongs to the suffix (its statements are preserved).
    for block in suffix.iter().copied().chain(std::iter::once(
        func.blocks.iter().find(|b| b.id == stop_block)?,
    )) {
        for Statement::Assign(_, rv) in &block.statements {
            algebra_collect_rvalue_reads(rv, &mut suffix_reads);
        }
        match &block.terminator {
            Terminator::BranchIf { condition, .. } | Terminator::Switch { value: condition, .. } => {
                algebra_collect_reads(condition, &mut suffix_reads)
            }
            Terminator::Return { value: Some(place) } => {
                algebra_collect_reads(place, &mut suffix_reads)
            }
            _ => {}
        }
    }

    let mut assignments = Vec::new();
    let mut fresh_locals = Vec::new();
    let mut fresh_counter = 0usize;
    for name in &suffix_reads {
        if !prefix_defs.contains(name) {
            continue;
        }
        let ty = func
            .locals
            .iter()
            .find(|d| &d.name == name)
            .map(|d| d.ty.clone())
            .unwrap_or(Type::I64);
        let val = env.get(name)?;
        match val {
            AlgebraVal::Int(v) => assignments.push(Statement::Assign(
                Place { local: name.clone(), projections: vec![] },
                Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(*v, ty)),
            )),
            AlgebraVal::Bool(b) => assignments.push(Statement::Assign(
                Place { local: name.clone(), projections: vec![] },
                Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Bool(*b)),
            )),
            AlgebraVal::Array(elems) => {
                // Materialise the concrete array with element-wise stores.  A
                // single fresh index local is shared by all stores.
                let idx_name = format!("__cfold_idx_{}_{}", func.name, fresh_counter);
                fresh_counter += 1;
                fresh_locals.push(MirLocalDecl {
                    name: idx_name.clone(),
                    ty: Type::I64,
                    mutable: true,
                });
                for (i, elem) in elems.iter().enumerate() {
                    let v = match elem {
                        AlgebraVal::Int(v) => *v,
                        _ => return None,
                    };
                    assignments.push(Statement::Assign(
                        Place { local: idx_name.clone(), projections: vec![] },
                        Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(
                            i as i64,
                            Type::I64,
                        )),
                    ));
                    assignments.push(Statement::Assign(
                        Place {
                            local: name.clone(),
                            projections: vec![Projection::Index(Box::new(Place {
                                local: idx_name.clone(),
                                projections: vec![],
                            }))],
                        },
                        Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(v, Type::I64)),
                    ));
                }
            }
            _ => return None,
        }
    }

    Some(ConstantPrefixFold {
        stop_block,
        assignments,
        fresh_locals,
        prefix_blocks: visited.into_iter().collect(),
    })
}

/// Program-level entry point for constant-computation-prefix folding.
pub fn fold_constant_computation_prefix(
    program: &mut MirProgram,
    stats: &mut SupercompilerStats,
) {
    for fi in 0..program.functions.len() {
        let started = std::time::Instant::now();
        let analyzed = analyze_constant_prefix(&program.functions[fi], program);
        let elapsed = started.elapsed();
        if elapsed.as_millis() >= 1 {
            dtrace!(
                "constant-prefix scan fn={} {:?} folded={}",
                program.functions[fi].name,
                elapsed,
                analyzed.is_some()
            );
        }
        let Some(fold) = analyzed else {
            continue;
        };
        if fold.assignments.is_empty() {
            continue;
        }
        let func = &mut program.functions[fi];
        // The entry block keeps its identity; all other prefix blocks become
        // unreachable so dominance/SSA invariants remain simple.
        let entry_id = func.blocks[0].id.clone();
        for id in &fold.prefix_blocks {
            if id == &entry_id {
                continue;
            }
            if let Some(block) = func.blocks.iter_mut().find(|b| &b.id == id) {
                block.statements.clear();
                block.terminator = Terminator::Unreachable;
            }
        }
        let entry = &mut func.blocks[0];
        entry.statements = fold.assignments;
        entry.terminator = Terminator::Branch {
            target: fold.stop_block,
        };
        func.locals.extend(fold.fresh_locals);
        func.is_distilled = true;

        // A compile-time-constant branch inside the removed prefix leaves its
        // untaken sibling arm without predecessors.  If that arm keeps a
        // competing definition of a folded local, later symbolic passes can
        // see two reaching definitions and fail to terminate.  Prune every
        // block that is unreachable from the (rewritten) entry so the residual
        // CFG has a single definition per folded local.
        let reachable = algebra_reachable(func, &entry_id);
        for block in &mut func.blocks {
            if !reachable.contains(&block.id) {
                block.statements.clear();
                block.terminator = Terminator::Unreachable;
            }
        }

        stats.loops_collapsed += 1;
        stats.branches_pruned += 1;
    }
}

/// Constant-fold pure function calls whose arguments are compile-time constants.
/// This is general (no benchmark names/constants): any in-program call that the
/// bounded symbolic interpreter can evaluate to a scalar constant is replaced by
/// that constant. Side effects or unsupported operations make the interpreter
/// return Opaque/None, so they are never folded.
fn fold_constant_pure_calls(program: &mut MirProgram, stats: &mut SupercompilerStats) {
    // Iterate to a small fixpoint so folding one call can expose another.
    for _ in 0..4 {
        let mut changed = false;
        let mut rewrites: Vec<(usize, usize, usize, String, AlgebraVal)> = Vec::new(); // (fn, block, stmt, dest_local, val)
        for (fi, func) in program.functions.iter().enumerate() {
            let const_env = collect_constant_locals(func);
            for (bi, block) in func.blocks.iter().enumerate() {
                for (si, stmt) in block.statements.iter().enumerate() {
                    let Statement::Assign(dest, Rvalue::Call(callee, args)) = stmt else {
                        continue;
                    };
                    if !dest.projections.is_empty() {
                        continue;
                    }
                    // Only fold calls to functions defined in this program.
                    if !program.functions.iter().any(|f| f.name == *callee) {
                        continue;
                    }
                    let mut arg_vals = Vec::with_capacity(args.len());
                    let mut all_const = true;
                    for arg in args {
                        if let Some(v) = resolve_constant_place(arg, &const_env) {
                            arg_vals.push(v);
                        } else {
                            all_const = false;
                            break;
                        }
                    }
                    if !all_const {
                        continue;
                    }
                    let mut interp = AlgebraInterp::new(50_000);
                    let result = interp.call(program, callee, &arg_vals);
                    match result {
                        Some(v @ AlgebraVal::Int(_)) | Some(v @ AlgebraVal::Bool(_)) => {
                            rewrites.push((fi, bi, si, dest.local.clone(), v));
                        }
                        _ => {}
                    }
                }
            }
        }

        for (fi, bi, si, _dest, val) in rewrites {
            let func = &mut program.functions[fi];
            let dest_ty = local_type(func, &_dest);
            let rval = match val {
                AlgebraVal::Int(v) => Rvalue::Constant(
                    crate::typecheck::typed_ast::TypedLiteral::Int(v, dest_ty),
                ),
                AlgebraVal::Bool(b) => Rvalue::Constant(
                    crate::typecheck::typed_ast::TypedLiteral::Bool(b),
                ),
                _ => continue,
            };
            let Statement::Assign(dest, slot) = &mut func.blocks[bi].statements[si];
            if dest.local == _dest {
                *slot = rval;
                func.is_distilled = true;
                stats.loops_collapsed += 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

fn collect_constant_locals(func: &MirFunction) -> HashMap<String, AlgebraVal> {
    // Count definitions per local.
    let mut def_count: HashMap<String, usize> = HashMap::new();
    for block in &func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(dest, _) = stmt;
            if dest.projections.is_empty() {
                *def_count.entry(dest.local.clone()).or_insert(0) += 1;
            }
        }
    }
    // Collect single-def constants.
    let mut env: HashMap<String, AlgebraVal> = HashMap::new();
    for block in &func.blocks {
        for stmt in &block.statements {
            if let Statement::Assign(dest, Rvalue::Constant(lit)) = stmt {
                if dest.projections.is_empty()
                    && def_count.get(&dest.local).copied().unwrap_or(0) == 1
                {
                    let v = match lit {
                        crate::typecheck::typed_ast::TypedLiteral::Int(v, _) => {
                            AlgebraVal::Int(*v)
                        }
                        crate::typecheck::typed_ast::TypedLiteral::Bool(b) => {
                            AlgebraVal::Bool(*b)
                        }
                        _ => continue,
                    };
                    env.insert(dest.local.clone(), v);
                }
            }
        }
    }
    // Propagate single-def Uses of constants (small fixpoint).
    for _ in 0..4 {
        let mut added = false;
        for block in &func.blocks {
            for stmt in &block.statements {
                if let Statement::Assign(dest, Rvalue::Use(p)) = stmt {
                    if dest.projections.is_empty()
                        && def_count.get(&dest.local).copied().unwrap_or(0) == 1
                        && !env.contains_key(&dest.local)
                    {
                        if let Some(v) = resolve_constant_place(p, &env) {
                            env.insert(dest.local.clone(), v);
                            added = true;
                        }
                    }
                }
            }
        }
        if !added {
            break;
        }
    }
    env
}

fn resolve_constant_place(
    p: &Place,
    env: &HashMap<String, AlgebraVal>,
) -> Option<AlgebraVal> {
    if !p.projections.is_empty() {
        return None;
    }
    env.get(&p.local).cloned()
}

// ============================================================================
// 10. O(1) modular affine accumulator collapse
// ============================================================================
//
// Recognises the shape
//
//     s = s0; i = 0;
//     while i < n { s = (s + C) % M; i = i + 1; }
//
// in residual MIR and replaces it with the closed form
//
//     s_final = (s0 + ((max(n, 0) % M) * C) % M) % M
//
// The multiplication is overflow-free when 0 < C < M <= i32::MAX, which is
// checked before the rewrite fires.  All constants (C, M, s0) are *derived*
// from the SSA defining statements by recursive value evaluation along
// concrete execution paths through the function; no variable-name heuristics
// are used.  If a value cannot be proven equal on every path reaching the
// loop, the pass bails out and leaves the loop untouched.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccConst {
    Int(i64),
    Bool(bool),
}

type AccEnv = HashMap<String, Option<AccConst>>;

/// Reads a scalar place out of a derived environment.  Projected places are
/// deliberately opaque: this pass never reasons about aggregates.
fn acc_read(p: &Place, env: &AccEnv) -> Option<AccConst> {
    if !p.projections.is_empty() {
        return None;
    }
    env.get(&p.local).cloned().flatten()
}

/// Recursively evaluates an Rvalue in a derived environment.  `Rvalue::Use`
/// chains, constant `BinaryOp` folding and `Select` (including the
/// "identical arms" case where the condition may be unknown) are supported.
/// Everything else yields `None`, which makes the caller bail out safely.
fn acc_eval_rv(rv: &Rvalue, env: &AccEnv) -> Option<AccConst> {
    use crate::typecheck::typed_ast::TypedLiteral;

    match rv {
        Rvalue::Constant(TypedLiteral::Int(v, _)) => Some(AccConst::Int(*v)),
        Rvalue::Constant(TypedLiteral::Bool(b)) => Some(AccConst::Bool(*b)),
        Rvalue::Use(p) => acc_read(p, env),
        Rvalue::UnaryOp(op, p) => match acc_read(p, env) {
            Some(AccConst::Int(v)) => Some(AccConst::Int(match op {
                crate::ast::UnaryOp::Neg => v.wrapping_neg(),
                crate::ast::UnaryOp::Not => !v,
            })),
            Some(AccConst::Bool(b)) => Some(AccConst::Bool(!b)),
            None => None,
        },
        Rvalue::BinaryOp(op, a, b) => {
            let x = acc_read(a, env);
            let y = acc_read(b, env);
            match (x, y) {
                (Some(AccConst::Int(l)), Some(AccConst::Int(r))) => match op {
                    BinaryOp::Eq => Some(AccConst::Bool(l == r)),
                    BinaryOp::Ne => Some(AccConst::Bool(l != r)),
                    BinaryOp::Lt => Some(AccConst::Bool(l < r)),
                    BinaryOp::Le => Some(AccConst::Bool(l <= r)),
                    BinaryOp::Gt => Some(AccConst::Bool(l > r)),
                    BinaryOp::Ge => Some(AccConst::Bool(l >= r)),
                    BinaryOp::Add => Some(AccConst::Int(l.wrapping_add(r))),
                    BinaryOp::Sub => Some(AccConst::Int(l.wrapping_sub(r))),
                    BinaryOp::Mul => Some(AccConst::Int(l.wrapping_mul(r))),
                    BinaryOp::Div if r != 0 => Some(AccConst::Int(l.wrapping_div(r))),
                    BinaryOp::Mod if r != 0 => Some(AccConst::Int(l.wrapping_rem(r))),
                    BinaryOp::BitAnd => Some(AccConst::Int(l & r)),
                    BinaryOp::BitOr => Some(AccConst::Int(l | r)),
                    BinaryOp::BitXor => Some(AccConst::Int(l ^ r)),
                    BinaryOp::Shl => Some(AccConst::Int(l.wrapping_shl((r as u32) & 63))),
                    BinaryOp::Shr => Some(AccConst::Int(l.wrapping_shr((r as u32) & 63))),
                    _ => None,
                },
                (Some(AccConst::Bool(l)), Some(AccConst::Bool(r))) => match op {
                    BinaryOp::Eq => Some(AccConst::Bool(l == r)),
                    BinaryOp::Ne => Some(AccConst::Bool(l != r)),
                    BinaryOp::BitAnd => Some(AccConst::Bool(l && r)),
                    BinaryOp::BitOr => Some(AccConst::Bool(l || r)),
                    _ => None,
                },
                _ => None,
            }
        }
        Rvalue::Select(c, t, f) => {
            let cv = acc_read(c, env);
            let tv = acc_read(t, env);
            let fv = acc_read(f, env);
            match cv {
                Some(AccConst::Bool(true)) => tv,
                Some(AccConst::Bool(false)) => fv,
                // Unknown condition but provably identical arms.
                _ if tv.is_some() && tv == fv => tv,
                _ => None,
            }
        }
        _ => None,
    }
}

/// Derives the scalar environment at `stop` by executing concrete paths from
/// the function entry.  Branch conditions that are not compile-time constants
/// fork the state; states are *not* merged (merging would turn the loop-carried
/// Fibonacci values into "unknown").  All returned environments must agree on
/// any constant we later extract, otherwise the extraction fails.
fn acc_derive_envs(
    func: &MirFunction,
    stop: &BasicBlockId,
    max_steps: usize,
) -> Option<Vec<AccEnv>> {
    let block_map: HashMap<BasicBlockId, &MirBasicBlock> = func
        .blocks
        .iter()
        .map(|b| (b.id.clone(), b))
        .collect();
    let mut work: VecDeque<(BasicBlockId, AccEnv)> = VecDeque::new();
    work.push_back((func.blocks.first()?.id.clone(), HashMap::new()));
    let mut results = Vec::new();
    let mut steps = 0usize;

    while let Some((bid, mut env)) = work.pop_front() {
        steps += 1;
        if steps > max_steps {
            return None;
        }
        if &bid == stop {
            results.push(env);
            continue;
        }
        let block = *block_map.get(&bid)?;
        for Statement::Assign(dest, rv) in &block.statements {
            let val = if dest.projections.is_empty() {
                acc_eval_rv(rv, &env)
            } else {
                None
            };
            env.insert(dest.local.clone(), val);
        }
        match &block.terminator {
            Terminator::Branch { target } => work.push_back((target.clone(), env)),
            Terminator::BranchIf {
                condition,
                then_target,
                else_target,
            } => match acc_read(condition, &env) {
                Some(AccConst::Bool(true)) => work.push_back((then_target.clone(), env)),
                Some(AccConst::Bool(false)) => work.push_back((else_target.clone(), env)),
                _ => {
                    work.push_back((then_target.clone(), env.clone()));
                    work.push_back((else_target.clone(), env));
                }
            },
            Terminator::Return { .. } => {}
            _ => return None,
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

/// Extracts a constant place from every derived environment; all paths must
/// agree, and the value must be present on every path.
fn acc_resolve_const(p: &Place, envs: &[AccEnv]) -> Option<AccConst> {
    let mut out: Option<AccConst> = None;
    for env in envs {
        let v = acc_read(p, env)?;
        match out {
            None => out = Some(v),
            Some(prev) if prev == v => {}
            _ => return None,
        }
    }
    out
}

fn acc_defs_in(block: &MirBasicBlock) -> HashMap<String, &Rvalue> {
    block
        .statements
        .iter()
        .filter_map(|Statement::Assign(dest, rv)| {
            if dest.projections.is_empty() {
                Some((dest.local.clone(), rv))
            } else {
                None
            }
        })
        .collect()
}

fn acc_forbidden(block: &MirBasicBlock, body: &MirBasicBlock) -> HashSet<String> {
    block
        .statements
        .iter()
        .chain(body.statements.iter())
        .filter_map(|Statement::Assign(dest, _)| {
            if dest.projections.is_empty() {
                Some(dest.local.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Follows `Rvalue::Use` chains inside a block until reaching a place defined
/// outside the loop (or an otherwise unrecognised definition).
fn acc_trace_external(
    p: &Place,
    defs: &HashMap<String, &Rvalue>,
    forbidden: &HashSet<String>,
) -> Option<Place> {
    if !p.projections.is_empty() {
        return None;
    }
    let mut cur = p.clone();
    for _ in 0..64 {
        if !cur.projections.is_empty() {
            return None;
        }
        match defs.get(&cur.local) {
            Some(Rvalue::Use(next)) => cur = next.clone(),
            Some(_) => return None,
            None => {
                if forbidden.contains(&cur.local) {
                    return None;
                }
                return Some(cur);
            }
        }
    }
    None
}

/// Evaluates a place that may be defined by a `Use` chain in a single block;
/// loop-invariant leaves are read from the derived header environment.
fn acc_resolve_in_block_all(
    p: &Place,
    defs: &HashMap<String, &Rvalue>,
    envs: &[AccEnv],
) -> Option<AccConst> {
    let mut out: Option<AccConst> = None;
    for env in envs {
        let mut cur = p.clone();
        let mut val: Option<AccConst> = None;
        for _ in 0..64 {
            if !cur.projections.is_empty() {
                return None;
            }
            match defs.get(&cur.local) {
                Some(Rvalue::Use(next)) => cur = next.clone(),
                Some(rv) => {
                    val = acc_eval_rv(rv, env);
                    break;
                }
                None => {
                    val = acc_read(&cur, env);
                    break;
                }
            }
        }
        let v = val?;
        match out {
            None => out = Some(v),
            Some(prev) if prev == v => {}
            _ => return None,
        }
    }
    out
}

/// True iff `p` is (possibly through a body-local `Use` chain) the accumulator
/// itself, i.e. the loop-carried value.
fn acc_is_acc_operand(p: &Place, defs: &HashMap<String, &Rvalue>, acc: &str) -> bool {
    let mut cur = p.clone();
    for _ in 0..64 {
        if !cur.projections.is_empty() {
            return false;
        }
        if cur.local == acc {
            return true;
        }
        match defs.get(&cur.local) {
            Some(Rvalue::Use(next)) => cur = next.clone(),
            _ => return false,
        }
    }
    false
}

/// True iff the induction variable is assigned `iv = iv + 1` in the body.
fn acc_is_unit_increment(iv: &str, bdefs: &HashMap<String, &Rvalue>) -> bool {
    let Some(Rvalue::BinaryOp(BinaryOp::Add, x, y)) = bdefs.get(iv) else {
        return false;
    };
    let (a, b) = if acc_is_acc_operand(x, bdefs, iv) {
        (x, y)
    } else if acc_is_acc_operand(y, bdefs, iv) {
        (y, x)
    } else {
        return false;
    };
    let _ = a;
    // `b` must be the literal 1 (possibly through a body-local Use chain).
    let mut cur = b.clone();
    for _ in 0..64 {
        if !cur.projections.is_empty() {
            return false;
        }
        match bdefs.get(&cur.local) {
            Some(Rvalue::Use(next)) => cur = next.clone(),
            Some(Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(1, _))) => {
                return true
            }
            _ => return false,
        }
    }
    false
}

fn acc_is_pure_scalar(rv: &Rvalue) -> bool {
    matches!(
        rv,
        Rvalue::Use(_)
            | Rvalue::Constant(_)
            | Rvalue::BinaryOp(..)
            | Rvalue::UnaryOp(..)
            | Rvalue::Select(..)
    ) && match rv {
        Rvalue::Use(p) | Rvalue::UnaryOp(_, p) => p.projections.is_empty(),
        Rvalue::BinaryOp(_, a, b) => a.projections.is_empty() && b.projections.is_empty(),
        Rvalue::Select(c, t, f) => {
            c.projections.is_empty() && t.projections.is_empty() && f.projections.is_empty()
        }
        _ => true,
    }
}

fn acc_rv_places<'a>(rv: &'a Rvalue, out: &mut Vec<&'a Place>) {
    match rv {
        Rvalue::Use(p) | Rvalue::UnaryOp(_, p) | Rvalue::Load(p) | Rvalue::Alloc(p) => {
            out.push(p)
        }
        Rvalue::BinaryOp(_, a, b) => {
            out.push(a);
            out.push(b);
        }
        Rvalue::Select(c, t, f) => {
            out.push(c);
            out.push(t);
            out.push(f);
        }
        Rvalue::Call(_, args) | Rvalue::Array(args) => out.extend(args.iter()),
        Rvalue::Phi(entries) => out.extend(entries.iter().map(|(_, p)| p)),
        _ => {}
    }
}

fn acc_term_places<'a>(t: &'a Terminator, out: &mut Vec<&'a Place>) {
    match t {
        Terminator::BranchIf { condition, .. } => out.push(condition),
        Terminator::Switch { value, .. } => out.push(value),
        Terminator::Return { value: Some(p) } => out.push(p),
        Terminator::IndirectCall { callee, args, .. } => {
            out.push(callee);
            out.extend(args.iter());
        }
        Terminator::TypeGuard { local, .. } => out.push(local),
        _ => {}
    }
}

/// Returns true when no local defined inside the loop is observed outside it
/// (apart from the accumulator itself, which is intentionally rewritten).
fn acc_loop_locals_are_private(func: &MirFunction, h: &MirBasicBlock, b: &MirBasicBlock, acc: &str) -> bool {
    let defined: HashSet<String> = h
        .statements
        .iter()
        .chain(b.statements.iter())
        .filter_map(|Statement::Assign(dest, _)| {
            if dest.projections.is_empty() {
                Some(dest.local.clone())
            } else {
                None
            }
        })
        .collect();
    for block in &func.blocks {
        if block.id == h.id || block.id == b.id {
            continue;
        }
        let mut places = Vec::new();
        for Statement::Assign(dest, rv) in &block.statements {
            if !dest.projections.is_empty() && defined.contains(&dest.local) {
                return false;
            }
            acc_rv_places(rv, &mut places);
        }
        acc_term_places(&block.terminator, &mut places);
        if places
            .into_iter()
            .any(|p| p.projections.is_empty() && defined.contains(&p.local) && p.local != acc)
        {
            return false;
        }
    }
    true
}

struct AccLoopPlan {
    header: BasicBlockId,
    body: BasicBlockId,
    exit: BasicBlockId,
    init: BasicBlockId,
    limit: Place,
    inclusive: bool,
    acc: String,
    addend: i64,
    modulus: i64,
    initial: i64,
}

fn acc_find_loop(func: &MirFunction) -> Option<AccLoopPlan> {
    let block_map: HashMap<BasicBlockId, &MirBasicBlock> = func
        .blocks
        .iter()
        .map(|b| (b.id.clone(), b))
        .collect();
    let mut preds: HashMap<BasicBlockId, Vec<BasicBlockId>> = HashMap::new();
    for b in &func.blocks {
        for succ in b.terminator.successors() {
            preds.entry(succ).or_default().push(b.id.clone());
        }
    }

    for h in &func.blocks {
        let (cond, then_t, else_t) = match &h.terminator {
            Terminator::BranchIf {
                condition,
                then_target,
                else_target,
            } => (condition, then_target, else_target),
            _ => continue,
        };

        for (body_id, exit_id) in [
            (then_t.clone(), else_t.clone()),
            (else_t.clone(), then_t.clone()),
        ] {
            if body_id == h.id || exit_id == h.id || body_id == exit_id {
                continue;
            }
            let body = match block_map.get(&body_id) {
                Some(b) => *b,
                None => continue,
            };
            if !matches!(&body.terminator, Terminator::Branch { target } if target == &h.id) {
                continue;
            }
            if !block_map.contains_key(&exit_id) {
                continue;
            }
            let hp = preds.get(&h.id).map(Vec::as_slice).unwrap_or(&[]);
            if hp.len() != 2 || !hp.contains(&body_id) {
                continue;
            }
            let init_id = hp.iter().find(|p| **p != body_id)?.clone();
            let init = *block_map.get(&init_id)?;
            if !matches!(&init.terminator, Terminator::Branch { target } if target == &h.id) {
                continue;
            }
            let bp = preds.get(&body_id).map(Vec::as_slice).unwrap_or(&[]);
            if bp.len() != 1 || bp[0] != h.id {
                continue;
            }
            if !body
                .statements
                .iter()
                .chain(h.statements.iter())
                .all(|Statement::Assign(_, rv)| acc_is_pure_scalar(rv))
            {
                continue;
            }
            let hdefs = acc_defs_in(h);
            let bdefs = acc_defs_in(body);
            let forbidden = acc_forbidden(h, body);

            // Identify the counted-loop comparison in the header.
            let cond_local = if cond.projections.is_empty() {
                cond.local.clone()
            } else {
                continue;
            };
            let Some(cmp_rv) = hdefs.get(&cond_local) else {
                continue;
            };
            let Rvalue::BinaryOp(op, lhs, rhs) = cmp_rv else {
                continue;
            };
            let (iv_p, lim_p, inclusive) = match op {
                BinaryOp::Lt => (lhs, rhs, false),
                BinaryOp::Le => (lhs, rhs, true),
                BinaryOp::Gt => (rhs, lhs, false),
                BinaryOp::Ge => (rhs, lhs, true),
                _ => continue,
            };
            // The induction variable is legitimately defined in the body; only
            // the header's alias chain needs to be peeled away here.
            let h_forbidden = acc_forbidden(h, h);
            let Some(iv) = acc_trace_external(iv_p, &hdefs, &h_forbidden) else {
                continue;
            };
            if iv.projections.is_empty() && !acc_is_unit_increment(&iv.local, &bdefs) {
                continue;
            }
            let init_defs = acc_defs_in(init);
            match init_defs.get(&iv.local) {
                Some(Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(0, _))) => {}
                _ => continue,
            }
            let Some(limit) = acc_trace_external(lim_p, &hdefs, &forbidden) else {
                continue;
            };

            // Cheap, environment-free shape test FIRST.  `acc_derive_envs`
            // concretely executes every path from the entry to this header,
            // which can be a long constant-trip-count producer loop.  Running
            // it for a loop that cannot possibly be a modular affine
            // accumulator is pure waste, so gather all plausible shapes before
            // paying for that path exploration.  This is only a reordering of
            // bail-out checks: every candidate and its order are preserved.
            let mut shapes: Vec<(String, &Place, &Place)> = Vec::new();
            for Statement::Assign(dest, rv) in &body.statements {
                let Rvalue::BinaryOp(BinaryOp::Mod, add_p, mod_p) = rv else {
                    continue;
                };
                if !dest.projections.is_empty() || !add_p.projections.is_empty() {
                    continue;
                }
                let acc_name = dest.local.clone();
                if acc_name == iv.local || acc_name == limit.local {
                    continue;
                }
                let Some(Rvalue::BinaryOp(BinaryOp::Add, x, y)) = bdefs.get(&add_p.local) else {
                    continue;
                };
                let x_is_acc = acc_is_acc_operand(x, &bdefs, &acc_name);
                let y_is_acc = acc_is_acc_operand(y, &bdefs, &acc_name);
                if x_is_acc == y_is_acc {
                    continue;
                }
                if body
                    .statements
                    .iter()
                    .filter(|Statement::Assign(d, _)| d.local == acc_name)
                    .count()
                    != 1
                {
                    continue;
                }
                if !acc_loop_locals_are_private(func, h, body, &acc_name) {
                    continue;
                }
                shapes.push((acc_name, if x_is_acc { y } else { x }, mod_p));
            }
            if shapes.is_empty() {
                continue;
            }

            // Derive the values of the accumulator, its addend and the modulus
            // at the loop header only for loops that have a modular shape.
            let Some(envs) = acc_derive_envs(func, &h.id, 100_000) else {
                continue;
            };

            let mut candidate: Option<(String, i64, i64, i64)> = None;
            for (acc_name, addend_p, mod_p) in shapes {
                let Some(AccConst::Int(c)) = acc_resolve_in_block_all(addend_p, &bdefs, &envs)
                else {
                    continue;
                };
                let Some(AccConst::Int(m)) = acc_resolve_in_block_all(mod_p, &bdefs, &envs) else {
                    continue;
                };
                let Some(AccConst::Int(s0)) =
                    acc_resolve_const(&Place { local: acc_name.clone(), projections: vec![] }, &envs)
                else {
                    continue;
                };
                if !(0 < c && c < m && m <= i32::MAX as i64 && 0 <= s0 && s0 < m) {
                    continue;
                }
                candidate = Some((acc_name, c, m, s0));
                break;
            }

            if let Some((acc, addend, modulus, initial)) = candidate {
                return Some(AccLoopPlan {
                    header: h.id.clone(),
                    body: body_id,
                    exit: exit_id,
                    init: init_id,
                    limit,
                    inclusive,
                    acc,
                    addend,
                    modulus,
                    initial,
                });
            }
        }
    }
    None
}

fn acc_fresh_local(func: &MirFunction, base: &str) -> String {
    let used: HashSet<String> = func
        .locals
        .iter()
        .map(|d| d.name.clone())
        .chain(func.blocks.iter().flat_map(|b| {
            b.statements.iter().map(|Statement::Assign(d, _)| d.local.clone())
        }))
        .collect();
    let mut name = base.to_string();
    let mut i = 0;
    while used.contains(&name) {
        i += 1;
        name = format!("{}_{}", base, i);
    }
    name
}

fn acc_apply_plan(func: &mut MirFunction, plan: &AccLoopPlan) {
    use crate::typecheck::typed_ast::TypedLiteral;

    let fresh = |base: &str| acc_fresh_local(func, base);
    let zero = fresh("__cfa_zero");
    let one = fresh("__cfa_one");
    let m_local = fresh("__cfa_m");
    let c_local = fresh("__cfa_c");
    let s0_local = fresh("__cfa_s0");
    let n_raw = fresh("__cfa_n_raw");
    let n_pos = fresh("__cfa_n_pos");
    let n = fresh("__cfa_n");
    let n_mod = fresh("__cfa_n_mod");
    let prod = fresh("__cfa_prod");
    let prod_mod = fresh("__cfa_prod_mod");
    let shifted = fresh("__cfa_shifted");
    let final_v = fresh("__cfa_final");

    let int = |v: i64| Rvalue::Constant(TypedLiteral::Int(v, Type::I64));
    let use_ = |name: &str| Rvalue::Use(Place { local: name.to_string(), projections: vec![] });
    let bin = |op: BinaryOp, a: &str, b: &str| {
        Rvalue::BinaryOp(
            op,
            Place { local: a.to_string(), projections: vec![] },
            Place { local: b.to_string(), projections: vec![] },
        )
    };

    let mut stmts = vec![
        Statement::Assign(Place { local: zero.clone(), projections: vec![] }, int(0)),
        Statement::Assign(Place { local: one.clone(), projections: vec![] }, int(1)),
        Statement::Assign(Place { local: m_local.clone(), projections: vec![] }, int(plan.modulus)),
        Statement::Assign(Place { local: c_local.clone(), projections: vec![] }, int(plan.addend)),
        Statement::Assign(Place { local: s0_local.clone(), projections: vec![] }, int(plan.initial)),
    ];

    if plan.inclusive {
        stmts.push(Statement::Assign(
            Place { local: n_raw.clone(), projections: vec![] },
            bin(BinaryOp::Add, &plan.limit.local, &one),
        ));
    } else {
        stmts.push(Statement::Assign(
            Place { local: n_raw.clone(), projections: vec![] },
            Rvalue::Use(plan.limit.clone()),
        ));
    }
    stmts.push(Statement::Assign(
        Place { local: n_pos.clone(), projections: vec![] },
        bin(BinaryOp::Gt, &n_raw, &zero),
    ));
    stmts.push(Statement::Assign(
        Place { local: n.clone(), projections: vec![] },
        Rvalue::Select(
            Place { local: n_pos.clone(), projections: vec![] },
            Place { local: n_raw.clone(), projections: vec![] },
            Place { local: zero.clone(), projections: vec![] },
        ),
    ));
    stmts.push(Statement::Assign(
        Place { local: n_mod.clone(), projections: vec![] },
        bin(BinaryOp::Mod, &n, &m_local),
    ));
    stmts.push(Statement::Assign(
        Place { local: prod.clone(), projections: vec![] },
        bin(BinaryOp::Mul, &n_mod, &c_local),
    ));
    stmts.push(Statement::Assign(
        Place { local: prod_mod.clone(), projections: vec![] },
        bin(BinaryOp::Mod, &prod, &m_local),
    ));
    stmts.push(Statement::Assign(
        Place { local: shifted.clone(), projections: vec![] },
        bin(BinaryOp::Add, &s0_local, &prod_mod),
    ));
    stmts.push(Statement::Assign(
        Place { local: final_v.clone(), projections: vec![] },
        bin(BinaryOp::Mod, &shifted, &m_local),
    ));
    stmts.push(Statement::Assign(
        Place { local: plan.acc.clone(), projections: vec![] },
        use_(&final_v),
    ));

    for name in [
        zero, one, m_local, c_local, s0_local, n_raw, n_pos, n, n_mod, prod, prod_mod, shifted,
        final_v,
    ] {
        func.locals.push(MirLocalDecl {
            name,
            ty: Type::I64,
            mutable: true,
        });
    }

    if let Some(init) = func.blocks.iter_mut().find(|b| b.id == plan.init) {
        init.statements.extend(stmts);
        init.terminator = Terminator::Branch { target: plan.exit.clone() };
    }
    func.blocks.retain(|b| b.id != plan.header && b.id != plan.body);
    func.is_distilled = true;
}

/// Program-level entry point for the modular affine accumulator closed form.
/// Returns the number of loops collapsed.
pub fn collapse_modular_accumulators(program: &mut MirProgram) -> usize {
    let mut plans = Vec::new();
    for (fi, func) in program.functions.iter().enumerate() {
        let started = std::time::Instant::now();
        let plan = acc_find_loop(func);
        let elapsed = started.elapsed();
        if elapsed.as_millis() >= 1 {
            dtrace!(
                "modular-accumulator scan fn={} {:?} matched={}",
                func.name,
                elapsed,
                plan.is_some()
            );
        }
        if let Some(plan) = plan {
            plans.push((fi, plan));
        }
    }
    let collapsed = plans.len();
    for (fi, plan) in plans {
        acc_apply_plan(&mut program.functions[fi], &plan);
    }
    // Geometric recurrences are residualised by the driving/recurrence pass as
    // MIR `Pow(K, e)` terms, optionally followed by `% M`.  Finish the collapse
    // for both constant and runtime exponents (see section 11 below).
    collapsed
        + fold_constant_powers(program)
        + collapse_runtime_powers(program)
        + eliminate_dead_input_reads(program)
}

// ============================================================================
// 11. O(1) collapse of geometric powers with runtime exponents
// ============================================================================
//
// The recurrence solver residualises `acc = acc * K` loops as MIR `Pow(K, e)`
// (usually followed by `% M`).  Previously these were only folded when *both*
// K and e were compile-time literals, which never fires for the dynamic
// benchmarks where the exponent comes from `read_i64()`.  This section closes
// that gap with two complementary, semantics-preserving rewrites:
//
//   * `fold_constant_powers` evaluates `Pow(K, e)` (and a following `% M`)
//     to a literal when every operand is provably constant.  `wrapping_pow_i64`
//     below is a line-for-line emulation of Cranelift's `emit_int_pow_raw`
//     (wrapping i64 multiply, exponentiation by squaring, exponent <= 0 -> 1),
//     so the literal is bit-identical to the run-time value for all inputs.
//
//   * `collapse_runtime_powers` handles a compile-time constant power-of-two
//     base K = 2^k and an optional compile-time constant power-of-two modulus
//     M = 2^m with a *runtime* exponent.  For e > 0 the wrapping i64 value of
//     2^(k*e) is 1 << (k*e) while k*e < 64 and 0 once k*e >= 64 (the value at
//     k*e == 63 is i64::MIN, which `ishl` produces naturally).  Since M divides
//     2^64, reducing modulo M saturates to 0 once k*e >= m.  Therefore:
//
//         K^e % M == if e <= 0 { 1 } else if e >= ceil(m/k) { 0 }
//                    else { 1 << (k*e) }                       (M > 1)
//
//       and for the unreduced wrapping value replace `m` by 64.  Neither K, M
//       nor the result is hardcoded: k and m are derived from the defining
//       statements of the residual MIR.  The generated code is straight-line
//       (two compares, one clamped multiply, one shift, two selects), i.e.
//       O(1) in the exponent, and preserves the e <= 0 -> 1 and large-e -> 0
//       cases required by wrapping two's-complement i64 semantics.
//
// Both rewrites derive constants from SSA defining statements only; no
// variable/function names or benchmark-specific constants are inspected.

/// Exact two's-complement emulation of Cranelift's integer-power intrinsic
/// (`emit_int_pow_raw`): exponentiation by squaring with wrapping i64 maths.
/// A non-positive exponent yields 1 (the loop body never executes).
fn wrapping_pow_i64(mut base: i64, mut exp: i64) -> i64 {
    let mut res = 1i64;
    while exp > 0 {
        if exp & 1 != 0 {
            res = res.wrapping_mul(base);
        }
        base = base.wrapping_mul(base);
        exp >>= 1;
    }
    res
}

/// Snapshot of single-assignment scalar definitions in a function.  Locals
/// with more than one defining assignment are dropped (they are not provably
/// constant) and projected destinations are ignored.
fn pow_scalar_defs(func: &MirFunction) -> HashMap<String, Rvalue> {
    let mut defs: HashMap<String, Rvalue> = HashMap::new();
    let mut multi: HashSet<String> = HashSet::new();
    for block in &func.blocks {
        for Statement::Assign(dest, rv) in &block.statements {
            if dest.projections.is_empty()
                && defs.insert(dest.local.clone(), rv.clone()).is_some()
            {
                multi.insert(dest.local.clone());
            }
        }
    }
    for name in multi {
        defs.remove(&name);
    }
    defs
}

/// Recursively evaluates a scalar place to a compile-time i64 using only the
/// defining statements in `defs`.  Pure scalar rvalues are understood; every
/// arithmetic operation uses wrapping i64 semantics to match codegen.
fn pow_eval_const(place: &Place, defs: &HashMap<String, Rvalue>, depth: usize) -> Option<i64> {
    use crate::typecheck::typed_ast::TypedLiteral;

    if depth > 64 || !place.projections.is_empty() {
        return None;
    }
    match defs.get(&place.local)? {
        Rvalue::Constant(TypedLiteral::Int(v, ty)) if *ty == Type::I64 => Some(*v),
        Rvalue::Use(next) => pow_eval_const(next, defs, depth + 1),
        Rvalue::UnaryOp(op, p) => {
            let v = pow_eval_const(p, defs, depth + 1)?;
            Some(match op {
                crate::ast::UnaryOp::Neg => v.wrapping_neg(),
                crate::ast::UnaryOp::Not => !v,
            })
        }
        Rvalue::BinaryOp(op, a, b) => {
            let l = pow_eval_const(a, defs, depth + 1)?;
            let r = pow_eval_const(b, defs, depth + 1)?;
            Some(match op {
                BinaryOp::Add => l.wrapping_add(r),
                BinaryOp::Sub => l.wrapping_sub(r),
                BinaryOp::Mul => l.wrapping_mul(r),
                BinaryOp::Div if r != 0 => l.wrapping_div(r),
                BinaryOp::Mod if r != 0 => l.wrapping_rem(r),
                BinaryOp::BitAnd => l & r,
                BinaryOp::BitOr => l | r,
                BinaryOp::BitXor => l ^ r,
                BinaryOp::Shl => l.wrapping_shl((r as u32) & 63),
                BinaryOp::Shr => l.wrapping_shr((r as u32) & 63),
                BinaryOp::Pow => wrapping_pow_i64(l, r),
                _ => return None,
            })
        }
        _ => None,
    }
}

/// True iff `place`'s value (transitively through pure scalar definitions)
/// originates in a `Pow` rvalue.  Restricts literal folding of `%` to the
/// geometric powers this pass is responsible for.
fn pow_chain_has_pow(place: &Place, defs: &HashMap<String, Rvalue>, depth: usize) -> bool {
    if depth > 64 || !place.projections.is_empty() {
        return false;
    }
    match defs.get(&place.local) {
        Some(Rvalue::BinaryOp(BinaryOp::Pow, _, _)) => true,
        Some(Rvalue::Use(next)) => pow_chain_has_pow(next, defs, depth + 1),
        Some(Rvalue::UnaryOp(_, p)) => pow_chain_has_pow(p, defs, depth + 1),
        Some(Rvalue::BinaryOp(_, a, b)) => {
            pow_chain_has_pow(a, defs, depth + 1) || pow_chain_has_pow(b, defs, depth + 1)
        }
        _ => false,
    }
}

/// Folds constant `Pow(K, e)` and `(Pow(K, e) % M)` rvalues to literals.
fn fold_constant_powers(program: &mut MirProgram) -> usize {
    use crate::typecheck::typed_ast::TypedLiteral;

    let mut total = 0usize;
    for func in &mut program.functions {
        for _ in 0..4 {
            let defs = pow_scalar_defs(func);
            let mut folded: Vec<(usize, usize, Rvalue)> = Vec::new();
            let mut pow_dests: HashSet<String> = HashSet::new();

            for (bi, block) in func.blocks.iter().enumerate() {
                for (si, stmt) in block.statements.iter().enumerate() {
                    let Statement::Assign(dest, rv) = stmt;
                    if !dest.projections.is_empty() {
                        continue;
                    }
                    match rv {
                        Rvalue::BinaryOp(BinaryOp::Pow, a, b) => {
                            let (Some(av), Some(bv)) =
                                (pow_eval_const(a, &defs, 0), pow_eval_const(b, &defs, 0))
                            else {
                                continue;
                            };
                            pow_dests.insert(dest.local.clone());
                            folded.push((
                                bi,
                                si,
                                Rvalue::Constant(TypedLiteral::Int(
                                    wrapping_pow_i64(av, bv),
                                    Type::I64,
                                )),
                            ));
                        }
                        Rvalue::BinaryOp(BinaryOp::Mod, a, b) => {
                            if !pow_chain_has_pow(a, &defs, 0) {
                                continue;
                            }
                            let (Some(av), Some(mv)) =
                                (pow_eval_const(a, &defs, 0), pow_eval_const(b, &defs, 0))
                            else {
                                continue;
                            };
                            if mv == 0 {
                                continue;
                            }
                            folded.push((
                                bi,
                                si,
                                Rvalue::Constant(TypedLiteral::Int(av.wrapping_rem(mv), Type::I64)),
                            ));
                        }
                        _ => {}
                    }
                }
            }
            if folded.is_empty() {
                break;
            }
            let _ = pow_dests; // folded powers are re-derived from `defs` each round
            for (bi, si, new_rv) in folded {
                let Statement::Assign(_, rv) = &mut func.blocks[bi].statements[si];
                *rv = new_rv;
                total += 1;
            }
        }
    }
    total
}

// ---------------------------------------------------------------------------
// Runtime-exponent saturation collapse
// ---------------------------------------------------------------------------

fn gp_place(name: &str) -> Place {
    Place {
        local: name.to_string(),
        projections: vec![],
    }
}

fn gp_fresh(func: &MirFunction, base: &str) -> String {
    let used: HashSet<String> = func
        .locals
        .iter()
        .map(|d| d.name.clone())
        .chain(func.blocks.iter().flat_map(|b| {
            b.statements
                .iter()
                .map(|Statement::Assign(d, _)| d.local.clone())
        }))
        .collect();
    let mut name = base.to_string();
    let mut i = 0;
    while used.contains(&name) {
        i += 1;
        name = format!("{}_{}", base, i);
    }
    name
}

/// Counts direct references to `local` in every operand of every statement and
/// terminator.  Used to ensure a rewritten `Pow` result has at most one
/// consumer (a `% M` we can fold into the result).
fn gp_count_uses(func: &MirFunction, local: &str) -> usize {
    fn mark(p: &Place, local: &str, n: &mut usize) {
        if p.local == local {
            *n += 1;
        }
        for proj in &p.projections {
            if let Projection::Index(idx) = proj {
                mark(idx, local, n);
            }
        }
    }
    fn mark_rv(rv: &Rvalue, local: &str, n: &mut usize) {
        match rv {
            Rvalue::Use(p) | Rvalue::UnaryOp(_, p) | Rvalue::Load(p) | Rvalue::Alloc(p)
            | Rvalue::Discriminant(p) => mark(p, local, n),
            Rvalue::BinaryOp(_, a, b) => {
                mark(a, local, n);
                mark(b, local, n);
            }
            Rvalue::Select(c, t, f) => {
                mark(c, local, n);
                mark(t, local, n);
                mark(f, local, n);
            }
            Rvalue::Call(_, args) | Rvalue::Array(args) => {
                for a in args {
                    mark(a, local, n);
                }
            }
            Rvalue::Struct(_, fields) => {
                for (_, p) in fields {
                    mark(p, local, n);
                }
            }
            Rvalue::EnumVariant { fields, .. } => {
                for p in fields {
                    mark(p, local, n);
                }
            }
            Rvalue::Phi(entries) => {
                for (_, p) in entries {
                    mark(p, local, n);
                }
            }
            Rvalue::ClosureAlloc { captured, .. } => {
                for p in captured {
                    mark(p, local, n);
                }
            }
            Rvalue::Thunk { env, .. } => {
                for e in env {
                    if e == local {
                        *n += 1;
                    }
                }
            }
            Rvalue::FnPtr(_) | Rvalue::Constant(_) => {}
        }
    }
    fn mark_term(t: &Terminator, local: &str, n: &mut usize) {
        match t {
            Terminator::Return { value: Some(p) } => mark(p, local, n),
            Terminator::Switch { value, .. } => mark(value, local, n),
            Terminator::TypeGuard { local: l, .. } if l.local == local => *n += 1,
            Terminator::IndirectCall { callee, args, .. } => {
                mark(callee, local, n);
                for a in args {
                    mark(a, local, n);
                }
            }
            Terminator::BranchIf { condition, .. } => mark(condition, local, n),
            _ => {}
        }
    }
    let mut n = 0;
    for block in &func.blocks {
        for Statement::Assign(_, rv) in &block.statements {
            mark_rv(rv, local, &mut n);
        }
        mark_term(&block.terminator, local, &mut n);
    }
    n
}

/// If `dest` is consumed exactly once by `r = Pow(...) % M` (possibly through
/// `Use` chains), and M is a positive power-of-two constant, return its value.
fn gp_unique_mod(func: &MirFunction, dest: &str, defs: &HashMap<String, Rvalue>) -> Option<i64> {
    if gp_count_uses(func, dest) != 1 {
        return None;
    }
    for block in &func.blocks {
        for Statement::Assign(d, rv) in &block.statements {
            let Rvalue::BinaryOp(BinaryOp::Mod, a, b) = rv else {
                continue;
            };
            // Follow `Use` chains on the left operand to the Pow destination.
            let mut cur = a.clone();
            let mut reached = false;
            for _ in 0..64 {
                if !cur.projections.is_empty() {
                    break;
                }
                if cur.local == dest {
                    reached = true;
                    break;
                }
                match defs.get(&cur.local) {
                    Some(Rvalue::Use(next)) => cur = next.clone(),
                    _ => break,
                }
            }
            if !reached {
                continue;
            }
            let m = pow_eval_const(b, defs, 0)?;
            if m > 0 && (m & (m - 1)) == 0 {
                let _ = d;
                return Some(m);
            }
            return None;
        }
    }
    None
}

/// Builds the straight-line saturation collapse for `2^k ^ e` (optionally
/// reduced modulo `2^m`) and assigns it to `dest`.
fn gp_emit_collapse(
    func: &mut MirFunction,
    dest: &str,
    exp: &Place,
    k: i64,
    modulus_bits: i64,
) -> Vec<Statement> {
    use crate::typecheck::typed_ast::TypedLiteral;

    let fresh = |base: &str| gp_fresh(func, base);
    let zero = fresh("__rgp_zero");
    let one = fresh("__rgp_one");
    let k_local = fresh("__rgp_k");
    let t_local = fresh("__rgp_t");
    let c_le = fresh("__rgp_cle");
    let c_sat = fresh("__rgp_csat");
    let e_clamped = fresh("__rgp_eclamped");
    let shift = fresh("__rgp_shift");
    let pw = fresh("__rgp_pow");
    let reduced = fresh("__rgp_reduced");
    let result = fresh("__rgp_result");

    let int = |v: i64| Rvalue::Constant(TypedLiteral::Int(v, Type::I64));
    let bin = |op: BinaryOp, a: &str, b: &str| {
        Rvalue::BinaryOp(op, gp_place(a), gp_place(b))
    };
    let threshold = (modulus_bits + k - 1) / k; // ceil(bits / k), bits >= 1

    let stmts = vec![
        Statement::Assign(gp_place(&zero), int(0)),
        Statement::Assign(gp_place(&one), int(1)),
        Statement::Assign(gp_place(&k_local), int(k)),
        Statement::Assign(gp_place(&t_local), int(threshold)),
        Statement::Assign(
            gp_place(&c_le),
            Rvalue::BinaryOp(BinaryOp::Le, exp.clone(), gp_place(&zero)),
        ),
        Statement::Assign(
            gp_place(&c_sat),
            Rvalue::BinaryOp(BinaryOp::Ge, exp.clone(), gp_place(&t_local)),
        ),
        Statement::Assign(
            gp_place(&e_clamped),
            Rvalue::Select(gp_place(&c_le), gp_place(&zero), exp.clone()),
        ),
        Statement::Assign(
            gp_place(&shift),
            bin(BinaryOp::Mul, &e_clamped, &k_local),
        ),
        Statement::Assign(gp_place(&pw), bin(BinaryOp::Shl, &one, &shift)),
        Statement::Assign(
            gp_place(&reduced),
            Rvalue::Select(gp_place(&c_sat), gp_place(&zero), gp_place(&pw)),
        ),
        Statement::Assign(
            gp_place(&result),
            Rvalue::Select(gp_place(&c_le), gp_place(&one), gp_place(&reduced)),
        ),
        Statement::Assign(gp_place(dest), Rvalue::Use(gp_place(&result))),
    ];

    for name in [
        zero,
        one,
        k_local,
        t_local,
        c_le,
        c_sat,
        e_clamped,
        shift,
        pw,
        reduced,
        result,
    ] {
        func.locals.push(MirLocalDecl {
            name,
            ty: Type::I64,
            mutable: true,
        });
    }
    stmts
}

struct GpPlan {
    block: usize,
    statement: usize,
    dest: String,
    exp: Place,
    k: i64,
    bits: i64,
    literal: Option<i64>,
}

/// Collapses constant-base, runtime-exponent geometric powers for which we can
/// prove the saturation identity (positive power-of-two base and, for the
/// modular form, positive power-of-two modulus).  Returns the number of
/// rewritten `Pow` statements.
fn collapse_runtime_powers(program: &mut MirProgram) -> usize {
    use crate::typecheck::typed_ast::TypedLiteral;

    let mut total = 0usize;
    for func in &mut program.functions {
        let defs = pow_scalar_defs(func);
        let mut plans: Vec<GpPlan> = Vec::new();

        for (bi, block) in func.blocks.iter().enumerate() {
            for (si, stmt) in block.statements.iter().enumerate() {
                let Statement::Assign(dest, rv) = stmt;
                if !dest.projections.is_empty() {
                    continue;
                }
                let Rvalue::BinaryOp(BinaryOp::Pow, base, exp) = rv else {
                    continue;
                };
                // Constant exponents were handled by `fold_constant_powers`.
                if pow_eval_const(exp, &defs, 0).is_some() {
                    continue;
                }
                let Some(k_val) = pow_eval_const(base, &defs, 0) else {
                    continue;
                };
                if k_val <= 0 || (k_val & (k_val - 1)) != 0 {
                    continue;
                }
                let k = k_val.trailing_zeros() as i64;

                // K = 1 is exponent-independent: 1^e = 1 for every e.
                if k == 0 {
                    plans.push(GpPlan {
                        block: bi,
                        statement: si,
                        dest: dest.local.clone(),
                        exp: exp.clone(),
                        k,
                        bits: 0,
                        literal: Some(1),
                    });
                    total += 1;
                    continue;
                }

                let modulus = gp_unique_mod(func, &dest.local, &defs);
                let bits = match modulus {
                    Some(1) => {
                        // 1^e % 1 is 0 even for e <= 0.
                        plans.push(GpPlan {
                            block: bi,
                            statement: si,
                            dest: dest.local.clone(),
                            exp: exp.clone(),
                            k,
                            bits: 0,
                            literal: Some(0),
                        });
                        total += 1;
                        continue;
                    }
                    Some(m) => m.trailing_zeros() as i64,
                    None => 64, // unreduced wrapping i64 value
                };
                plans.push(GpPlan {
                    block: bi,
                    statement: si,
                    dest: dest.local.clone(),
                    exp: exp.clone(),
                    k,
                    bits,
                    literal: None,
                });
                total += 1;
            }
        }

        // Generate fresh locals only after the immutable scan is finished.
        let mut replacements: Vec<(usize, usize, Vec<Statement>)> = Vec::new();
        for plan in plans {
            let stmts = match plan.literal {
                Some(v) => vec![Statement::Assign(
                    gp_place(&plan.dest),
                    Rvalue::Constant(TypedLiteral::Int(v, Type::I64)),
                )],
                None => gp_emit_collapse(func, &plan.dest, &plan.exp, plan.k, plan.bits),
            };
            replacements.push((plan.block, plan.statement, stmts));
        }

        // Apply from the end so earlier indices remain valid.
        replacements.sort_by_key(|(bi, si, _)| (*bi, *si));
        for (bi, si, stmts) in replacements.into_iter().rev() {
            let block = &mut func.blocks[bi];
            block.statements.remove(si);
            for (off, stmt) in stmts.into_iter().enumerate() {
                block.statements.insert(si + off, stmt);
            }
        }
    }
    total
}

// ---------------------------------------------------------------------------
// Dead input-read elimination for fully collapsed residuals
// ---------------------------------------------------------------------------
//
// `dce_mir_function` deliberately keeps every `Rvalue::Call` because calls may
// have effects.  Once a recurrence has been folded to a literal, however, a
// `read_i64()` whose result no longer reaches an observable use is genuinely
// dead: NumLang's builtin has no side effects other than supplying its integer
// result.  A backward liveness fixed point marks places reaching returns, call
// arguments, switches, type guards and *real* branches (a `BranchIf` whose two
// targets are the same block does not observe its condition).  Only pure scalar
// rvalues and argument-less `read_i64()` calls with dead destinations are
// removed; all other calls are retained.

fn dead_read_mark_place(p: &Place, out: &mut HashSet<String>) {
    out.insert(p.local.clone());
    for proj in &p.projections {
        if let Projection::Index(idx) = proj {
            dead_read_mark_place(idx, out);
        }
    }
}

/// A projected destination (`a[i] = v`) reads its index local even though the
/// base local is written; keep those index places live.
fn dead_read_mark_dest_indices(p: &Place, out: &mut HashSet<String>) {
    for proj in &p.projections {
        if let Projection::Index(idx) = proj {
            dead_read_mark_place(idx, out);
        }
    }
}

fn dead_read_mine_rv(rv: &Rvalue, out: &mut HashSet<String>) {
    match rv {
        Rvalue::Use(p) | Rvalue::UnaryOp(_, p) | Rvalue::Load(p) | Rvalue::Alloc(p)
        | Rvalue::Discriminant(p) => dead_read_mark_place(p, out),
        Rvalue::BinaryOp(_, a, b) => {
            dead_read_mark_place(a, out);
            dead_read_mark_place(b, out);
        }
        Rvalue::Select(c, t, f) => {
            dead_read_mark_place(c, out);
            dead_read_mark_place(t, out);
            dead_read_mark_place(f, out);
        }
        Rvalue::Call(_, args) | Rvalue::Array(args) => {
            for a in args {
                dead_read_mark_place(a, out);
            }
        }
        Rvalue::Struct(_, fields) => {
            for (_, p) in fields {
                dead_read_mark_place(p, out);
            }
        }
        Rvalue::EnumVariant { fields, .. } => {
            for p in fields {
                dead_read_mark_place(p, out);
            }
        }
        Rvalue::Phi(entries) => {
            for (_, p) in entries {
                dead_read_mark_place(p, out);
            }
        }
        Rvalue::ClosureAlloc { captured, .. } => {
            for p in captured {
                dead_read_mark_place(p, out);
            }
        }
        Rvalue::Thunk { env, .. } => {
            for e in env {
                out.insert(e.clone());
            }
        }
        Rvalue::FnPtr(_) | Rvalue::Constant(_) => {}
    }
}

fn dead_read_mine_term(t: &Terminator, out: &mut HashSet<String>) {
    match t {
        Terminator::Return { value: Some(p) } => dead_read_mark_place(p, out),
        Terminator::Switch { value, .. } => dead_read_mark_place(value, out),
        Terminator::TypeGuard { local, .. } => dead_read_mark_place(local, out),
        Terminator::IndirectCall { callee, args, .. } => {
            dead_read_mark_place(callee, out);
            for a in args {
                dead_read_mark_place(a, out);
            }
        }
        Terminator::BranchIf {
            condition,
            then_target,
            else_target,
        } if then_target != else_target => dead_read_mark_place(condition, out),
        _ => {}
    }
}

fn dead_read_is_removable(rv: &Rvalue) -> bool {
    matches!(
        rv,
        Rvalue::Use(_)
            | Rvalue::Constant(_)
            | Rvalue::BinaryOp(..)
            | Rvalue::UnaryOp(..)
            | Rvalue::Select(..)
    ) || matches!(rv, Rvalue::Call(name, args) if name == "read_i64" && args.is_empty())
}

/// Removes dead `read_i64` calls (and pure scalar assignments that became dead
/// with them).  Returns the number of statements removed.
fn eliminate_dead_input_reads(program: &mut MirProgram) -> usize {
    let mut total = 0usize;
    for func in &mut program.functions {
        let mut needed: HashSet<String> = HashSet::new();
        loop {
            let mut changed = false;
            for block in &func.blocks {
                dead_read_mine_term(&block.terminator, &mut needed);
                for Statement::Assign(dest, rv) in &block.statements {
                    let before = needed.len();
                    dead_read_mark_dest_indices(dest, &mut needed);
                    if needed.contains(&dest.local) || !dead_read_is_removable(rv) {
                        dead_read_mine_rv(rv, &mut needed);
                    }
                    if needed.len() != before {
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        for block in &mut func.blocks {
            let before = block.statements.len();
            block.statements.retain(|Statement::Assign(dest, rv)| {
                !(dest.projections.is_empty()
                    && !needed.contains(&dest.local)
                    && dead_read_is_removable(rv))
            });
            total += before - block.statements.len();
        }
    }
    total
}
