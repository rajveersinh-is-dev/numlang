//! Content-Addressed Basic Block Outlining for Post-Residualization MIR.
//!
//! Mitigates binary size bloat resulting from deep Reynolds defunctionalization
//! and multi-result MRSC specialization. Normalizes basic block instruction
//! sequences modulo register names, hashes operations using SHA-256, and extracts
//! duplicated instruction sequences into shared outlined subroutines.

use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

use crate::mir::lower::{MirBasicBlock, MirFunction, MirLocalDecl, MirProgram, Rvalue, Statement};
use crate::mir::{BasicBlockId, Place, Projection, Terminator};
use crate::typecheck::Type;

/// Configuration for basic block outlining pass.
#[derive(Debug, Clone)]
pub struct OutlinerConfig {
    /// Minimum number of statements in a candidate sequence to consider for outlining.
    pub min_sequence_length: usize,
    /// Minimum sequence similarity threshold (0.0 to 1.0) to group as duplicates.
    pub similarity_threshold: f64,
    /// Maximum number of outlined functions to create.
    pub max_outlined_functions: usize,
}

impl Default for OutlinerConfig {
    fn default() -> Self {
        Self {
            min_sequence_length: 2,
            similarity_threshold: 0.90,
            max_outlined_functions: 100,
        }
    }
}

/// Telemetry metrics produced by the outliner pass.
#[derive(Debug, Default, Clone)]
pub struct OutlinerStats {
    pub candidate_sequences_found: usize,
    pub outlined_functions_created: usize,
    pub call_sites_replaced: usize,
    pub statements_saved: usize,
}

/// Abstract normalized operation representation for content-addressed hashing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NormalizedOp {
    Binary(String),
    Unary(String),
    ConstInt(i64),
    ConstFloat(u64),
    ConstBool(bool),
    ConstString(String),
    Use,
    Call(String),
    Array(usize),
    Struct(String, usize),
    Load,
    Alloc,
    FnPtr(String),
    Other(String),
}

/// Normalized instruction where register names are canonicalized to sequential indices.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NormalizedStatement {
    pub dest_reg: usize,
    pub op: NormalizedOp,
    pub arg_regs: Vec<usize>,
}

/// Normalized view of a basic block or statement sequence.
#[derive(Debug, Clone)]
pub struct NormalizedBlock {
    pub statements: Vec<NormalizedStatement>,
    pub hash: String,
    pub original_fn: String,
    pub block_id: BasicBlockId,
    pub start_stmt: usize,
    pub len: usize,
    pub live_in: Vec<String>,
    pub live_out: Vec<String>,
}

/// Helper for content-addressed hashing of normalized MIR instruction sequences.
pub struct BlockHasher;

impl BlockHasher {
    /// Normalizes a slice of MIR statements into canonical register indices (%0, %1, ...).
    pub fn normalize_statements(
        stmts: &[Statement],
    ) -> (Vec<NormalizedStatement>, HashMap<String, usize>) {
        let mut local_map: HashMap<String, usize> = HashMap::new();
        let mut next_id = 0usize;

        let mut get_reg = |name: &str, map: &mut HashMap<String, usize>| -> usize {
            if let Some(&id) = map.get(name) {
                id
            } else {
                let id = next_id;
                next_id += 1;
                map.insert(name.to_string(), id);
                id
            }
        };

        let mut normalized = Vec::with_capacity(stmts.len());

        for stmt in stmts {
            match stmt {
                Statement::Assign(dest, rval) => {
                    // Extract read registers first
                    let reads = get_rvalue_reads(rval);
                    let mut arg_regs = Vec::with_capacity(reads.len());
                    for r in reads {
                        arg_regs.push(get_reg(&r, &mut local_map));
                    }

                    // Extract dest register
                    let dest_reg = get_reg(&dest.local, &mut local_map);

                    let op = match rval {
                        Rvalue::BinaryOp(bin_op, _, _) => {
                            NormalizedOp::Binary(format!("{:?}", bin_op))
                        }
                        Rvalue::UnaryOp(un_op, _) => NormalizedOp::Unary(format!("{:?}", un_op)),
                        Rvalue::Constant(lit) => match lit {
                            crate::typecheck::typed_ast::TypedLiteral::Int(i, _) => {
                                NormalizedOp::ConstInt(*i)
                            }
                            crate::typecheck::typed_ast::TypedLiteral::Float(f, _) => {
                                NormalizedOp::ConstFloat(f.to_bits())
                            }
                            crate::typecheck::typed_ast::TypedLiteral::Bool(b) => {
                                NormalizedOp::ConstBool(*b)
                            }
                            crate::typecheck::typed_ast::TypedLiteral::Str(s) => {
                                NormalizedOp::ConstString(s.clone())
                            }
                        },
                        Rvalue::Use(_) => NormalizedOp::Use,
                        Rvalue::Call(callee, _) => NormalizedOp::Call(callee.clone()),
                        Rvalue::Array(elems) => NormalizedOp::Array(elems.len()),
                        Rvalue::Struct(name, fields) => {
                            NormalizedOp::Struct(name.clone(), fields.len())
                        }
                        Rvalue::Load(_) => NormalizedOp::Load,
                        Rvalue::Alloc(_) => NormalizedOp::Alloc,
                        Rvalue::FnPtr(name) => NormalizedOp::FnPtr(name.clone()),
                        other => NormalizedOp::Other(format!("{:?}", other)),
                    };

                    normalized.push(NormalizedStatement {
                        dest_reg,
                        op,
                        arg_regs,
                    });
                }
            }
        }

        (normalized, local_map)
    }

    /// Computes the SHA-256 hash of a normalized statement sequence.
    pub fn hash_normalized_sequence(stmts: &[NormalizedStatement]) -> String {
        let mut hasher = Sha256::new();
        for s in stmts {
            hasher.update((s.dest_reg as u64).to_le_bytes());
            match &s.op {
                NormalizedOp::Binary(b) => {
                    hasher.update(b"BIN:");
                    hasher.update(b.as_bytes());
                }
                NormalizedOp::Unary(u) => {
                    hasher.update(b"UN:");
                    hasher.update(u.as_bytes());
                }
                NormalizedOp::ConstInt(val) => {
                    hasher.update(b"INT:");
                    hasher.update(val.to_le_bytes());
                }
                NormalizedOp::ConstFloat(bits) => {
                    hasher.update(b"FLOAT:");
                    hasher.update(bits.to_le_bytes());
                }
                NormalizedOp::ConstBool(b) => {
                    hasher.update(b"BOOL:");
                    hasher.update([*b as u8]);
                }
                NormalizedOp::ConstString(str_val) => {
                    hasher.update(b"STR:");
                    hasher.update(str_val.as_bytes());
                }
                NormalizedOp::Use => {
                    hasher.update(b"USE");
                }
                NormalizedOp::Call(callee) => {
                    hasher.update(b"CALL:");
                    hasher.update(callee.as_bytes());
                }
                NormalizedOp::Array(n) => {
                    hasher.update(b"ARR:");
                    hasher.update((*n as u64).to_le_bytes());
                }
                NormalizedOp::Struct(name, n) => {
                    hasher.update(b"STRUCT:");
                    hasher.update(name.as_bytes());
                    hasher.update((*n as u64).to_le_bytes());
                }
                NormalizedOp::Load => {
                    hasher.update(b"LOAD");
                }
                NormalizedOp::Alloc => {
                    hasher.update(b"ALLOC");
                }
                NormalizedOp::FnPtr(name) => {
                    hasher.update(b"FNPTR:");
                    hasher.update(name.as_bytes());
                }
                NormalizedOp::Other(s) => {
                    hasher.update(b"OTHER:");
                    hasher.update(s.as_bytes());
                }
            }
            for arg in &s.arg_regs {
                hasher.update((*arg as u64).to_le_bytes());
            }
        }
        format!("{:x}", hasher.finalize())
    }
}

/// Computes sequence similarity between two normalized instruction sequences
/// using the Longest Common Subsequence (LCS) ratio:
///   similarity = 2 * LCS(A, B) / (|A| + |B|)
pub fn compute_sequence_similarity(a: &[NormalizedStatement], b: &[NormalizedStatement]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }

    let m = a.len();
    let n = b.len();

    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..m {
        for j in 0..n {
            if a[i] == b[j] {
                dp[i + 1][j + 1] = dp[i][j] + 1;
            } else {
                dp[i + 1][j + 1] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    let lcs = dp[m][n];
    (2.0 * lcs as f64) / (m + n) as f64
}

fn get_place_reads(p: &Place) -> Vec<String> {
    let mut reads = vec![p.local.clone()];
    for proj in &p.projections {
        if let Projection::Index(idx_place) = proj {
            reads.extend(get_place_reads(idx_place));
        }
    }
    reads
}

fn get_rvalue_reads(rval: &Rvalue) -> Vec<String> {
    match rval {
        Rvalue::Use(p)
        | Rvalue::UnaryOp(_, p)
        | Rvalue::Discriminant(p)
        | Rvalue::Alloc(p)
        | Rvalue::Load(p) => get_place_reads(p),
        Rvalue::BinaryOp(_, p1, p2) => {
            let mut r = get_place_reads(p1);
            r.extend(get_place_reads(p2));
            r
        }
        Rvalue::Constant(_) | Rvalue::FnPtr(_) => vec![],
        Rvalue::Call(_, args) => args.iter().flat_map(get_place_reads).collect(),
        Rvalue::Array(elems) => elems.iter().flat_map(get_place_reads).collect(),
        Rvalue::Struct(_, fields) => fields
            .iter()
            .flat_map(|(_, p)| get_place_reads(p))
            .collect(),
        Rvalue::EnumVariant { fields, .. } => fields.iter().flat_map(get_place_reads).collect(),
        Rvalue::Phi(branches) => branches
            .iter()
            .flat_map(|(_, p)| get_place_reads(p))
            .collect(),
        Rvalue::ClosureAlloc { captured, .. } => {
            captured.iter().flat_map(get_place_reads).collect()
        }
        Rvalue::Thunk { env, .. } => env.clone(),
    }
}

fn get_terminator_reads(term: &Terminator) -> Vec<String> {
    match term {
        Terminator::BranchIf { condition, .. } => get_place_reads(condition),
        Terminator::Switch { value, .. } => get_place_reads(value),
        Terminator::Return { value } => {
            if let Some(p) = value {
                get_place_reads(p)
            } else {
                vec![]
            }
        }
        Terminator::IndirectCall { callee, args, .. } => {
            let mut r = get_place_reads(callee);
            r.extend(args.iter().flat_map(get_place_reads));
            r
        }
        Terminator::Force { thunk, .. } => vec![thunk.clone()],
        Terminator::TypeGuard { local, .. } => get_place_reads(local),
        Terminator::Branch { .. } | Terminator::Unreachable | Terminator::Fork { .. } => vec![],
    }
}

/// Computes the live-in and live-out variables for a statement sequence within a function.
pub fn compute_live_variables(
    func: &MirFunction,
    block_id: &BasicBlockId,
    start_stmt: usize,
    len: usize,
) -> (Vec<String>, Vec<String>) {
    let block = match func.blocks.iter().find(|b| &b.id == block_id) {
        Some(b) => b,
        None => return (vec![], vec![]),
    };

    let slice = if start_stmt + len <= block.statements.len() {
        &block.statements[start_stmt..start_stmt + len]
    } else {
        return (vec![], vec![]);
    };

    let mut defined_in_seq = HashSet::new();
    let mut live_in = Vec::new();

    for stmt in slice {
        match stmt {
            Statement::Assign(dest, rval) => {
                for r in get_rvalue_reads(rval) {
                    if !defined_in_seq.contains(&r) && !live_in.contains(&r) {
                        live_in.push(r);
                    }
                }
                for r in get_place_reads(dest) {
                    if r != dest.local && !defined_in_seq.contains(&r) && !live_in.contains(&r) {
                        live_in.push(r);
                    }
                }
                defined_in_seq.insert(dest.local.clone());
            }
        }
    }

    // Collect all reads after the sequence in the same block
    let mut reads_after = HashSet::new();
    for stmt in &block.statements[start_stmt + len..] {
        match stmt {
            Statement::Assign(dest, rval) => {
                reads_after.extend(get_rvalue_reads(rval));
                for r in get_place_reads(dest) {
                    if r != dest.local {
                        reads_after.insert(r);
                    }
                }
            }
        }
    }
    reads_after.extend(get_terminator_reads(&block.terminator));

    // Collect reads across all other blocks in the function
    for b in &func.blocks {
        if &b.id != block_id {
            for stmt in &b.statements {
                match stmt {
                    Statement::Assign(dest, rval) => {
                        reads_after.extend(get_rvalue_reads(rval));
                        for r in get_place_reads(dest) {
                            if r != dest.local {
                                reads_after.insert(r);
                            }
                        }
                    }
                }
            }
            reads_after.extend(get_terminator_reads(&b.terminator));
        }
    }

    let mut live_out = Vec::new();
    for def in &defined_in_seq {
        if reads_after.contains(def) {
            live_out.push(def.clone());
        }
    }
    // Deterministic ordering
    live_out.sort();

    (live_in, live_out)
}

/// Identifies duplicate instruction sequences across basic blocks and extracts
/// them into shared outlined subroutines `__nl_outlined_<hash[0..8]>`.
pub fn outline_program(mir: &mut MirProgram, config: &OutlinerConfig) -> OutlinerStats {
    let mut stats = OutlinerStats::default();

    // Step 1: Collect normalized candidate blocks/sequences
    let mut candidates: Vec<NormalizedBlock> = Vec::new();

    for func in &mir.functions {
        if func.name.starts_with("__nl_outlined_") {
            continue;
        }

        for block in &func.blocks {
            let stmt_len = block.statements.len();
            if stmt_len >= config.min_sequence_length {
                let (norm_stmts, _) = BlockHasher::normalize_statements(&block.statements);
                let hash = BlockHasher::hash_normalized_sequence(&norm_stmts);
                let (live_in, live_out) = compute_live_variables(func, &block.id, 0, stmt_len);

                candidates.push(NormalizedBlock {
                    statements: norm_stmts,
                    hash,
                    original_fn: func.name.clone(),
                    block_id: block.id.clone(),
                    start_stmt: 0,
                    len: stmt_len,
                    live_in,
                    live_out,
                });
            }
        }
    }

    stats.candidate_sequences_found = candidates.len();

    // Step 2: Group identical sequences by content-addressed hash
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, cand) in candidates.iter().enumerate() {
        groups.entry(cand.hash.clone()).or_default().push(idx);
    }

    // Step 3: Extract duplicate groups (>= 2 occurrences) into outlined subroutines
    let mut outlined_fns: Vec<MirFunction> = Vec::new();

    for (hash, indices) in groups {
        if indices.len() < 2 {
            continue;
        }
        if stats.outlined_functions_created >= config.max_outlined_functions {
            break;
        }

        let first_idx = indices[0];
        let rep = &candidates[first_idx];

        // Skip candidate if more than 1 variable is live-out
        if rep.live_out.len() > 1 {
            continue;
        }

        let outlined_fn_name = format!("__nl_outlined_{}", &hash[0..8.min(hash.len())]);

        // Find representative function to determine parameter types
        let rep_func = match mir.functions.iter().find(|f| f.name == rep.original_fn) {
            Some(f) => f,
            None => continue,
        };

        let mut param_types = Vec::with_capacity(rep.live_in.len());
        for var in &rep.live_in {
            let ty = rep_func
                .locals
                .iter()
                .find(|l| &l.name == var)
                .map(|l| l.ty.clone())
                .or_else(|| {
                    rep_func
                        .params
                        .iter()
                        .find(|(p_name, _)| p_name == var)
                        .map(|(_, ty)| ty.clone())
                })
                .unwrap_or(Type::I64);
            param_types.push(ty);
        }

        let return_ty = if rep.live_out.len() == 1 {
            let out_var = &rep.live_out[0];
            rep_func
                .locals
                .iter()
                .find(|l| &l.name == out_var)
                .map(|l| l.ty.clone())
                .unwrap_or(Type::I64)
        } else {
            Type::Void
        };

        let params: Vec<(String, Type)> = rep
            .live_in
            .iter()
            .enumerate()
            .map(|(i, _)| (format!("arg_{}", i), param_types[i].clone()))
            .collect();

        // Build substitution map for live-in variables
        let var_subst: HashMap<String, String> = rep
            .live_in
            .iter()
            .enumerate()
            .map(|(i, orig)| (orig.clone(), format!("arg_{}", i)))
            .collect();

        // Clone and rename statements for the outlined function body
        let rep_block = match rep_func.blocks.iter().find(|b| b.id == rep.block_id) {
            Some(b) => b,
            None => continue,
        };

        let mut outlined_stmts = Vec::with_capacity(rep.len);
        for stmt in &rep_block.statements[rep.start_stmt..rep.start_stmt + rep.len] {
            let mut s = stmt.clone();
            substitute_stmt_locals(&mut s, &var_subst);
            outlined_stmts.push(s);
        }

        let terminator = if rep.live_out.len() == 1 {
            let out_local = var_subst
                .get(&rep.live_out[0])
                .cloned()
                .unwrap_or_else(|| rep.live_out[0].clone());
            Terminator::Return {
                value: Some(Place {
                    local: out_local,
                    projections: vec![],
                }),
            }
        } else {
            Terminator::Return { value: None }
        };

        let mut outlined_locals = rep_func.locals.clone();
        for (p_name, p_ty) in &params {
            if !outlined_locals.iter().any(|l| &l.name == p_name) {
                outlined_locals.push(MirLocalDecl {
                    name: p_name.clone(),
                    ty: p_ty.clone(),
                    mutable: false,
                });
            }
        }

        let outlined_fn = MirFunction {
            name: outlined_fn_name.clone(),
            params,
            return_ty: return_ty.clone(),
            locals: outlined_locals,
            blocks: vec![MirBasicBlock {
                id: BasicBlockId(0),
                arguments: vec![],
                statements: outlined_stmts,
                terminator,
            }],
            is_distilled: false,
        };

        outlined_fns.push(outlined_fn);
        stats.outlined_functions_created += 1;

        // Step 4: Replace all occurrence sites with call to the outlined subroutine
        for &idx in &indices {
            let cand = &candidates[idx];
            if let Some(target_func) = mir
                .functions
                .iter_mut()
                .find(|f| f.name == cand.original_fn)
            {
                if let Some(target_block) = target_func
                    .blocks
                    .iter_mut()
                    .find(|b| b.id == cand.block_id)
                {
                    let call_args: Vec<Place> = cand
                        .live_in
                        .iter()
                        .map(|v| Place {
                            local: v.clone(),
                            projections: vec![],
                        })
                        .collect();

                    let replacement_stmt = if cand.live_out.len() == 1 {
                        Statement::Assign(
                            Place {
                                local: cand.live_out[0].clone(),
                                projections: vec![],
                            },
                            Rvalue::Call(outlined_fn_name.clone(), call_args),
                        )
                    } else {
                        if !target_func.locals.iter().any(|l| l.name == "_t_outl_dummy") {
                            target_func.locals.push(MirLocalDecl {
                                name: "_t_outl_dummy".to_string(),
                                ty: Type::I64,
                                mutable: false,
                            });
                        }
                        Statement::Assign(
                            Place {
                                local: "_t_outl_dummy".to_string(),
                                projections: vec![],
                            },
                            Rvalue::Call(outlined_fn_name.clone(), call_args),
                        )
                    };

                    // Replace original statements
                    let end_stmt = cand.start_stmt + cand.len;
                    if end_stmt <= target_block.statements.len() {
                        target_block
                            .statements
                            .splice(cand.start_stmt..end_stmt, std::iter::once(replacement_stmt));
                        stats.call_sites_replaced += 1;
                        stats.statements_saved += cand.len.saturating_sub(1);
                    }
                }
            }
        }
    }

    // Append all newly created outlined functions to MirProgram
    mir.functions.extend(outlined_fns);

    stats
}

fn substitute_place_local(p: &mut Place, subst: &HashMap<String, String>) {
    if let Some(new_name) = subst.get(&p.local) {
        p.local = new_name.clone();
    }
    for proj in &mut p.projections {
        if let Projection::Index(idx_place) = proj {
            substitute_place_local(idx_place, subst);
        }
    }
}

fn substitute_stmt_locals(stmt: &mut Statement, subst: &HashMap<String, String>) {
    match stmt {
        Statement::Assign(dest, rval) => {
            substitute_place_local(dest, subst);
            match rval {
                Rvalue::Use(p)
                | Rvalue::UnaryOp(_, p)
                | Rvalue::Discriminant(p)
                | Rvalue::Alloc(p)
                | Rvalue::Load(p) => {
                    substitute_place_local(p, subst);
                }
                Rvalue::BinaryOp(_, p1, p2) => {
                    substitute_place_local(p1, subst);
                    substitute_place_local(p2, subst);
                }
                Rvalue::Call(_, args) | Rvalue::Array(args) => {
                    for a in args {
                        substitute_place_local(a, subst);
                    }
                }
                Rvalue::Struct(_, fields) => {
                    for (_, p) in fields {
                        substitute_place_local(p, subst);
                    }
                }
                Rvalue::EnumVariant { fields, .. } => {
                    for p in fields {
                        substitute_place_local(p, subst);
                    }
                }
                Rvalue::Phi(branches) => {
                    for (_, p) in branches {
                        substitute_place_local(p, subst);
                    }
                }
                Rvalue::ClosureAlloc { captured, .. } => {
                    for p in captured {
                        substitute_place_local(p, subst);
                    }
                }
                Rvalue::Thunk { env, .. } => {
                    for v in env {
                        if let Some(new_name) = subst.get(v) {
                            *v = new_name.clone();
                        }
                    }
                }
                Rvalue::Constant(_) | Rvalue::FnPtr(_) => {}
            }
        }
    }
}
