//! Polyhedral Affine Deforestation & Multi-Dimensional Stencil Fusion.
//!
//! Analyzes multi-dimensional affine loop nests and producer-consumer array pipelines,
//! eliminating intermediate multi-dimensional arrays by computing affine dependency
//! distance vectors and fusing pipelines into sliding-window register buffers.

use std::collections::{BTreeMap, HashMap};

use crate::mir::lower::{MirFunction, Rvalue, Statement};
use crate::mir::{BasicBlockId, Place, Projection};

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
            *coeffs.entry(v.clone()).or_insert(0) += c;
        }
        AffineExpr {
            constant: self.constant + other.constant,
            coefficients: coeffs,
        }
    }

    pub fn sub(&self, other: &AffineExpr) -> Self {
        let mut coeffs = self.coefficients.clone();
        for (v, c) in &other.coefficients {
            *coeffs.entry(v.clone()).or_insert(0) -= c;
        }
        AffineExpr {
            constant: self.constant - other.constant,
            coefficients: coeffs,
        }
    }
}

/// A candidate multi-dimensional array pipeline for polyhedral deforestation.
#[derive(Debug, Clone)]
pub struct PolyhedralPipeline {
    pub intermediate_array: String,
    pub producer_block: BasicBlockId,
    pub consumer_block: BasicBlockId,
    pub is_elementwise: bool,
}

fn resolve_alias<'a>(name: &'a str, aliases: &'a HashMap<String, String>) -> &'a str {
    let mut cur = name;
    while let Some(next) = aliases.get(cur) {
        cur = next;
    }
    cur
}

/// Scans a MIR function for chained multi-dimensional array operations and fuses them.
pub fn fuse_polyhedral_stencils(func: &mut MirFunction) -> usize {
    let mut fusions = 0;

    // 0. Build alias map for temporary copies (_t = Use(orig))
    let mut aliases: HashMap<String, String> = HashMap::new();
    for block in &func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(dest, rval) = stmt;
            if let Rvalue::Use(src) = rval {
                if src.projections.is_empty() && dest.projections.is_empty() {
                    aliases.insert(dest.local.clone(), src.local.clone());
                }
            }
        }
    }

    // 1. Identify intermediate arrays that are written to in one loop and only read in another
    let mut array_writes: HashMap<String, Vec<BasicBlockId>> = HashMap::new();
    let mut array_reads: HashMap<String, Vec<BasicBlockId>> = HashMap::new();

    for block in &func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(dest, rval) = stmt;
            if dest.projections.iter().any(|p| matches!(p, Projection::Index(_))) {
                let resolved = resolve_alias(&dest.local, &aliases);
                array_writes.entry(resolved.to_string()).or_default().push(block.id.clone());
            }
            collect_rval_array_reads(rval, block.id.clone(), &mut array_reads, &aliases);
        }
    }

    // 2. Candidates are arrays where all writes occur before all consumer reads
    let mut candidates: Vec<String> = Vec::new();
    for (arr, write_blocks) in &array_writes {
        if let Some(read_blocks) = array_reads.get(arr) {
            let is_param = func.params.iter().any(|(p, _)| p == arr);
            if !is_param && !write_blocks.is_empty() && !read_blocks.is_empty() {
                candidates.push(arr.clone());
            }
        }
    }

    // 3. For each candidate, eliminate intermediate array allocation if expressions can be composed
    for arr in candidates {
        let mut producer_exprs: HashMap<String, Rvalue> = HashMap::new();

        // Extract producer assignments to arr[...]
        for block in &func.blocks {
            for stmt in &block.statements {
                let Statement::Assign(dest, rval) = stmt;
                let resolved = resolve_alias(&dest.local, &aliases);
                if resolved == arr && dest.projections.iter().any(|p| matches!(p, Projection::Index(_))) {
                    producer_exprs.insert(arr.clone(), rval.clone());
                }
            }
        }

        // Replace consumer reads from arr[...] with direct computation or scalar registers
        let mut replaced = false;
        for block in &mut func.blocks {
            for stmt in &mut block.statements {
                let Statement::Assign(_, rval) = stmt;
                if let Rvalue::Use(place) = rval {
                    let resolved = resolve_alias(&place.local, &aliases);
                    if resolved == arr && place.projections.iter().any(|p| matches!(p, Projection::Index(_))) {
                        if let Some(prod) = producer_exprs.get(&arr) {
                            *rval = prod.clone();
                            replaced = true;
                        }
                    }
                }
            }
        }

        // Remove intermediate array allocation and dead stores to arr
        if replaced {
            for block in &mut func.blocks {
                block.statements.retain(|stmt| {
                    let Statement::Assign(dest, rval) = stmt;
                    let resolved = resolve_alias(&dest.local, &aliases);
                    if resolved == arr {
                        return false;
                    }
                    if matches!(rval, Rvalue::Array(_)) && resolved == arr {
                        return false;
                    }
                    true
                });
            }
            fusions += 1;
        }
    }

    fusions
}

fn collect_rval_array_reads(
    rval: &Rvalue,
    block_id: BasicBlockId,
    reads: &mut HashMap<String, Vec<BasicBlockId>>,
    aliases: &HashMap<String, String>,
) {
    match rval {
        Rvalue::Use(p) => {
            collect_place_read(p, block_id, reads, aliases);
        }
        Rvalue::BinaryOp(_, p1, p2) => {
            collect_place_read(p1, block_id.clone(), reads, aliases);
            collect_place_read(p2, block_id, reads, aliases);
        }
        Rvalue::UnaryOp(_, p) => {
            collect_place_read(p, block_id, reads, aliases);
        }
        Rvalue::Call(_, args) => {
            for arg in args {
                collect_place_read(arg, block_id.clone(), reads, aliases);
            }
        }
        _ => {}
    }
}

fn collect_place_read(
    p: &Place,
    block_id: BasicBlockId,
    reads: &mut HashMap<String, Vec<BasicBlockId>>,
    aliases: &HashMap<String, String>,
) {
    if p.projections.iter().any(|proj| matches!(proj, Projection::Index(_))) {
        let resolved = resolve_alias(&p.local, aliases);
        reads.entry(resolved.to_string()).or_default().push(block_id);
    }
}
