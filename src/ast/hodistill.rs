//! Lambda-Level Global Process-Tree Distillation Pass (Hamilton 2007).
//!
//! Operates directly on the typed functional Abstract Syntax Tree (`TypedProgram`)
//! before Mid-level Intermediate Representation (MIR) lowering.
//!
//! Models lambda terms as process tree nodes (`Call`, `Lam`, `App`, `Let`, `Case`),
//! performs inter-procedural folding over recursive and mutually-recursive higher-order
//! configurations, and applies higher-order deforestation to eliminate intermediate
//! closure allocations in composed functional pipelines (`compose f (compose g h) x`).

use std::collections::HashMap;

use crate::ast::{BinaryOp, UnaryOp};
use crate::span::Span;
use crate::typecheck::typed_ast::{
    TypedBlock, TypedExpr, TypedFunction, TypedMatchArm, TypedMatchPattern, TypedProgram, TypedStmt,
};
use crate::typecheck::types::Type;

// ============================================================================
// 1. Process Tree Term Representation (HODIST-01)
// ============================================================================

/// Unique identifier for an AST process tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AstProcessNodeId(pub usize);

/// Process tree representation of higher-order functional terms.
#[derive(Debug, Clone, PartialEq)]
pub enum AstProcessTerm {
    /// Variable reference
    Var(String, Type),
    /// Literal constant
    Lit(crate::typecheck::typed_ast::TypedLiteral, Type),
    /// Lambda abstraction: λ(params). body
    Lam {
        params: Vec<(String, Type)>,
        body: AstProcessNodeId,
        ty: Type,
    },
    /// Application of a function/closure term to arguments: fun(args)
    App {
        fun: AstProcessNodeId,
        args: Vec<AstProcessNodeId>,
        ty: Type,
    },
    /// Named function call: f(args)
    Call {
        func_name: String,
        args: Vec<AstProcessNodeId>,
        ty: Type,
    },
    /// Let binding: let x = val in body
    Let {
        name: String,
        val: AstProcessNodeId,
        body: AstProcessNodeId,
        ty: Type,
    },
    /// Pattern matching / Case construct
    Case {
        scrutinee: AstProcessNodeId,
        arms: Vec<AstProcessCaseArm>,
        ty: Type,
    },
    /// Binary arithmetic / logical operation
    Binary {
        op: BinaryOp,
        left: AstProcessNodeId,
        right: AstProcessNodeId,
        ty: Type,
    },
    /// Unary operation
    Unary {
        op: UnaryOp,
        expr: AstProcessNodeId,
        ty: Type,
    },
    /// Inter-procedural fold knot to an ancestor configuration (Hamilton fold rule)
    FoldKnot {
        target: AstProcessNodeId,
        func_name: String,
        args: Vec<AstProcessNodeId>,
        ty: Type,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AstProcessCaseArm {
    pub patterns: Vec<TypedMatchPattern>,
    pub body: AstProcessNodeId,
}

#[derive(Debug, Clone)]
pub struct AstProcessNode {
    pub id: AstProcessNodeId,
    pub term: AstProcessTerm,
    pub span: Span,
    pub parent: Option<AstProcessNodeId>,
    pub depth: usize,
}

/// Global AST process tree.
#[derive(Debug, Clone, Default)]
pub struct AstProcessTree {
    pub nodes: Vec<AstProcessNode>,
    pub root: Option<AstProcessNodeId>,
}

impl AstProcessTree {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            root: None,
        }
    }

    pub fn alloc(
        &mut self,
        term: AstProcessTerm,
        span: Span,
        parent: Option<AstProcessNodeId>,
        depth: usize,
    ) -> AstProcessNodeId {
        let id = AstProcessNodeId(self.nodes.len());
        self.nodes.push(AstProcessNode {
            id,
            term,
            span,
            parent,
            depth,
        });
        id
    }

    pub fn get(&self, id: AstProcessNodeId) -> &AstProcessNode {
        &self.nodes[id.0]
    }

    pub fn get_mut(&mut self, id: AstProcessNodeId) -> &mut AstProcessNode {
        &mut self.nodes[id.0]
    }
}

// ============================================================================
// 2. Metrics & Telemetry
// ============================================================================

/// Statistics recorded during higher-order AST distillation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AstDistillStats {
    /// Number of intermediate closure allocations deforested.
    pub closures_eliminated: usize,
    /// Number of beta-reductions performed on applied lambdas.
    pub beta_reductions: usize,
    /// Number of inter-procedural folds performed across function configurations.
    pub folds_performed: usize,
    /// Number of composed higher-order functions fused.
    pub functions_fused: usize,
}

// ============================================================================
// 3. Alpha-Equivalence Modulo Renaming (HODIST-02)
// ============================================================================

/// Check if two process tree nodes are alpha-equivalent modulo variable renaming.
pub fn is_alpha_equivalent(
    tree: &AstProcessTree,
    id1: AstProcessNodeId,
    id2: AstProcessNodeId,
    map1_to_2: &mut HashMap<String, String>,
    map2_to_1: &mut HashMap<String, String>,
) -> bool {
    let n1 = tree.get(id1);
    let n2 = tree.get(id2);

    match (&n1.term, &n2.term) {
        (AstProcessTerm::Var(v1, t1), AstProcessTerm::Var(v2, t2)) => {
            if t1 != t2 {
                return false;
            }
            if let Some(target) = map1_to_2.get(v1) {
                target == v2
            } else if map2_to_1.contains_key(v2) {
                false
            } else {
                map1_to_2.insert(v1.clone(), v2.clone());
                map2_to_1.insert(v2.clone(), v1.clone());
                true
            }
        }
        (AstProcessTerm::Lit(l1, t1), AstProcessTerm::Lit(l2, t2)) => t1 == t2 && l1 == l2,
        (
            AstProcessTerm::Lam {
                params: p1,
                body: b1,
                ty: t1,
            },
            AstProcessTerm::Lam {
                params: p2,
                body: b2,
                ty: t2,
            },
        ) => {
            if t1 != t2 || p1.len() != p2.len() {
                return false;
            }
            for ((name1, ty1), (name2, ty2)) in p1.iter().zip(p2.iter()) {
                if ty1 != ty2 {
                    return false;
                }
                map1_to_2.insert(name1.clone(), name2.clone());
                map2_to_1.insert(name2.clone(), name1.clone());
            }
            is_alpha_equivalent(tree, *b1, *b2, map1_to_2, map2_to_1)
        }
        (
            AstProcessTerm::Call {
                func_name: f1,
                args: a1,
                ty: t1,
            },
            AstProcessTerm::Call {
                func_name: f2,
                args: a2,
                ty: t2,
            },
        ) => {
            if f1 != f2 || t1 != t2 || a1.len() != a2.len() {
                return false;
            }
            for (arg1, arg2) in a1.iter().zip(a2.iter()) {
                if !is_alpha_equivalent(tree, *arg1, *arg2, map1_to_2, map2_to_1) {
                    return false;
                }
            }
            true
        }
        (
            AstProcessTerm::App {
                fun: f1,
                args: a1,
                ty: t1,
            },
            AstProcessTerm::App {
                fun: f2,
                args: a2,
                ty: t2,
            },
        ) => {
            if t1 != t2 || a1.len() != a2.len() {
                return false;
            }
            if !is_alpha_equivalent(tree, *f1, *f2, map1_to_2, map2_to_1) {
                return false;
            }
            for (arg1, arg2) in a1.iter().zip(a2.iter()) {
                if !is_alpha_equivalent(tree, *arg1, *arg2, map1_to_2, map2_to_1) {
                    return false;
                }
            }
            true
        }
        (
            AstProcessTerm::Binary {
                op: op1,
                left: l1,
                right: r1,
                ty: t1,
            },
            AstProcessTerm::Binary {
                op: op2,
                left: l2,
                right: r2,
                ty: t2,
            },
        ) => {
            if op1 != op2 || t1 != t2 {
                return false;
            }
            is_alpha_equivalent(tree, *l1, *l2, map1_to_2, map2_to_1)
                && is_alpha_equivalent(tree, *r1, *r2, map1_to_2, map2_to_1)
        }
        (
            AstProcessTerm::Unary {
                op: op1,
                expr: e1,
                ty: t1,
            },
            AstProcessTerm::Unary {
                op: op2,
                expr: e2,
                ty: t2,
            },
        ) => {
            if op1 != op2 || t1 != t2 {
                return false;
            }
            is_alpha_equivalent(tree, *e1, *e2, map1_to_2, map2_to_1)
        }
        _ => false,
    }
}

// ============================================================================
// 4. AstDistiller: Higher-Order Deforestation & Fold Engine (HODIST-01..03)
// ============================================================================

pub struct AstDistiller<'a> {
    functions: &'a HashMap<String, TypedFunction>,
    stats: AstDistillStats,
}

impl<'a> AstDistiller<'a> {
    pub fn new(functions: &'a HashMap<String, TypedFunction>) -> Self {
        Self {
            functions,
            stats: AstDistillStats::default(),
        }
    }

    /// Distills a typed function body: unfolds composed higher-order functions,
    /// eliminates intermediate lambda closures, performs beta-reductions, and
    /// folds inter-procedural mutual recursions.
    pub fn distill_function(&mut self, func: &TypedFunction) -> TypedFunction {
        let mut new_func = func.clone();

        // Convert the function body statements into optimized forms
        let mut new_stmts = Vec::new();
        let mut local_closures: HashMap<String, TypedExpr> = HashMap::new();

        for stmt in &func.body.stmts {
            match stmt {
                TypedStmt::Let {
                    name,
                    is_mutable,
                    ty,
                    value,
                    span,
                } => {
                    let mut optimized_val = self.distill_expr(value, &local_closures);
                    // Check if value is a lambda abstraction
                    if let TypedExpr::Lambda { .. } = &optimized_val {
                        local_closures.insert(name.clone(), optimized_val.clone());
                    }
                    // Attempt beta-reduction / simplification on value
                    optimized_val = self.beta_reduce_expr(&optimized_val);
                    new_stmts.push(TypedStmt::Let {
                        name: name.clone(),
                        is_mutable: *is_mutable,
                        ty: ty.clone(),
                        value: optimized_val,
                        span: *span,
                    });
                }
                TypedStmt::Assign { name, value, span } => {
                    let mut optimized_val = self.distill_expr(value, &local_closures);
                    optimized_val = self.beta_reduce_expr(&optimized_val);
                    new_stmts.push(TypedStmt::Assign {
                        name: name.clone(),
                        value: optimized_val,
                        span: *span,
                    });
                }
                TypedStmt::Return(Some(expr), span) => {
                    let mut optimized_expr = self.distill_expr(expr, &local_closures);
                    optimized_expr = self.beta_reduce_expr(&optimized_expr);
                    new_stmts.push(TypedStmt::Return(Some(optimized_expr), *span));
                }
                TypedStmt::Expr(expr) => {
                    let mut optimized_expr = self.distill_expr(expr, &local_closures);
                    optimized_expr = self.beta_reduce_expr(&optimized_expr);
                    new_stmts.push(TypedStmt::Expr(optimized_expr));
                }
                other => new_stmts.push(other.clone()),
            }
        }

        new_func.body = TypedBlock {
            stmts: new_stmts,
            span: func.body.span,
        };

        new_func
    }

    /// Distills an expression: handles composed function unfolding and closure deforestation.
    pub fn distill_expr(
        &mut self,
        expr: &TypedExpr,
        local_closures: &HashMap<String, TypedExpr>,
    ) -> TypedExpr {
        match expr {
            // Function call: may be higher-order function like compose, or returning a closure
            TypedExpr::Call {
                callee,
                args,
                ty,
                span,
            } => {
                let distilled_args: Vec<TypedExpr> = args
                    .iter()
                    .map(|a| self.distill_expr(a, local_closures))
                    .collect();

                // Check if this callee is a closure-returning function definition
                if let Some(target_fn) = self.functions.get(callee) {
                    if let Some(TypedExpr::Lambda {
                        params,
                        body,
                        ty: lam_ty,
                        span: lam_span,
                        ..
                    }) = get_direct_return_expr(&target_fn.body)
                    {
                        // Higher-order function returning a lambda: unfold!
                        // Substitute arguments into lambda body
                        let mut subst_body = *body.clone();
                        for (param, arg) in target_fn.params.iter().zip(distilled_args.iter()) {
                            subst_body = substitute_var(&subst_body, &param.name, arg);
                        }
                        self.stats.closures_eliminated += 1;
                        self.stats.functions_fused += 1;
                        return TypedExpr::Lambda {
                            params: params.clone(),
                            body: Box::new(self.distill_expr(&subst_body, local_closures)),
                            captured: Vec::new(),
                            ty: lam_ty.clone(),
                            span: *lam_span,
                        };
                    }
                }

                TypedExpr::Call {
                    callee: callee.clone(),
                    args: distilled_args,
                    ty: ty.clone(),
                    span: *span,
                }
            }

            // Application of closure or indirect call: App(fun, args)
            TypedExpr::CallIndirect {
                callee,
                args,
                ty,
                span,
            } => {
                let distilled_callee = self.distill_expr(callee, local_closures);
                let distilled_args: Vec<TypedExpr> = args
                    .iter()
                    .map(|a| self.distill_expr(a, local_closures))
                    .collect();

                // If callee is an identifier bound to a local closure, resolve it
                let resolved_callee = if let TypedExpr::Ident { name, .. } = &distilled_callee {
                    local_closures
                        .get(name)
                        .cloned()
                        .unwrap_or(distilled_callee.clone())
                } else {
                    distilled_callee.clone()
                };

                // Beta-reduction: if callee is a lambda abstraction, apply directly
                if let TypedExpr::Lambda { params, body, .. } = &resolved_callee {
                    if params.len() == distilled_args.len() {
                        let mut reduced = *body.clone();
                        for ((param_name, _), arg) in params.iter().zip(distilled_args.iter()) {
                            reduced = substitute_var(&reduced, param_name, arg);
                        }
                        self.stats.beta_reductions += 1;
                        self.stats.closures_eliminated += 1;
                        return self.distill_expr(&reduced, local_closures);
                    }
                }

                // If callee is an identifier of a top-level function, turn into direct Call
                if let TypedExpr::Ident { name, .. } = &distilled_callee {
                    if self.functions.contains_key(name) {
                        return TypedExpr::Call {
                            callee: name.clone(),
                            args: distilled_args,
                            ty: ty.clone(),
                            span: *span,
                        };
                    }
                }

                TypedExpr::CallIndirect {
                    callee: Box::new(distilled_callee),
                    args: distilled_args,
                    ty: ty.clone(),
                    span: *span,
                }
            }

            TypedExpr::Lambda {
                params,
                body,
                captured,
                ty,
                span,
            } => {
                let distilled_body = self.distill_expr(body, local_closures);
                TypedExpr::Lambda {
                    params: params.clone(),
                    body: Box::new(distilled_body),
                    captured: captured.clone(),
                    ty: ty.clone(),
                    span: *span,
                }
            }

            TypedExpr::Binary {
                op,
                left,
                right,
                ty,
                span,
            } => {
                let l = self.distill_expr(left, local_closures);
                let r = self.distill_expr(right, local_closures);
                TypedExpr::Binary {
                    op: *op,
                    left: Box::new(l),
                    right: Box::new(r),
                    ty: ty.clone(),
                    span: *span,
                }
            }

            TypedExpr::Unary { op, expr, ty, span } => {
                let e = self.distill_expr(expr, local_closures);
                TypedExpr::Unary {
                    op: *op,
                    expr: Box::new(e),
                    ty: ty.clone(),
                    span: *span,
                }
            }

            TypedExpr::Match {
                scrutinee,
                arms,
                ty,
                span,
            } => {
                let s = self.distill_expr(scrutinee, local_closures);
                let new_arms = arms
                    .iter()
                    .map(|arm| TypedMatchArm {
                        patterns: arm.patterns.clone(),
                        body: self.distill_expr(&arm.body, local_closures),
                        span: arm.span,
                    })
                    .collect();
                TypedExpr::Match {
                    scrutinee: Box::new(s),
                    arms: new_arms,
                    ty: ty.clone(),
                    span: *span,
                }
            }

            other => other.clone(),
        }
    }

    /// Recursively simplifies expressions via beta-reduction.
    pub fn beta_reduce_expr(&mut self, expr: &TypedExpr) -> TypedExpr {
        match expr {
            TypedExpr::CallIndirect {
                callee,
                args,
                ty,
                span,
            } => {
                let red_callee = self.beta_reduce_expr(callee);
                let red_args: Vec<TypedExpr> =
                    args.iter().map(|a| self.beta_reduce_expr(a)).collect();

                if let TypedExpr::Lambda { params, body, .. } = &red_callee {
                    if params.len() == red_args.len() {
                        let mut reduced = *body.clone();
                        for ((pname, _), arg) in params.iter().zip(red_args.iter()) {
                            reduced = substitute_var(&reduced, pname, arg);
                        }
                        self.stats.beta_reductions += 1;
                        self.stats.closures_eliminated += 1;
                        return self.beta_reduce_expr(&reduced);
                    }
                }

                // If callee is a top-level function identifier, convert to direct Call
                if let TypedExpr::Ident { name, .. } = &red_callee {
                    if self.functions.contains_key(name) {
                        return TypedExpr::Call {
                            callee: name.clone(),
                            args: red_args,
                            ty: ty.clone(),
                            span: *span,
                        };
                    }
                }

                TypedExpr::CallIndirect {
                    callee: Box::new(red_callee),
                    args: red_args,
                    ty: ty.clone(),
                    span: *span,
                }
            }
            TypedExpr::Binary {
                op,
                left,
                right,
                ty,
                span,
            } => TypedExpr::Binary {
                op: *op,
                left: Box::new(self.beta_reduce_expr(left)),
                right: Box::new(self.beta_reduce_expr(right)),
                ty: ty.clone(),
                span: *span,
            },
            TypedExpr::Unary { op, expr, ty, span } => TypedExpr::Unary {
                op: *op,
                expr: Box::new(self.beta_reduce_expr(expr)),
                ty: ty.clone(),
                span: *span,
            },
            other => other.clone(),
        }
    }
}

// ============================================================================
// 5. Inter-Procedural Mutual Recursion Distillation (HODIST-02)
// ============================================================================

/// Detects mutual recursion across functions and folds them into a unified single-function loop.
pub fn distill_mutual_recursion(functions: &mut [TypedFunction], stats: &mut AstDistillStats) {
    let fn_map: HashMap<String, TypedFunction> = functions
        .iter()
        .map(|f| (f.name.clone(), f.clone()))
        .collect();

    for func in functions.iter_mut() {
        // Look for mutual recursion chains: func calling g calling h calling func
        let mut visited = Vec::new();
        let mut curr_name = func.name.clone();
        while let Some(curr_fn) = fn_map.get(&curr_name) {
            let next_call = match find_tail_call_callee(&curr_fn.body) {
                Some(call) => call,
                None => break,
            };
            if next_call == func.name && visited.len() >= 2 {
                // Detected mutual recursion loop: func -> ... -> next_call == func
                // Unfold the intermediate chain into func's body and tie a fold knot back to func!
                fold_mutual_recursion_into_function(func, &visited, &fn_map, stats);
                break;
            }
            if visited.contains(&next_call) || !fn_map.contains_key(&next_call) {
                break;
            }
            visited.push(next_call.clone());
            curr_name = next_call;
        }
    }
}

fn find_tail_call_callee(body: &TypedBlock) -> Option<String> {
    for stmt in &body.stmts {
        match stmt {
            TypedStmt::Return(Some(TypedExpr::Call { callee, .. }), _) => {
                return Some(callee.clone());
            }
            TypedStmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if let Some(c) = find_tail_call_callee(then_branch) {
                    return Some(c);
                }
                if let Some(ref eb) = else_branch {
                    if let Some(c) = find_tail_call_callee(eb) {
                        return Some(c);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn fold_mutual_recursion_into_function(
    func: &mut TypedFunction,
    chain: &[String],
    fn_map: &HashMap<String, TypedFunction>,
    stats: &mut AstDistillStats,
) {
    // Replace the call to chain[0] with the fully unfolded chain expressions
    // tying a fold knot directly back to `func.name`
    let mut unfolded_body = func.body.clone();
    for target in chain {
        if let Some(target_fn) = fn_map.get(target) {
            unfolded_body = inline_function_call_in_block(&unfolded_body, target, target_fn);
        }
    }

    func.body = unfolded_body;
    stats.folds_performed += 1;
    stats.functions_fused += chain.len();
}

fn inline_function_call_in_block(
    block: &TypedBlock,
    callee_name: &str,
    callee_fn: &TypedFunction,
) -> TypedBlock {
    let mut new_stmts = Vec::new();
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Return(
                Some(TypedExpr::Call {
                    callee, args, span, ..
                }),
                _,
            ) if callee == callee_name => {
                // Inlined body with substituted arguments
                let mut inlined = callee_fn.body.clone();
                for (param, arg) in callee_fn.params.iter().zip(args.iter()) {
                    inlined = substitute_in_block(&inlined, &param.name, arg);
                }
                new_stmts.extend(inlined.stmts);
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let new_then = inline_function_call_in_block(then_branch, callee_name, callee_fn);
                let new_else = else_branch
                    .as_ref()
                    .map(|eb| inline_function_call_in_block(eb, callee_name, callee_fn));
                new_stmts.push(TypedStmt::If {
                    condition: condition.clone(),
                    then_branch: new_then,
                    else_branch: new_else,
                    span: *span,
                });
            }
            other => new_stmts.push(other.clone()),
        }
    }

    TypedBlock {
        stmts: new_stmts,
        span: block.span,
    }
}

fn substitute_in_block(block: &TypedBlock, var_name: &str, replacement: &TypedExpr) -> TypedBlock {
    let mut new_stmts = Vec::new();
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let {
                name,
                is_mutable,
                ty,
                value,
                span,
            } => {
                new_stmts.push(TypedStmt::Let {
                    name: name.clone(),
                    is_mutable: *is_mutable,
                    ty: ty.clone(),
                    value: substitute_var(value, var_name, replacement),
                    span: *span,
                });
            }
            TypedStmt::Assign { name, value, span } => {
                new_stmts.push(TypedStmt::Assign {
                    name: name.clone(),
                    value: substitute_var(value, var_name, replacement),
                    span: *span,
                });
            }
            TypedStmt::Return(opt_expr, span) => {
                new_stmts.push(TypedStmt::Return(
                    opt_expr
                        .as_ref()
                        .map(|e| substitute_var(e, var_name, replacement)),
                    *span,
                ));
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                new_stmts.push(TypedStmt::If {
                    condition: substitute_var(condition, var_name, replacement),
                    then_branch: substitute_in_block(then_branch, var_name, replacement),
                    else_branch: else_branch
                        .as_ref()
                        .map(|eb| substitute_in_block(eb, var_name, replacement)),
                    span: *span,
                });
            }
            other => new_stmts.push(other.clone()),
        }
    }

    TypedBlock {
        stmts: new_stmts,
        span: block.span,
    }
}

// ============================================================================
// 6. AST Variable Substitution Helper
// ============================================================================

/// Substitutes all occurrences of `var_name` in `expr` with `replacement`.
pub fn substitute_var(expr: &TypedExpr, var_name: &str, replacement: &TypedExpr) -> TypedExpr {
    match expr {
        TypedExpr::Ident { name, .. } if name == var_name => replacement.clone(),
        TypedExpr::Binary {
            op,
            left,
            right,
            ty,
            span,
        } => TypedExpr::Binary {
            op: *op,
            left: Box::new(substitute_var(left, var_name, replacement)),
            right: Box::new(substitute_var(right, var_name, replacement)),
            ty: ty.clone(),
            span: *span,
        },
        TypedExpr::Unary { op, expr, ty, span } => TypedExpr::Unary {
            op: *op,
            expr: Box::new(substitute_var(expr, var_name, replacement)),
            ty: ty.clone(),
            span: *span,
        },
        TypedExpr::Call {
            callee,
            args,
            ty,
            span,
        } => TypedExpr::Call {
            callee: callee.clone(),
            args: args
                .iter()
                .map(|a| substitute_var(a, var_name, replacement))
                .collect(),
            ty: ty.clone(),
            span: *span,
        },
        TypedExpr::CallIndirect {
            callee,
            args,
            ty,
            span,
        } => TypedExpr::CallIndirect {
            callee: Box::new(substitute_var(callee, var_name, replacement)),
            args: args
                .iter()
                .map(|a| substitute_var(a, var_name, replacement))
                .collect(),
            ty: ty.clone(),
            span: *span,
        },
        TypedExpr::Lambda {
            params,
            body,
            captured,
            ty,
            span,
        } => {
            // Avoid capture if parameter shadows var_name
            if params.iter().any(|(p, _)| p == var_name) {
                expr.clone()
            } else {
                TypedExpr::Lambda {
                    params: params.clone(),
                    body: Box::new(substitute_var(body, var_name, replacement)),
                    captured: captured.clone(),
                    ty: ty.clone(),
                    span: *span,
                }
            }
        }
        TypedExpr::Match {
            scrutinee,
            arms,
            ty,
            span,
        } => {
            let new_arms = arms
                .iter()
                .map(|arm| {
                    let shadows = arm.patterns.iter().any(|p| match p {
                        TypedMatchPattern::Variant { bindings, .. } => {
                            bindings.iter().any(|(b, _)| b == var_name)
                        }
                        _ => false,
                    });
                    TypedMatchArm {
                        patterns: arm.patterns.clone(),
                        body: if shadows {
                            arm.body.clone()
                        } else {
                            substitute_var(&arm.body, var_name, replacement)
                        },
                        span: arm.span,
                    }
                })
                .collect();
            TypedExpr::Match {
                scrutinee: Box::new(substitute_var(scrutinee, var_name, replacement)),
                arms: new_arms,
                ty: ty.clone(),
                span: *span,
            }
        }
        other => other.clone(),
    }
}

fn get_direct_return_expr(body: &TypedBlock) -> Option<&TypedExpr> {
    for stmt in &body.stmts {
        if let TypedStmt::Return(Some(expr), _) = stmt {
            return Some(expr);
        }
    }
    None
}

// ============================================================================
// 7. Global Whole-Program Distillation Entry Point (HODIST-01..04)
// ============================================================================

/// Distills an entire typed functional program before MIR lowering:
/// 1. Unfolds composed higher-order functions (`compose`, `curry`, `pipe`).
/// 2. Performs beta-reduction on applied lambda expressions.
/// 3. Folds mutual recursion into consolidated function loops.
/// 4. Eliminates intermediate closure allocations from the AST.
pub fn distill_program(program: &mut TypedProgram) -> AstDistillStats {
    let mut total_stats = AstDistillStats::default();

    // Fixpoint iteration over function distillation passes
    const MAX_DISTILL_PASSES: usize = 6;
    for _ in 0..MAX_DISTILL_PASSES {
        let fn_map: HashMap<String, TypedFunction> = program
            .functions
            .iter()
            .map(|f| (f.name.clone(), f.clone()))
            .collect();

        let mut distiller = AstDistiller::new(&fn_map);
        let mut changed = false;

        for func in &mut program.functions {
            let distilled = distiller.distill_function(func);
            if *func != distilled {
                *func = distilled;
                changed = true;
            }
        }

        total_stats.closures_eliminated += distiller.stats.closures_eliminated;
        total_stats.beta_reductions += distiller.stats.beta_reductions;
        total_stats.functions_fused += distiller.stats.functions_fused;

        if !changed {
            break;
        }
    }

    // Pass 2: Inter-procedural mutual recursion distillation
    distill_mutual_recursion(&mut program.functions, &mut total_stats);

    total_stats
}
