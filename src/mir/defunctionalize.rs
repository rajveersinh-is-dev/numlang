//! Whole-program type-directed Reynolds defunctionalization (Reynolds 1972).
//!
//! Compiles higher-order closures and indirect calls into zero-allocation,
//! monomorphic, first-order SSA forms with static dispatch:
//!
//! 1. Discovers all closures (`Rvalue::ClosureAlloc`) and groups them by call signature.
//! 2. Synthesizes a discriminated union enum `ClosureTag_<Signature>` per distinct signature.
//! 3. Rewrites `Rvalue::ClosureAlloc` into typed `Rvalue::EnumVariant` carrying captured environment payloads.
//! 4. Lowers `Terminator::IndirectCall` into direct `Terminator::Switch` over tags, dispatching
//!    to monomorphic static `Rvalue::Call` sites in specialized basic blocks.

use std::collections::BTreeMap;
use crate::span::Span;
use crate::mir::{BasicBlockId, Place, Projection, Terminator};
use crate::mir::lower::{MirBasicBlock, MirLocalDecl, MirProgram, Rvalue, Statement};
use crate::typecheck::typed_ast::{TypedEnumDef, TypedEnumVariant};
use crate::typecheck::types::Type;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClosureSignature {
    pub param_tys: Vec<Type>,
    pub return_ty: Type,
}

#[derive(Debug, Clone)]
struct ClosureCandidate {
    fn_name: String,
    captured_types: Vec<Type>,
}

#[derive(Debug, Clone)]
struct ClosureGroup {
    signature: ClosureSignature,
    candidates: Vec<ClosureCandidate>,
}

#[derive(Debug, Clone)]
struct ResolvedVariant {
    fn_name: String,
    tag: usize,
    captured_types: Vec<Type>,
}

#[derive(Debug, Clone)]
struct SignatureGroup {
    signature: ClosureSignature,
    variants: Vec<ResolvedVariant>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefunctionalizeStats {
    pub closures_defunctionalized: usize,
    pub indirect_calls_resolved: usize,
    pub synthetic_enums_created: usize,
}

/// Executes whole-program Reynolds defunctionalization on the given MIR program.
pub fn defunctionalize_program(program: &mut MirProgram) -> DefunctionalizeStats {
    let mut stats = DefunctionalizeStats::default();

    // 1. Scan for all closure allocations to discover signatures and variants
    let mut closure_allocs: Vec<(String, usize)> = Vec::new(); // (fn_name, captured_count)
    for func in &program.functions {
        for block in &func.blocks {
            for stmt in &block.statements {
                let Statement::Assign(_, rval) = stmt;
                if let Rvalue::ClosureAlloc { fn_name, captured } = rval {
                    if !closure_allocs.iter().any(|(n, _)| n == fn_name) {
                        closure_allocs.push((fn_name.clone(), captured.len()));
                    }
                }
            }
        }
    }

    if closure_allocs.is_empty() {
        return stats;
    }

    // 2. Map closure functions to their callable signatures and captured types
    let mut groups: Vec<ClosureGroup> = Vec::new();
    for (fn_name, cap_count) in &closure_allocs {
        if let Some(target_fn) = program.functions.iter().find(|f| &f.name == fn_name) {
            let cap_tys: Vec<Type> = target_fn.params[0..*cap_count]
                .iter()
                .map(|(_, ty)| ty.clone())
                .collect();
            let callable_tys: Vec<Type> = target_fn.params[*cap_count..]
                .iter()
                .map(|(_, ty)| ty.clone())
                .collect();
            let sig = ClosureSignature {
                param_tys: callable_tys,
                return_ty: target_fn.return_ty.clone(),
            };
            let cand = ClosureCandidate {
                fn_name: fn_name.clone(),
                captured_types: cap_tys,
            };
            if let Some(grp) = groups.iter_mut().find(|g| g.signature == sig) {
                grp.candidates.push(cand);
            } else {
                groups.push(ClosureGroup {
                    signature: sig,
                    candidates: vec![cand],
                });
            }
        }
    }

    // 3. Synthesize discriminated union sum types (enums)
    // Map fn_name -> (enum_name, variant_name, tag, captured_tys)
    let mut variant_map: BTreeMap<String, (String, String, usize, Vec<Type>)> = BTreeMap::new();
    let mut sig_groups: Vec<SignatureGroup> = Vec::new();

    for (sig_idx, group) in groups.iter().enumerate() {
        let enum_name = format!("ClosureTag_{}_{}", sig_idx, group.signature.param_tys.len());

        let mut enum_variants = Vec::new();
        let mut resolved_variants = Vec::new();
        for (tag, cand) in group.candidates.iter().enumerate() {
            let variant_name = format!("Variant_{}", cand.fn_name);
            enum_variants.push(TypedEnumVariant {
                name: variant_name.clone(),
                tag,
                payload: cand.captured_types.clone(),
                span: Span { start: 0, end: 0 },
            });
            variant_map.insert(
                cand.fn_name.clone(),
                (enum_name.clone(), variant_name, tag, cand.captured_types.clone()),
            );
            resolved_variants.push(ResolvedVariant {
                fn_name: cand.fn_name.clone(),
                tag,
                captured_types: cand.captured_types.clone(),
            });
            stats.closures_defunctionalized += 1;
        }

        sig_groups.push(SignatureGroup {
            signature: group.signature.clone(),
            variants: resolved_variants,
        });

        program.enums.push(TypedEnumDef {
            name: enum_name,
            variants: enum_variants,
            span: Span { start: 0, end: 0 },
        });
        stats.synthetic_enums_created += 1;
    }

    // 4. Rewrite Rvalue::ClosureAlloc into Rvalue::EnumVariant
    for func in &mut program.functions {
        for block in &mut func.blocks {
            for stmt in &mut block.statements {
                let Statement::Assign(_, rval) = stmt;
                if let Rvalue::ClosureAlloc { fn_name, captured } = rval {
                    if let Some((enum_name, variant_name, tag, _)) = variant_map.get(fn_name) {
                        *rval = Rvalue::EnumVariant {
                            enum_name: enum_name.clone(),
                            variant_name: variant_name.clone(),
                            tag: *tag,
                            fields: captured.clone(),
                        };
                    }
                }
            }
        }
    }

    // 5. Rewrite Terminator::IndirectCall into direct static dispatch Switch
    for func in &mut program.functions {
        let mut new_blocks: Vec<MirBasicBlock> = Vec::new();
        let mut max_bb = func.blocks.iter().map(|b| b.id.0).max().unwrap_or(0) + 1;

        for block in &mut func.blocks {
            if let Terminator::IndirectCall { callee, args, dest, next } = &block.terminator {
                // Find matching signature by arity
                let matching_group = sig_groups.iter().find(|g| {
                    g.signature.param_tys.len() == args.len()
                });

                if let Some(group) = matching_group {
                    let tag_place = Place {
                        local: format!("_dt_tag_{}_{}", callee.local, block.id.0),
                        projections: vec![],
                    };
                    func.locals.push(MirLocalDecl {
                        name: tag_place.local.clone(),
                        ty: Type::I64,
                        mutable: true,
                    });

                    // Tag read statement via Discriminant
                    block.statements.push(Statement::Assign(
                        tag_place.clone(),
                        Rvalue::Discriminant(callee.clone()),
                    ));

                    let mut targets = Vec::new();
                    for variant in &group.variants {
                        let arm_bb_id = BasicBlockId(max_bb);
                        max_bb += 1;
                        let mut arm_stmts = Vec::new();
                        let mut call_args = Vec::with_capacity(variant.captured_types.len() + args.len());

                        // Unpack captured environment payload fields
                        for (j, cap_ty) in variant.captured_types.iter().enumerate() {
                            let cap_temp = Place {
                                local: format!("_dt_cap_{}_{}_{}", variant.fn_name, j, arm_bb_id.0),
                                projections: vec![],
                            };
                            func.locals.push(MirLocalDecl {
                                name: cap_temp.local.clone(),
                                ty: cap_ty.clone(),
                                mutable: true,
                            });
                            let mut proj_place = callee.clone();
                            proj_place.projections.push(Projection::Payload(j));
                            arm_stmts.push(Statement::Assign(cap_temp.clone(), Rvalue::Use(proj_place)));
                            call_args.push(cap_temp);
                        }

                        // Pass remaining args
                        for arg in args {
                            call_args.push(arg.clone());
                        }

                        // Direct monomorphic call
                        arm_stmts.push(Statement::Assign(
                            dest.clone(),
                            Rvalue::Call(variant.fn_name.clone(), call_args),
                        ));

                        new_blocks.push(MirBasicBlock {
                            id: arm_bb_id.clone(),
                            arguments: vec![],
                            statements: arm_stmts,
                            terminator: Terminator::Branch { target: next.clone() },
                        });

                        targets.push((variant.tag as i64, arm_bb_id));
                    }

                    let default_bb_id = BasicBlockId(max_bb);
                    max_bb += 1;
                    new_blocks.push(MirBasicBlock {
                        id: default_bb_id.clone(),
                        arguments: vec![],
                        statements: vec![],
                        terminator: Terminator::Unreachable,
                    });

                    block.terminator = Terminator::Switch {
                        value: tag_place,
                        targets,
                        default: default_bb_id,
                    };

                    stats.indirect_calls_resolved += 1;
                }
            }
        }

        func.blocks.extend(new_blocks);
    }

    stats
}
