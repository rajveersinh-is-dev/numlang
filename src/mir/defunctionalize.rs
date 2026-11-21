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

use crate::mir::lower::{MirBasicBlock, MirFunction, MirLocalDecl, MirProgram, Rvalue, Statement};
use crate::mir::{BasicBlockId, Place, Projection, Terminator};
use crate::span::Span;
use crate::typecheck::typed_ast::{TypedEnumDef, TypedEnumVariant};
use crate::typecheck::types::Type;
use std::collections::{BTreeMap, HashMap};

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
    /// Non-escaping closures that were inlined as direct calls instead of tagged.
    pub closures_inlined: usize,
    /// Escaping closures whose scalar captured fields were read directly from the enum payload
    /// (unboxed) at dispatch sites.
    pub scalar_payloads_unboxed: usize,
}

/// A closure allocation whose destination local never escapes.
struct NonEscapingClosure {
    fn_name: String,
    /// Fresh temporaries holding the captured values, copied at the allocation
    /// site so that the direct-call rewrite preserves allocation-time semantics.
    captured_copies: Vec<Place>,
}

/// Returns the type of the local/param named `name` in `func`, if known.
fn local_type_of(func: &MirFunction, name: &str) -> Type {
    func.locals
        .iter()
        .find(|l| l.name == name)
        .map(|l| l.ty.clone())
        .or_else(|| {
            func.params
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, t)| t.clone())
        })
        .unwrap_or(Type::I64)
}

/// Places referenced by an Rvalue, for escape analysis.
fn rvalue_places(rv: &Rvalue) -> Vec<&Place> {
    match rv {
        Rvalue::Use(p)
        | Rvalue::Discriminant(p)
        | Rvalue::Alloc(p)
        | Rvalue::Load(p) => vec![p],
        Rvalue::BinaryOp(_, a, b) => vec![a, b],
        Rvalue::Select(c, t, e) => vec![c, t, e],
        Rvalue::UnaryOp(_, p) => vec![p],
        Rvalue::Call(_, args) => args.iter().collect(),
        Rvalue::Array(elems) => elems.iter().collect(),
        Rvalue::Struct(_, fields) => fields.iter().map(|(_, p)| p).collect(),
        Rvalue::EnumVariant { fields, .. } => fields.iter().collect(),
        Rvalue::Phi(incoming) => incoming.iter().map(|(_, p)| p).collect(),
        Rvalue::ClosureAlloc { captured, .. } => captured.iter().collect(),
        Rvalue::FnPtr(_) | Rvalue::Constant(_) | Rvalue::Thunk { .. } => vec![],
    }
}

/// Places referenced by a Terminator, for escape analysis.
fn terminator_places(t: &Terminator) -> Vec<&Place> {
    match t {
        Terminator::Branch { .. } => vec![],
        Terminator::BranchIf { condition, .. } => vec![condition],
        Terminator::Switch { value, .. } => vec![value],
        Terminator::Return { value: Some(v) } => vec![v],
        Terminator::Return { value: None } => vec![],
        Terminator::Unreachable => vec![],
        Terminator::IndirectCall { callee, .. } => vec![callee],
        Terminator::Fork { .. } => vec![],
        Terminator::Force { .. } => vec![],
        Terminator::TypeGuard { local, .. } => vec![local],
    }
}

/// How a local is used by other statements/terminators (for escape analysis).
#[derive(Clone, Debug, PartialEq, Eq)]
enum UseKind {
    /// The local is the callee of an `IndirectCall` (a direct dispatch).
    Dispatch,
    /// The local is read into another local via `Rvalue::Use` (a copy); the
    /// payload is `String` naming the destination local.
    ValueRead(String),
    /// Any other use (stores into structs/arrays, passing to calls, forcing, etc.).
    Other,
}

/// Collects the uses of every local: `local -> Vec<UseKind>` (one entry per use).
/// The statement/terminator `alloc` itself is excluded.
fn local_uses(func: &MirFunction, alloc: (usize, usize)) -> BTreeMap<String, Vec<UseKind>> {
    let mut uses: BTreeMap<String, Vec<UseKind>> = BTreeMap::new();
    for (bi, block) in func.blocks.iter().enumerate() {
        for (si, stmt) in block.statements.iter().enumerate() {
            if (bi, si) == alloc {
                continue; // The allocation itself is not a use.
            }
            let Statement::Assign(dest, rv) = stmt;
            for p in rvalue_places(rv) {
                let kind = match rv {
                    Rvalue::Use(_) => UseKind::ValueRead(dest.local.clone()),
                    _ => UseKind::Other,
                };
                uses.entry(p.local.clone()).or_default().push(kind);
            }
        }
        for p in terminator_places(&block.terminator) {
            // For IndirectCall the callee is a dispatch; other places are Other.
            let kind = match &block.terminator {
                Terminator::IndirectCall { callee, .. } if p.local == callee.local => {
                    UseKind::Dispatch
                }
                _ => UseKind::Other,
            };
            uses.entry(p.local.clone()).or_default().push(kind);
        }
        // Force: the thunk is a bare local name, not a Place.
        if let Terminator::Force { thunk, .. } = &block.terminator {
            uses.entry(thunk.clone()).or_default().push(UseKind::Dispatch);
        }
    }
    uses
}

/// Returns true if the closure allocated at statement index `(bi, si)` of `func` escapes.
///
/// A closure is non-escaping when every use of its destination local is a direct
/// dispatch (`Terminator::IndirectCall` on the closure local, or on a local that
/// is itself only ever directly dispatched — the lowering of `f(x)` copies the
/// callee into a temp first, so we follow these one-hop "dispatch aliases"),
/// the local is never reassigned, and no other statement reads the closure as a value.
/// Non-escaping closures are direct-called instead of becoming tagged variants.
fn closure_alloc_escapes(func: &MirFunction, local: &str, alloc: (usize, usize)) -> bool {
    // Reassignment of the closure local itself escapes.
    for (bi, block) in func.blocks.iter().enumerate() {
        for (si, stmt) in block.statements.iter().enumerate() {
            if (bi, si) == alloc {
                continue;
            }
            let Statement::Assign(dest, _rv) = stmt;
            if dest.local == local {
                return true;
            }
        }
    }

    let uses = local_uses(func, alloc);
    let Some(use_list) = uses.get(local) else {
        return false; // Never used: non-escaping (dead closure).
    };

    for use_kind in use_list {
        match use_kind {
            UseKind::Dispatch => {}
            UseKind::Other => return true,
            UseKind::ValueRead(dest) => {
                // `let d = f`: `f` escapes unless `d` is itself only directly dispatched.
                if !dest_is_dispatch_only(func, dest, alloc) {
                    return true;
                }
            }
        }
    }
    false
}

/// Returns true if every use of the local `dest` is a direct dispatch.
fn dest_is_dispatch_only(func: &MirFunction, dest: &str, alloc: (usize, usize)) -> bool {
    let uses = local_uses(func, alloc);
    uses.get(dest)
        .map(|list| list.iter().all(|uk| matches!(uk, UseKind::Dispatch)))
        .unwrap_or(true)
}

/// Returns the locals that are direct dispatches of a non-escaping closure: the
/// closure local itself plus any one-hop dispatch-alias locals (`let d = f` copies
/// that are only used as indirect-call callees).
fn dispatch_alias_locals(func: &MirFunction, local: &str, alloc: (usize, usize)) -> Vec<String> {
    let uses = local_uses(func, alloc);
    let mut locals = vec![local.to_string()];
    if let Some(use_list) = uses.get(local) {
        for use_kind in use_list {
            if let UseKind::ValueRead(dest) = use_kind {
                if dest_is_dispatch_only(func, dest, alloc) {
                    locals.push(dest.clone());
                }
            }
        }
    }
    locals
}

/// Replaces `Rvalue::ClosureAlloc` for closures that do not escape with direct calls.
///
/// Non-escaping closures are inlined/direct-called rather than tagged: the closure's
/// captured values are copied into fresh temporaries at the allocation site (preserving
/// allocation-time semantics even if the captured variables are later reassigned), and
/// every direct dispatch of the closure becomes a direct `Rvalue::Call` to the closure
/// function followed by a `Branch` to the original continuation.
fn rewrite_non_escaping_closures(program: &mut MirProgram, stats: &mut DefunctionalizeStats) {
    for func in &mut program.functions {
        // 1. Collect closure allocs and filter to the non-escaping ones.
        let mut allocs: BTreeMap<String, (String, Vec<Place>, usize, usize)> = BTreeMap::new();
        for (bi, block) in func.blocks.iter().enumerate() {
            for (si, stmt) in block.statements.iter().enumerate() {
                if let Statement::Assign(dest, Rvalue::ClosureAlloc { fn_name, captured }) = stmt {
                    allocs.entry(dest.local.clone()).or_insert((
                        fn_name.clone(),
                        captured.clone(),
                        bi,
                        si,
                    ));
                }
            }
        }

        let mut non_escaping: HashMap<String, NonEscapingClosure> = HashMap::new();
        // Dispatch-alias local -> owning closure local, for every local that is
        // directly dispatched at an `IndirectCall` for a non-escaping closure.
        let mut alias_to_nc: HashMap<String, String> = HashMap::new();
        for (local, (fn_name, _captured, bi, si)) in &allocs {
            if closure_alloc_escapes(func, local, (*bi, *si)) {
                continue;
            }
            for alias in dispatch_alias_locals(func, local, (*bi, *si)) {
                alias_to_nc.insert(alias, local.clone());
            }
            non_escaping.insert(
                local.clone(),
                NonEscapingClosure {
                    fn_name: fn_name.clone(),
                    captured_copies: Vec::new(),
                },
            );
        }
        if non_escaping.is_empty() {
            continue;
        }

        // 2. Replace each alloc with copies of the captured values (fresh temporaries)
        //    and keep the closure local defined as a function pointer. Process in
        //    descending statement order so splices do not invalidate indices.
        let mut candidates: Vec<(usize, usize, String)> = allocs
            .iter()
            .filter(|(local, _)| non_escaping.contains_key(*local))
            .map(|(local, (_, _, bi, si))| (*bi, *si, local.clone()))
            .collect();
        candidates.sort_by(|a, b| b.cmp(a));
        for (bi, si, local) in candidates {
            let (_ignored, captured, alloc_bi, alloc_si) = allocs[&local].clone();
            debug_assert_eq!((alloc_bi, alloc_si), (bi, si));
            let mut new_stmts = Vec::with_capacity(captured.len() + 1);
            let mut copies = Vec::with_capacity(captured.len());
            for (j, cap_place) in captured.iter().enumerate() {
                let temp = Place {
                    local: format!("_ne_cap_{}_{}", local, j),
                    projections: vec![],
                };
                func.locals.push(MirLocalDecl {
                    name: temp.local.clone(),
                    ty: local_type_of(func, &cap_place.local),
                    mutable: true,
                });
                new_stmts.push(Statement::Assign(
                    temp.clone(),
                    Rvalue::Use(cap_place.clone()),
                ));
                copies.push(temp);
            }
            // The closure local stays defined as a function pointer; all dispatches
            // of it (and of its dispatch aliases) are rewritten to direct calls, so
            // nothing reads it afterwards.
            new_stmts.push(Statement::Assign(
                Place {
                    local: local.clone(),
                    projections: vec![],
                },
                Rvalue::FnPtr(non_escaping[&local].fn_name.clone()),
            ));
            func.blocks[bi].statements.splice(si..si + 1, new_stmts);
            if let Some(nc) = non_escaping.get_mut(&local) {
                nc.captured_copies = copies;
            }
        }

        // 3. Rewrite direct dispatches of non-escaping closures into direct calls.
        for block in &mut func.blocks {
            let mut rewrite: Option<(Place, Rvalue)> = None;
            let mut new_term = None;
            if let Terminator::IndirectCall {
                callee,
                args,
                dest,
                next,
            } = &block.terminator
            {
                if callee.projections.is_empty() {
                    if let Some(owner) = alias_to_nc.get(&callee.local) {
                        let nc = &non_escaping[owner];
                        let mut call_args = nc.captured_copies.clone();
                        call_args.extend(args.iter().cloned());
                        rewrite = Some((
                            dest.clone(),
                            Rvalue::Call(nc.fn_name.clone(), call_args),
                        ));
                        new_term = Some(Terminator::Branch { target: next.clone() });
                        stats.closures_inlined += 1;
                    }
                }
            }
            if let Some((dest, rv)) = rewrite {
                block.statements.push(Statement::Assign(dest, rv));
            }
            if let Some(term) = new_term {
                block.terminator = term;
            }
        }
    }
}

/// Executes whole-program Reynolds defunctionalization on the given MIR program.
pub fn defunctionalize_program(program: &mut MirProgram) -> DefunctionalizeStats {
    let mut stats = DefunctionalizeStats::default();

    // 0. Escape analysis + direct-call rewrite for non-escaping closures.
    rewrite_non_escaping_closures(program, &mut stats);

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
                (
                    enum_name.clone(),
                    variant_name,
                    tag,
                    cand.captured_types.clone(),
                ),
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
            if let Terminator::IndirectCall {
                callee,
                args,
                dest,
                next,
            } = &block.terminator
            {
                // Find matching signature by arity
                let matching_group = sig_groups
                    .iter()
                    .find(|g| g.signature.param_tys.len() == args.len());

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
                        let mut call_args =
                            Vec::with_capacity(variant.captured_types.len() + args.len());

                        // Unpack captured environment payload fields.
                        // Scalar (numeric) fields are *unboxed*: they are read directly
                        // from the enum payload as call arguments, avoiding temporaries.
                        for (j, cap_ty) in variant.captured_types.iter().enumerate() {
                            let mut proj_place = callee.clone();
                            proj_place.projections.push(Projection::Payload(j));
                            if cap_ty.is_numeric() {
                                call_args.push(proj_place);
                                stats.scalar_payloads_unboxed += 1;
                            } else {
                                let cap_temp = Place {
                                    local: format!(
                                        "_dt_cap_{}_{}_{}",
                                        variant.fn_name,
                                        j,
                                        arm_bb_id.0
                                    ),
                                    projections: vec![],
                                };
                                func.locals.push(MirLocalDecl {
                                    name: cap_temp.local.clone(),
                                    ty: cap_ty.clone(),
                                    mutable: true,
                                });
                                arm_stmts.push(Statement::Assign(
                                    cap_temp.clone(),
                                    Rvalue::Use(proj_place),
                                ));
                                call_args.push(cap_temp);
                            }
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
                            terminator: Terminator::Branch {
                                target: next.clone(),
                            },
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
