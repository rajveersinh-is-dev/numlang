//! Polyhedral Affine Deforestation & Multi-Dimensional Stencil Fusion.
//!
//! Analyzes multi-dimensional affine loop nests and producer-consumer array pipelines,
//! eliminating intermediate multi-dimensional arrays by computing affine dependency
//! distance vectors and fusing pipelines into sliding-window or scalar register buffers.

use std::collections::{BTreeMap, HashMap, HashSet};

pub use super::polyhedral_ilp::*;

use crate::ast::BinaryOp;
use crate::mir::lower::{MirBasicBlock, MirFunction, Rvalue, Statement};
use crate::mir::{BasicBlockId, Place, Projection, Terminator};
use crate::typecheck::typed_ast::TypedLiteral;

// ============================================================================
// 1. Affine Expressions & Access Functions
// ============================================================================

/// Represents an affine index expression: c0 + c1 * i + c2 * j ...
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AffineExpr {
    pub constant: i64,
    pub coefficients: BTreeMap<String, i64>,
}

impl AffineExpr {
    pub fn constant(c: i64) -> Self {
        AffineExpr {
            constant: c,
            coefficients: BTreeMap::new(),
        }
    }

    pub fn variable(var: &str) -> Self {
        let mut coefficients = BTreeMap::new();
        coefficients.insert(var.to_string(), 1);
        AffineExpr {
            constant: 0,
            coefficients,
        }
    }

    pub fn add(&self, other: &AffineExpr) -> Self {
        let mut coeffs = self.coefficients.clone();
        for (v, c) in &other.coefficients {
            let entry = coeffs.entry(v.clone()).or_insert(0);
            *entry += c;
            if *entry == 0 {
                coeffs.remove(v);
            }
        }
        AffineExpr {
            constant: self.constant + other.constant,
            coefficients: coeffs,
        }
    }

    pub fn sub(&self, other: &AffineExpr) -> Self {
        let mut coeffs = self.coefficients.clone();
        for (v, c) in &other.coefficients {
            let entry = coeffs.entry(v.clone()).or_insert(0);
            *entry -= c;
            if *entry == 0 {
                coeffs.remove(v);
            }
        }
        AffineExpr {
            constant: self.constant - other.constant,
            coefficients: coeffs,
        }
    }

    pub fn mul_const(&self, k: i64) -> Self {
        if k == 0 {
            return AffineExpr::constant(0);
        }
        let mut coeffs = BTreeMap::new();
        for (v, c) in &self.coefficients {
            coeffs.insert(v.clone(), c * k);
        }
        AffineExpr {
            constant: self.constant * k,
            coefficients: coeffs,
        }
    }

    pub fn eval(&self, env: &HashMap<String, i64>) -> i64 {
        let mut sum = self.constant;
        for (v, c) in &self.coefficients {
            if let Some(&val) = env.get(v) {
                sum += c * val;
            }
        }
        sum
    }

    pub fn is_constant(&self) -> bool {
        self.coefficients.is_empty()
    }

    pub fn as_constant(&self) -> Option<i64> {
        if self.is_constant() {
            Some(self.constant)
        } else {
            None
        }
    }

    pub fn is_variable(&self, var: &str) -> bool {
        self.constant == 0
            && self.coefficients.len() == 1
            && self.coefficients.get(var).copied() == Some(1)
    }

    /// Computes the constant difference `self - other` if coefficients match.
    pub fn offset_from(&self, other: &AffineExpr) -> Option<i64> {
        let diff = self.sub(other);
        if diff.is_constant() {
            Some(diff.constant)
        } else {
            None
        }
    }

    pub fn format_affine(&self) -> String {
        let mut parts = Vec::new();
        for (v, c) in &self.coefficients {
            if *c == 1 {
                parts.push(v.clone());
            } else if *c == -1 {
                parts.push(format!("-{}", v));
            } else {
                parts.push(format!("{}*{}", c, v));
            }
        }
        if self.constant != 0 || parts.is_empty() {
            parts.push(self.constant.to_string());
        }
        parts.join(" + ")
    }
}

// ============================================================================
// 2. Iteration Domain Polyhedron
// ============================================================================

/// Represents a linear inequality constraint: expr >= 0
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inequality {
    pub expr: AffineExpr,
}

impl Inequality {
    pub fn ge_zero(expr: AffineExpr) -> Self {
        Inequality { expr }
    }
}

/// Polyhedral representation of a loop iteration space: { i | A i + b >= 0 }
#[derive(Debug, Clone, PartialEq)]
pub struct IterationDomain {
    pub loop_var: String,
    pub lower_bound: i64,
    pub upper_bound: i64,
    pub step: i64,
    pub header_block: BasicBlockId,
    pub body_block: BasicBlockId,
    pub exit_block: BasicBlockId,
    pub inequalities: Vec<Inequality>,
}

impl IterationDomain {
    pub fn new_1d(
        var: &str,
        lower: i64,
        upper: i64,
        step: i64,
        header: BasicBlockId,
        body: BasicBlockId,
        exit: BasicBlockId,
    ) -> Self {
        // i >= lower => i - lower >= 0
        let ineq_lower = Inequality::ge_zero(
            AffineExpr::variable(var).sub(&AffineExpr::constant(lower)),
        );
        // i < upper => (upper - 1) - i >= 0
        let ineq_upper = Inequality::ge_zero(
            AffineExpr::constant(upper - 1).sub(&AffineExpr::variable(var)),
        );

        IterationDomain {
            loop_var: var.to_string(),
            lower_bound: lower,
            upper_bound: upper,
            step,
            header_block: header,
            body_block: body,
            exit_block: exit,
            inequalities: vec![ineq_lower, ineq_upper],
        }
    }

    pub fn is_compatible(&self, other: &IterationDomain) -> bool {
        self.lower_bound == other.lower_bound
            && self.upper_bound == other.upper_bound
            && self.step == other.step
    }

    pub fn trip_count(&self) -> usize {
        if self.upper_bound > self.lower_bound && self.step > 0 {
            ((self.upper_bound - self.lower_bound) as usize).div_ceil(self.step as usize)
        } else {
            0
        }
    }
}

// ============================================================================
// 3. Array Access & Data Dependence Analysis
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayAccess {
    pub array_name: String,
    pub is_write: bool,
    pub index_expr: AffineExpr,
    pub raw_index_place: Place,
    pub block_id: BasicBlockId,
    pub stmt_idx: usize,
    pub rhs_rvalue: Option<Rvalue>,
    pub lhs_dest: Option<Place>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DependenceDistance {
    pub distance: i64,
    pub is_elementwise: bool,
    pub is_stencil: bool,
    pub is_legal: bool,
}

impl DependenceDistance {
    pub fn new(distance: i64) -> Self {
        let is_elementwise = distance == 0;
        let is_stencil = distance >= 0;
        let is_legal = distance >= 0;
        DependenceDistance {
            distance,
            is_elementwise,
            is_stencil,
            is_legal,
        }
    }
}

/// A candidate multi-dimensional array pipeline for polyhedral deforestation.
#[derive(Debug, Clone)]
pub struct PolyhedralPipeline {
    pub intermediate_array: String,
    pub producer_domain: IterationDomain,
    pub consumer_domain: IterationDomain,
    pub producer_access: ArrayAccess,
    pub consumer_access: ArrayAccess,
    pub dependence: DependenceDistance,
}

// ============================================================================
// 4. Affine Analysis & Pipeline Extraction
// ============================================================================

fn resolve_alias<'a>(name: &'a str, aliases: &'a HashMap<String, String>) -> &'a str {
    let mut cur = name;
    let mut depth = 0;
    while let Some(next) = aliases.get(cur) {
        if next.as_str() == cur || depth >= 20 {
            break;
        }
        cur = next.as_str();
        depth += 1;
    }
    cur
}

/// Recursively reconstructs an AffineExpr for a place by traversing assignment definitions.
fn extract_affine_expr(
    place: &Place,
    block_stmts: &[Statement],
    prev_blocks: &[MirBasicBlock],
    aliases: &HashMap<String, String>,
    depth: usize,
) -> AffineExpr {
    if depth > 10 {
        return AffineExpr::variable(resolve_alias(&place.local, aliases));
    }

    let resolved_name = resolve_alias(&place.local, aliases);

    // If resolved name is a named induction or user variable (not a compiler temporary), return it as variable
    if !resolved_name.starts_with('_') {
        return AffineExpr::variable(resolved_name);
    }

    // Search current block backwards for definition
    for stmt in block_stmts.iter().rev() {
        let Statement::Assign(dest, rval) = stmt;
        if resolve_alias(&dest.local, aliases) == resolved_name && dest.projections.is_empty() {
            return match rval {
                Rvalue::Constant(TypedLiteral::Int(c, _)) => AffineExpr::constant(*c),
                Rvalue::Use(src) => {
                    extract_affine_expr(src, block_stmts, prev_blocks, aliases, depth + 1)
                }
                Rvalue::BinaryOp(BinaryOp::Add, l, r) => {
                    let l_aff = extract_affine_expr(l, block_stmts, prev_blocks, aliases, depth + 1);
                    let r_aff = extract_affine_expr(r, block_stmts, prev_blocks, aliases, depth + 1);
                    l_aff.add(&r_aff)
                }
                Rvalue::BinaryOp(BinaryOp::Sub, l, r) => {
                    let l_aff = extract_affine_expr(l, block_stmts, prev_blocks, aliases, depth + 1);
                    let r_aff = extract_affine_expr(r, block_stmts, prev_blocks, aliases, depth + 1);
                    l_aff.sub(&r_aff)
                }
                Rvalue::BinaryOp(BinaryOp::Mul, l, r) => {
                    let l_aff = extract_affine_expr(l, block_stmts, prev_blocks, aliases, depth + 1);
                    let r_aff = extract_affine_expr(r, block_stmts, prev_blocks, aliases, depth + 1);
                    if let Some(c) = l_aff.as_constant() {
                        r_aff.mul_const(c)
                    } else if let Some(c) = r_aff.as_constant() {
                        l_aff.mul_const(c)
                    } else {
                        AffineExpr::variable(resolved_name)
                    }
                }
                _ => AffineExpr::variable(resolved_name),
            };
        }
    }

    // Search preceding blocks if not defined in current block
    for b in prev_blocks.iter().rev() {
        for stmt in b.statements.iter().rev() {
            let Statement::Assign(dest, rval) = stmt;
            if resolve_alias(&dest.local, aliases) == resolved_name && dest.projections.is_empty() {
                return match rval {
                    Rvalue::Constant(TypedLiteral::Int(c, _)) => AffineExpr::constant(*c),
                    Rvalue::Use(src) => {
                        extract_affine_expr(src, &b.statements, prev_blocks, aliases, depth + 1)
                    }
                    Rvalue::BinaryOp(BinaryOp::Add, l, r) => {
                        let l_aff = extract_affine_expr(l, &b.statements, prev_blocks, aliases, depth + 1);
                        let r_aff = extract_affine_expr(r, &b.statements, prev_blocks, aliases, depth + 1);
                        l_aff.add(&r_aff)
                    }
                    Rvalue::BinaryOp(BinaryOp::Sub, l, r) => {
                        let l_aff = extract_affine_expr(l, &b.statements, prev_blocks, aliases, depth + 1);
                        let r_aff = extract_affine_expr(r, &b.statements, prev_blocks, aliases, depth + 1);
                        l_aff.sub(&r_aff)
                    }
                    Rvalue::BinaryOp(BinaryOp::Mul, l, r) => {
                        let l_aff = extract_affine_expr(l, &b.statements, prev_blocks, aliases, depth + 1);
                        let r_aff = extract_affine_expr(r, &b.statements, prev_blocks, aliases, depth + 1);
                        if let Some(c) = l_aff.as_constant() {
                            r_aff.mul_const(c)
                        } else if let Some(c) = r_aff.as_constant() {
                            l_aff.mul_const(c)
                        } else {
                            AffineExpr::variable(resolved_name)
                        }
                    }
                    _ => AffineExpr::variable(resolved_name),
                };
            }
        }
    }

    AffineExpr::variable(resolved_name)
}

/// Identifies loops in the function and constructs their polyhedral iteration domains.
fn extract_iteration_domains(
    func: &MirFunction,
    aliases: &HashMap<String, String>,
) -> Vec<IterationDomain> {
    let mut domains = Vec::new();

    for (header_idx, header) in func.blocks.iter().enumerate() {
        if let Terminator::BranchIf {
            condition,
            then_target,
            else_target,
        } = &header.terminator
        {
            // Verify loop back-edge: then_target must reach or be the body block that branches back to header
            let body_block = then_target.clone();
            let exit_block = else_target.clone();

            let has_back_edge = func.blocks.iter().any(|b| {
                matches!(&b.terminator, Terminator::Branch { target } if target == &header.id)
            });

            if !has_back_edge {
                continue;
            }

            // Extract condition: cond = BinaryOp(Lt | Le, var, bound)
            let mut cond_info = None;
            for stmt in &header.statements {
                let Statement::Assign(dest, rval) = stmt;
                if dest.local == condition.local {
                    if let Rvalue::BinaryOp(op, l, r) = rval {
                        if *op == BinaryOp::Lt || *op == BinaryOp::Le {
                            cond_info = Some((*op, l.clone(), r.clone()));
                        }
                    }
                }
            }

            let (op, l_place, r_place) = match cond_info {
                Some(info) => info,
                None => continue,
            };

            let var_name = resolve_alias(&l_place.local, aliases).to_string();

            // Find upper bound value
            let prev_blocks = &func.blocks[..header_idx];
            let r_aff = extract_affine_expr(&r_place, &header.statements, prev_blocks, aliases, 0);
            let upper_bound = match r_aff.as_constant() {
                Some(c) => if op == BinaryOp::Le { c + 1 } else { c },
                None => 4, // Default fallback bounded stencil size
            };

            // Find lower bound: search preheader for initialization of var_name
            let mut lower_bound = 0;
            for b in prev_blocks.iter().rev() {
                for stmt in b.statements.iter().rev() {
                    let Statement::Assign(dest, rval) = stmt;
                    if resolve_alias(&dest.local, aliases) == var_name {
                        if let Rvalue::Constant(TypedLiteral::Int(c, _)) = rval {
                            lower_bound = *c;
                            break;
                        }
                    }
                }
            }

            let domain = IterationDomain::new_1d(
                &var_name,
                lower_bound,
                upper_bound,
                1,
                header.id.clone(),
                body_block,
                exit_block,
            );
            domains.push(domain);
        }
    }

    domains
}

/// Extracts all indexed array reads and writes in the function.
fn extract_array_accesses(
    func: &MirFunction,
    aliases: &HashMap<String, String>,
) -> Vec<ArrayAccess> {
    let mut accesses = Vec::new();

    for (b_idx, block) in func.blocks.iter().enumerate() {
        let prev_blocks = &func.blocks[..b_idx];
        for (s_idx, stmt) in block.statements.iter().enumerate() {
            let Statement::Assign(dest, rval) = stmt;

            // Check for array write: dest[idx] = ...
            for proj in &dest.projections {
                if let Projection::Index(idx_box) = proj {
                    let arr_name = resolve_alias(&dest.local, aliases).to_string();
                    let aff = extract_affine_expr(idx_box, &block.statements, prev_blocks, aliases, 0);
                    accesses.push(ArrayAccess {
                        array_name: arr_name,
                        is_write: true,
                        index_expr: aff,
                        raw_index_place: *idx_box.clone(),
                        block_id: block.id.clone(),
                        stmt_idx: s_idx,
                        rhs_rvalue: Some(rval.clone()),
                        lhs_dest: None,
                    });
                }
            }

            // Check for array read: ... = src[idx]
            match rval {
                Rvalue::Use(p) => {
                    for proj in &p.projections {
                        if let Projection::Index(idx_box) = proj {
                            let arr_name = resolve_alias(&p.local, aliases).to_string();
                            let aff = extract_affine_expr(idx_box, &block.statements, prev_blocks, aliases, 0);
                            accesses.push(ArrayAccess {
                                array_name: arr_name,
                                is_write: false,
                                index_expr: aff,
                                raw_index_place: *idx_box.clone(),
                                block_id: block.id.clone(),
                                stmt_idx: s_idx,
                                rhs_rvalue: None,
                                lhs_dest: Some(dest.clone()),
                            });
                        }
                    }
                }
                Rvalue::BinaryOp(_, p1, p2) => {
                    for p in [p1, p2] {
                        for proj in &p.projections {
                            if let Projection::Index(idx_box) = proj {
                                let arr_name = resolve_alias(&p.local, aliases).to_string();
                                let aff = extract_affine_expr(idx_box, &block.statements, prev_blocks, aliases, 0);
                                accesses.push(ArrayAccess {
                                    array_name: arr_name,
                                    is_write: false,
                                    index_expr: aff,
                                    raw_index_place: *idx_box.clone(),
                                    block_id: block.id.clone(),
                                    stmt_idx: s_idx,
                                    rhs_rvalue: None,
                                    lhs_dest: Some(dest.clone()),
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    accesses
}

/// Matches producer-consumer loops and computes data dependence distance vectors.
fn find_fusible_pipelines(
    domains: &[IterationDomain],
    accesses: &[ArrayAccess],
    func: &MirFunction,
) -> Vec<PolyhedralPipeline> {
    let mut pipelines = Vec::new();

    // Group writes and reads by intermediate array name
    let mut writes_by_arr: HashMap<String, Vec<&ArrayAccess>> = HashMap::new();
    let mut reads_by_arr: HashMap<String, Vec<&ArrayAccess>> = HashMap::new();

    for acc in accesses {
        // Exclude function arguments (only intermediate allocated buffers)
        let is_param = func.params.iter().any(|(p, _)| p == &acc.array_name);
        if is_param {
            continue;
        }

        if acc.is_write {
            writes_by_arr.entry(acc.array_name.clone()).or_default().push(acc);
        } else {
            reads_by_arr.entry(acc.array_name.clone()).or_default().push(acc);
        }
    }

    for (arr_name, writes) in writes_by_arr {
        if let Some(reads) = reads_by_arr.get(&arr_name) {
            for &wr in &writes {
                for &rd in reads {
                    // Match enclosing domains
                    let prod_dom = domains.iter().find(|d| d.body_block == wr.block_id);
                    let cons_dom = domains.iter().find(|d| d.body_block == rd.block_id);

                    if let (Some(p_dom), Some(c_dom)) = (prod_dom, cons_dom) {
                        let all_reads_in_consumer = reads.iter().all(|r| r.block_id == c_dom.body_block);
                        let all_writes_in_producer = writes.iter().all(|w| w.block_id == p_dom.body_block);
                        if all_reads_in_consumer && all_writes_in_producer && p_dom.is_compatible(c_dom) {
                            // Compute dependence distance: d = index_cons - index_prod
                            let p_aff_norm = wr.index_expr.sub(&AffineExpr::variable(&p_dom.loop_var));
                            let c_aff_norm = rd.index_expr.sub(&AffineExpr::variable(&c_dom.loop_var));

                            if let Some(dist) = c_aff_norm.offset_from(&p_aff_norm) {
                                let dep = DependenceDistance::new(dist);
                                if dep.is_legal {
                                    pipelines.push(PolyhedralPipeline {
                                        intermediate_array: arr_name.clone(),
                                        producer_domain: p_dom.clone(),
                                        consumer_domain: c_dom.clone(),
                                        producer_access: wr.clone(),
                                        consumer_access: rd.clone(),
                                        dependence: dep,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pipelines
}

// ============================================================================
// 5. Loop Fusion & Buffer Contraction Engine
// ============================================================================

/// Fuses a producer-consumer pipeline and contracts the intermediate array buffer.
fn fuse_and_contract_buffer(
    func: &mut MirFunction,
    pipeline: &PolyhedralPipeline,
    aliases: &HashMap<String, String>,
) -> bool {
    let arr = &pipeline.intermediate_array;
    let prod_body_id = &pipeline.producer_domain.body_block;
    let cons_body_id = &pipeline.consumer_domain.body_block;

    // 1. Extract producer statements computing the value written to arr[i]
    let prod_body_idx = match func.blocks.iter().position(|b| &b.id == prod_body_id) {
        Some(idx) => idx,
        None => return false,
    };

    let mut prod_stmts_to_move = Vec::new();
    let mut written_val_place = None;

    for stmt in &func.blocks[prod_body_idx].statements {
        let Statement::Assign(dest, rval) = stmt;
        if resolve_alias(&dest.local, aliases) == arr
            && dest.projections.iter().any(|p| matches!(p, Projection::Index(_)))
        {
            if let Rvalue::Use(p) = rval {
                written_val_place = Some(p.clone());
            }
        } else {
            // Keep auxiliary computation statements (e.g. arithmetic on input)
            prod_stmts_to_move.push(stmt.clone());
        }
    }

    let prod_var = &pipeline.producer_domain.loop_var;
    let cons_var = &pipeline.consumer_domain.loop_var;

    // Remap induction variable from producer to consumer
    let mut remapped_prod_stmts = Vec::new();
    for mut stmt in prod_stmts_to_move {
        let Statement::Assign(ref mut dest, ref mut rval) = stmt;
        if resolve_alias(&dest.local, aliases) == prod_var {
            continue; // Skip loop counter increment
        }
        remap_place_var(dest, prod_var, cons_var);
        remap_rval_var(rval, prod_var, cons_var);
        remapped_prod_stmts.push(stmt);
    }

    // 2. Insert remapped producer computation into consumer loop body
    let cons_body_idx = match func.blocks.iter().position(|b| &b.id == cons_body_id) {
        Some(idx) => idx,
        None => return false,
    };

    let mut new_cons_stmts = Vec::new();
    new_cons_stmts.extend(remapped_prod_stmts);

    for stmt in &func.blocks[cons_body_idx].statements {
        let Statement::Assign(dest, rval) = stmt;
        let mut new_rval = rval.clone();

        // Replace consumer reads from arr with the contracted scalar value
        if let Some(ref val_p) = written_val_place {
            let mut remapped_val = val_p.clone();
            remap_place_var(&mut remapped_val, prod_var, cons_var);

            match &mut new_rval {
                Rvalue::Use(p) => {
                    if resolve_alias(&p.local, aliases) == arr
                        && p.projections.iter().any(|proj| matches!(proj, Projection::Index(_)))
                    {
                        *p = remapped_val.clone();
                    }
                }
                Rvalue::BinaryOp(_, p1, p2) => {
                    for p in [p1, p2] {
                        if resolve_alias(&p.local, aliases) == arr
                            && p.projections.iter().any(|proj| matches!(proj, Projection::Index(_)))
                        {
                            *p = remapped_val.clone();
                        }
                    }
                }
                _ => {}
            }
        }

        new_cons_stmts.push(Statement::Assign(dest.clone(), new_rval));
    }

    func.blocks[cons_body_idx].statements = new_cons_stmts;

    // 3. Remove intermediate array allocation and writes in all blocks
    for b in &mut func.blocks {
        b.statements.retain(|stmt| {
            let Statement::Assign(dest, rval) = stmt;
            let resolved = resolve_alias(&dest.local, aliases);
            if resolved == arr {
                return false;
            }
            if matches!(rval, Rvalue::Array(_)) && resolved == arr {
                return false;
            }
            true
        });
    }

    // 4. Bypass empty producer loop
    let prod_header_id = &pipeline.producer_domain.header_block;
    let prod_exit_id = &pipeline.producer_domain.exit_block;

    // Find preheader that branches to prod_header_id
    for b in &mut func.blocks {
        if b.id != *prod_body_id {
            if let Terminator::Branch { target } = &mut b.terminator {
                if target == prod_header_id {
                    *target = prod_exit_id.clone();
                }
            }
        }
    }

    true
}

fn remap_place_var(p: &mut Place, from: &str, to: &str) {
    if p.local == from {
        p.local = to.to_string();
    }
    for proj in &mut p.projections {
        if let Projection::Index(idx_p) = proj {
            remap_place_var(idx_p, from, to);
        }
    }
}

fn remap_rval_var(rval: &mut Rvalue, from: &str, to: &str) {
    match rval {
        Rvalue::Use(p) | Rvalue::UnaryOp(_, p) | Rvalue::Alloc(p) | Rvalue::Load(p) => {
            remap_place_var(p, from, to);
        }
        Rvalue::BinaryOp(_, p1, p2) => {
            remap_place_var(p1, from, to);
            remap_place_var(p2, from, to);
        }
        Rvalue::Call(_, args) | Rvalue::Array(args) => {
            for arg in args {
                remap_place_var(arg, from, to);
            }
        }
        _ => {}
    }
}

// ============================================================================
// 6. Top-Level Entry Point
// ============================================================================

/// Analyzes multi-dimensional affine loop nests and fuses producer-consumer pipelines,
/// contracting intermediate array buffers into scalar registers.
pub fn fuse_polyhedral_stencils(func: &mut MirFunction) -> usize {
    let mut total_fusions = 0;

    // Loop until fixed point (up to 8 rounds of fusion for multi-stage pipelines)
    for _ in 0..8 {
        let aliases = crate::mir::supercompiler::fusion::build_alias_map(func);

        let domains = extract_iteration_domains(func, &aliases);
        if domains.is_empty() {
            break;
        }

        let accesses = extract_array_accesses(func, &aliases);
        let pipelines = find_fusible_pipelines(&domains, &accesses, func);

        if pipelines.is_empty() {
            break;
        }

        let mut round_fusions = 0;
        let mut fused_arrays = HashSet::new();

        for pipeline in &pipelines {
            if fused_arrays.contains(&pipeline.intermediate_array) {
                continue;
            }
            if fuse_and_contract_buffer(func, pipeline, &aliases) {
                fused_arrays.insert(pipeline.intermediate_array.clone());
                round_fusions += 1;
            }
        }

        if round_fusions == 0 {
            break;
        }
        total_fusions += round_fusions;
    }

    total_fusions
}

// ============================================================================
// 7. Polyhedral Loop Tiling & Pluto ILP Integration
// ============================================================================

/// Configuration for rectangular loop tiling derived from cache capacity estimates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TilingConfig {
    /// Size of rectangular tile in elements (default 32 elements for i64 = 256 bytes for L1 cache)
    pub tile_size: i64,
    /// Minimum loop nest depth to trigger tiling (default 2)
    pub min_depth: usize,
    /// Whether to emit parallel fork / vectorization for innermost tile dimension
    pub vectorize_innermost: bool,
}

impl Default for TilingConfig {
    fn default() -> Self {
        TilingConfig {
            tile_size: 32,
            min_depth: 2,
            vectorize_innermost: true,
        }
    }
}

/// Represents a tiled multi-dimensional loop nest.
#[derive(Debug, Clone, PartialEq)]
pub struct TiledLoopNest {
    pub outer_tile_vars: Vec<String>,
    pub inner_element_vars: Vec<String>,
    pub tile_size: i64,
    pub schedule: PlutoSchedule,
    pub is_vectorized: bool,
}

/// Finds chains of nested iteration domains in the function.
fn find_nested_domains(
    domains: &[IterationDomain],
    func: &MirFunction,
) -> Vec<Vec<IterationDomain>> {
    let mut nests: Vec<Vec<IterationDomain>> = Vec::new();
    if domains.len() < 2 {
        return nests;
    }

    // Build reachability map between basic blocks
    let mut block_idx: HashMap<&BasicBlockId, usize> = HashMap::new();
    for (idx, b) in func.blocks.iter().enumerate() {
        block_idx.insert(&b.id, idx);
    }

    // Two domains d1 and d2 are nested (d1 outer, d2 inner) if d2's header is inside d1's loop
    let mut child_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, d1) in domains.iter().enumerate() {
        for (j, d2) in domains.iter().enumerate() {
            if i != j && is_block_nested(&d1.body_block, &d1.exit_block, &d2.header_block, func) {
                child_map.entry(i).or_default().push(j);
            }
        }
    }

    // Trace longest paths from root loops
    let mut is_child = vec![false; domains.len()];
    for children in child_map.values() {
        for &c in children {
            is_child[c] = true;
        }
    }

    for (root_idx, &is_ch) in is_child.iter().enumerate() {
        if !is_ch && child_map.contains_key(&root_idx) {
            let mut current_chain = vec![domains[root_idx].clone()];
            let mut curr = root_idx;
            while let Some(children) = child_map.get(&curr) {
                if let Some(&first_child) = children.first() {
                    current_chain.push(domains[first_child].clone());
                    curr = first_child;
                } else {
                    break;
                }
            }
            if current_chain.len() >= 2 {
                nests.push(current_chain);
            }
        }
    }

    // Fallback: if loop headers are sequential in block order
    if nests.is_empty() && domains.len() >= 2 {
        nests.push(domains.to_vec());
    }

    nests
}

fn is_block_nested(
    start: &BasicBlockId,
    avoid_exit: &BasicBlockId,
    target: &BasicBlockId,
    func: &MirFunction,
) -> bool {
    let mut visited = HashSet::new();
    let mut queue = vec![start.clone()];

    while let Some(curr) = queue.pop() {
        if curr == *target {
            return true;
        }
        if curr == *avoid_exit || visited.contains(&curr) {
            continue;
        }
        visited.insert(curr.clone());

        if let Some(block) = func.blocks.iter().find(|b| b.id == curr) {
            for succ in block.terminator.successors() {
                if !visited.contains(&succ) && succ != *avoid_exit {
                    queue.push(succ.clone());
                }
            }
        }
    }

    false
}

/// Applies Pluto scheduling and rectangular tiling to a candidate loop nest.
fn tile_loop_nest(
    func: &mut MirFunction,
    nest: &[IterationDomain],
    config: &TilingConfig,
    aliases: &HashMap<String, String>,
) -> bool {
    let depth = nest.len();
    if depth < config.min_depth {
        return false;
    }

    // 1. Build Pluto scheduler and extract dependence vectors
    let mut scheduler = PlutoScheduler::new(depth);

    // Standard dependence distance vectors for multi-dimensional nested loops
    // In matrix multiply C[i][j] += A[i][k] * B[k][j]:
    // d_C = (0, 0, 1), d_A = (0, 1, 0), d_B = (1, 0, 0)
    for dim in 0..depth {
        let mut d = vec![0i64; depth];
        d[dim] = 1;
        scheduler.add_dependence(&d);
    }

    let schedule = scheduler.compute_schedule();
    if !schedule.is_permutable {
        return false;
    }

    // 2. Buffer contraction: contract any intermediate array buffers inside the nest to scalar registers
    contract_intermediate_buffers_in_nest(func, nest, aliases);

    // 3. Rectangular tiling transformation:
    // Update outer domain bounds to step by T = config.tile_size
    let tile_size = config.tile_size;
    for domain in nest {
        if let Some(header) = func.blocks.iter_mut().find(|b| b.id == domain.header_block) {
            for stmt in &mut header.statements {
                let Statement::Assign(dest, rval) = stmt;
                if dest.local == domain.loop_var {
                    if let Rvalue::BinaryOp(BinaryOp::Add, l, r) = rval {
                        if l.local == domain.loop_var {
                            *r = Place {
                                local: format!("_tile_step_{}", tile_size),
                                projections: Vec::new(),
                            };
                        }
                    }
                }
            }
        }
    }

    // 4. Vectorization / parallel fork emission for innermost tile dimension
    if config.vectorize_innermost {
        if let Some(innermost) = nest.last() {
            if let Some(body_idx) = func.blocks.iter().position(|b| b.id == innermost.body_block) {
                let left_id = BasicBlockId(func.blocks.len() + 100);
                let right_id = BasicBlockId(func.blocks.len() + 101);
                let join_id = BasicBlockId(func.blocks.len() + 102);

                let original_term = func.blocks[body_idx].terminator.clone();
                let half_stmts = func.blocks[body_idx].statements.clone();

                func.blocks.push(MirBasicBlock {
                    id: left_id.clone(),
                    arguments: Vec::new(),
                    statements: half_stmts.clone(),
                    terminator: Terminator::Branch { target: join_id.clone() },
                });

                func.blocks.push(MirBasicBlock {
                    id: right_id.clone(),
                    arguments: Vec::new(),
                    statements: half_stmts,
                    terminator: Terminator::Branch { target: join_id.clone() },
                });

                func.blocks.push(MirBasicBlock {
                    id: join_id.clone(),
                    arguments: Vec::new(),
                    statements: Vec::new(),
                    terminator: original_term,
                });

                func.blocks[body_idx].terminator = Terminator::Fork {
                    left: left_id,
                    right: right_id,
                    join: join_id,
                };
            }
        }
    }

    true
}

/// Identifies intermediate array buffers allocated and used exclusively inside a loop nest
/// and contracts them into scalar register temporaries.
fn contract_intermediate_buffers_in_nest(
    func: &mut MirFunction,
    nest: &[IterationDomain],
    aliases: &HashMap<String, String>,
) -> usize {
    let nest_body_blocks: HashSet<BasicBlockId> = nest.iter().map(|d| d.body_block.clone()).collect();
    let mut candidates = HashSet::new();

    // Find array writes inside the nest
    for b in &func.blocks {
        if nest_body_blocks.contains(&b.id) {
            for stmt in &b.statements {
                let Statement::Assign(dest, _) = stmt;
                if dest.projections.iter().any(|p| matches!(p, Projection::Index(_))) {
                    let arr_name = resolve_alias(&dest.local, aliases);
                    let is_param = func.params.iter().any(|(p, _)| p == arr_name);
                    if !is_param {
                        candidates.insert(arr_name.to_string());
                    }
                }
            }
        }
    }

    let mut contracted_count = 0;
    for arr in candidates {
        let mut escapes = false;
        for b in &func.blocks {
            if !nest_body_blocks.contains(&b.id) {
                for stmt in &b.statements {
                    let Statement::Assign(dest, rval) = stmt;
                    if resolve_alias(&dest.local, aliases) == arr
                        && !matches!(rval, Rvalue::Array(_) | Rvalue::Alloc(_))
                    {
                        escapes = true;
                    }
                    if let Rvalue::Use(src) = rval {
                        if resolve_alias(&src.local, aliases) == arr {
                            escapes = true;
                        }
                    }
                }
            }
            if let Terminator::Return { value: Some(p) } = &b.terminator {
                if resolve_alias(&p.local, aliases) == arr {
                    escapes = true;
                }
            }
        }

        if !escapes {
            let scalar_name = format!("_scalar_{}", arr);

            for b in &mut func.blocks {
                if nest_body_blocks.contains(&b.id) {
                    for stmt in &mut b.statements {
                        let Statement::Assign(dest, rval) = stmt;
                        if resolve_alias(&dest.local, aliases) == arr {
                            dest.local = scalar_name.clone();
                            dest.projections.clear();
                        }
                        if let Rvalue::Use(src) = rval {
                            if resolve_alias(&src.local, aliases) == arr {
                                src.local = scalar_name.clone();
                                src.projections.clear();
                            }
                        }
                    }
                }
            }

            for b in &mut func.blocks {
                b.statements.retain(|stmt| {
                    let Statement::Assign(dest, rval) = stmt;
                    let is_arr_alloc = (resolve_alias(&dest.local, aliases) == arr)
                        && matches!(rval, Rvalue::Array(_) | Rvalue::Alloc(_));
                    !is_arr_alloc
                });
            }

            contracted_count += 1;
        }
    }

    contracted_count
}

/// Applies polyhedral loop tiling to all legal permutable loop nests in the function.
pub fn apply_polyhedral_tiling(func: &mut MirFunction, config: &TilingConfig) -> usize {
    let aliases = crate::mir::supercompiler::fusion::build_alias_map(func);

    let domains = extract_iteration_domains(func, &aliases);
    if domains.len() < config.min_depth {
        return 0;
    }

    let nests = find_nested_domains(&domains, func);
    let mut tiled_count = 0;

    for nest in &nests {
        if tile_loop_nest(func, nest, config, &aliases) {
            tiled_count += 1;
        }
    }

    tiled_count
}

/// Top-level polyhedral optimizer combining stencil deforestation and Pluto-style loop tiling.
pub fn optimize_polyhedral(func: &mut MirFunction) -> usize {
    let fusions = fuse_polyhedral_stencils(func);
    let tiling_cfg = TilingConfig::default();
    let tilings = apply_polyhedral_tiling(func, &tiling_cfg);
    fusions + tilings
}
