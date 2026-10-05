use std::collections::HashMap;

use crate::typecheck::typed_ast::TypedLiteral;
use crate::mir::lower::{MirFunction, Rvalue, Statement};
use crate::mir::{Place, Projection};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AliasResult {
    /// The two memory places never refer to the same memory location.
    NoAlias,
    /// The two memory places might refer to the same memory location.
    MayAlias,
    /// The two memory places are guaranteed to refer to the exact same memory location.
    MustAlias,
}

impl AliasResult {
    pub fn is_no_alias(self) -> bool {
        self == AliasResult::NoAlias
    }

    pub fn is_must_alias(self) -> bool {
        self == AliasResult::MustAlias
    }

    pub fn is_may_alias(self) -> bool {
        self == AliasResult::MayAlias
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModRefResult {
    NoModRef,
    Ref,
    Mod,
    ModRef,
}

impl ModRefResult {
    pub fn has_ref(self) -> bool {
        matches!(self, ModRefResult::Ref | ModRefResult::ModRef)
    }

    pub fn has_mod(self) -> bool {
        matches!(self, ModRefResult::Mod | ModRefResult::ModRef)
    }
}

#[derive(Debug, Clone)]
pub struct AliasAnalysis {
    /// Constant integer value table for local variables: local name -> constant value
    const_ints: HashMap<String, i64>,
}

impl AliasAnalysis {
    pub fn new(func: &MirFunction) -> Self {
        let mut const_ints = HashMap::new();

        // Scan statements for constant integer definitions
        for block in &func.blocks {
            for stmt in &block.statements {
                let Statement::Assign(dest, rval) = stmt;
                if dest.projections.is_empty() {
                    if let Rvalue::Constant(TypedLiteral::Int(val, _)) = rval {
                        const_ints.insert(dest.local.clone(), *val);
                    }
                }
            }
        }

        AliasAnalysis { const_ints }
    }

    /// Queries the alias relationship between two MIR memory places.
    pub fn alias(&self, p1: &Place, p2: &Place) -> AliasResult {
        // 1. If base locals differ:
        // In NumLang, distinct local variable names represent distinct stack allocations.
        if p1.local != p2.local {
            return AliasResult::NoAlias;
        }

        // 2. Both places have the same root local. Compare projections step-by-step.
        let min_len = p1.projections.len().min(p2.projections.len());
        for i in 0..min_len {
            match (&p1.projections[i], &p2.projections[i]) {
                (Projection::Field(f1), Projection::Field(f2)) => {
                    // Field sensitivity: distinct field names on the same struct are disjoint.
                    if f1 != f2 {
                        return AliasResult::NoAlias;
                    }
                }
                (Projection::Index(idx1), Projection::Index(idx2)) => {
                    // Array constant index analysis:
                    // Check if both indices are known constants
                    let c1 = self.resolve_constant_int(idx1);
                    let c2 = self.resolve_constant_int(idx2);

                    if let (Some(v1), Some(v2)) = (c1, c2) {
                        if v1 != v2 {
                            // Distinct constant array indices are provably disjoint!
                            return AliasResult::NoAlias;
                        }
                    } else if idx1 != idx2 {
                        // Dynamic or unknown indices on the same array base may alias
                        return AliasResult::MayAlias;
                    }
                }
                (Projection::Deref, Projection::Deref) => {
                    // Both dereferenced from the same base
                }
                // Incompatible projection types (e.g. Field vs Index) on the same level
                _ => return AliasResult::NoAlias,
            }
        }

        // 3. All common projections matched identically:
        if p1.projections.len() == p2.projections.len() {
            AliasResult::MustAlias
        } else {
            // One is a sub-path / prefix of the other (e.g., p vs p.x)
            AliasResult::MayAlias
        }
    }

    /// Resolves a place to a known constant integer if possible.
    fn resolve_constant_int(&self, place: &Place) -> Option<i64> {
        if place.projections.is_empty() {
            self.const_ints.get(&place.local).copied()
        } else {
            None
        }
    }

    /// Computes the ModRef effect of a statement with respect to a memory place `target`.
    pub fn modref(&self, stmt: &Statement, target: &Place) -> ModRefResult {
        let Statement::Assign(dest, rval) = stmt;

        let mut modifies = false;
        let mut references = false;

        // Check if destination aliases target
        let dest_alias = self.alias(dest, target);
        if dest_alias != AliasResult::NoAlias {
            modifies = true;
        }

        // Check if any read place aliases target
        let reads = collect_reads_for_alias(rval, dest);
        for read_p in reads {
            let read_alias = self.alias(&read_p, target);
            if read_alias != AliasResult::NoAlias {
                references = true;
                break;
            }
        }

        match (modifies, references) {
            (true, true) => ModRefResult::ModRef,
            (true, false) => ModRefResult::Mod,
            (false, true) => ModRefResult::Ref,
            (false, false) => ModRefResult::NoModRef,
        }
    }
}

fn collect_reads_for_alias(rv: &Rvalue, dest: &Place) -> Vec<Place> {
    let mut reads = Vec::new();

    for proj in &dest.projections {
        if let Projection::Index(idx_place) = proj {
            reads.push((**idx_place).clone());
        }
    }

    match rv {
        Rvalue::Use(p) => reads.push(p.clone()),
        Rvalue::BinaryOp(_, p1, p2) => {
            reads.push(p1.clone());
            reads.push(p2.clone());
        }
        Rvalue::UnaryOp(_, p) => reads.push(p.clone()),
        Rvalue::Constant(_) => {}
        Rvalue::Call(_, args) => {
            reads.extend(args.iter().cloned());
        }
        Rvalue::Array(elems) => {
            reads.extend(elems.iter().cloned());
        }
        Rvalue::Struct(_, fields) => {
            for (_, p) in fields {
                reads.push(p.clone());
            }
        }
        Rvalue::EnumVariant { fields, .. } => {
            reads.extend(fields.iter().cloned());
        }
        Rvalue::Discriminant(p) => {
            reads.push(p.clone());
        }
        Rvalue::Phi(incoming) => {
            for (_, p) in incoming {
                reads.push(p.clone());
            }
        }
        Rvalue::FnPtr(_) => {}
        Rvalue::ClosureAlloc { captured, .. } => {
            reads.extend(captured.iter().cloned());
        }
        Rvalue::Alloc(p) | Rvalue::Load(p) => {
            reads.push(p.clone());
        }
        Rvalue::Thunk { env, .. } => {
            for e in env {
                reads.push(Place { local: e.clone(), projections: vec![] });
            }
        }
    }

    reads
}
