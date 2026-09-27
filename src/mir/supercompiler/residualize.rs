use std::collections::HashMap;

use super::drive::{ProcessEdge, ProcessNodeId, ProcessTree};
use super::term::{SymTerm, SymTermId, TermInterner};
use crate::ast::BinaryOp;
use crate::mir::lower::{MirBasicBlock, MirFunction, MirLocalDecl, Rvalue, Statement};
use crate::mir::{BasicBlockId, Place, Terminator};
use crate::typecheck::types::Type;

/// Residualizes a ProcessTree back into an optimized MirFunction.
pub fn residualize_process_tree(tree: &ProcessTree, original_func: &MirFunction) -> MirFunction {
    if tree.nodes.is_empty() {
        return original_func.clone();
    }

    let mut residual_blocks: Vec<MirBasicBlock> = Vec::new();
    let mut node_to_block: HashMap<ProcessNodeId, BasicBlockId> = HashMap::new();
    let mut new_locals = original_func.locals.clone();
    let mut next_temp_id = original_func.locals.len();

    // 1. Assign a BasicBlockId to each reachable process tree node
    for (i, node) in tree.nodes.iter().enumerate() {
        let b_id = BasicBlockId(i);
        node_to_block.insert(node.id, b_id.clone());
        residual_blocks.push(MirBasicBlock {
            id: b_id,
            arguments: Vec::new(),
            statements: Vec::new(),
            terminator: Terminator::Unreachable,
        });
    }

    // Map process tree transitions to residual BasicBlockIds
    // predecessor_map[target_node_id] = HashMap<original_bb, residual_pred_bb>
    let mut node_preds: HashMap<ProcessNodeId, HashMap<BasicBlockId, BasicBlockId>> = HashMap::new();
    for node in &tree.nodes {
        let from_res_bb = node_to_block[&node.id].clone();
        let from_orig_bb = node.state.block.clone();
        for edge in &node.edges {
            let target_node_id = match edge {
                ProcessEdge::Step(tgt) => *tgt,
                ProcessEdge::BranchTrue(tgt, _) => *tgt,
                ProcessEdge::BranchFalse(tgt, _) => *tgt,
                ProcessEdge::Knot(tgt) => *tgt,
            };
            node_preds
                .entry(target_node_id)
                .or_default()
                .insert(from_orig_bb.clone(), from_res_bb.clone());
        }
    }

    // 2. Generate statements and terminators for each block
    for node in &tree.nodes {
        let block_id = &node_to_block[&node.id];
        let mut stmts = Vec::new();
        let phi_remap = node_preds.get(&node.id).cloned().unwrap_or_default();

        // If this is a return node
        if let Some(ret_term) = node.return_term {
            let ret_place = emit_term_eval(
                ret_term,
                &tree.interner,
                &mut stmts,
                &mut new_locals,
                &mut next_temp_id,
                &phi_remap,
            );
            let b_idx = block_id.0;
            residual_blocks[b_idx].statements = stmts;
            residual_blocks[b_idx].terminator = Terminator::Return {
                value: Some(ret_place),
            };
            continue;
        }

        // Handle outgoing edges
        let b_idx = block_id.0;
        let is_orig_void_return = original_func.blocks.iter().any(|b| {
            b.id == node.state.block && matches!(b.terminator, Terminator::Return { value: None })
        });
        if node.overflow || (node.edges.is_empty() && (!is_orig_void_return || original_func.return_ty != Type::Void)) {
            // Task 2: Budget-overflow or zero-edge non-Return leaf.
            // Safely terminate with Unreachable rather than an empty Return.
            #[cfg(debug_assertions)]
            eprintln!(
                "[supercompiler] Warning: emitting Unreachable for zero-edge non-return leaf (overflow={}) at node {:?}",
                node.overflow, node.id
            );
            residual_blocks[b_idx].statements = stmts;
            residual_blocks[b_idx].terminator = Terminator::Unreachable;
        } else if node.edges.is_empty() {
            residual_blocks[b_idx].statements = stmts;
            residual_blocks[b_idx].terminator = Terminator::Return { value: None };
        } else if node.edges.len() == 1 {
            match &node.edges[0] {
                ProcessEdge::Step(next_node) => {
                    let target_block = node_to_block[next_node].clone();
                    residual_blocks[b_idx].statements = stmts;
                    residual_blocks[b_idx].terminator = Terminator::Branch {
                        target: target_block,
                    };
                }
                ProcessEdge::Knot(next_node) => {
                    let target_block = node_to_block[next_node].clone();
                    let anc_state = &tree.nodes[next_node.0].state;
                    let curr_state = &node.state;

                    // Collect all places that mutated between ancestor and descendant
                    let mut transfers: Vec<(Place, SymTermId)> = Vec::new();
                    for (place, &anc_term) in &anc_state.env {
                        if place.projections.is_empty() {
                            if let Some(&curr_term) = curr_state.env.get(place) {
                                if curr_term != anc_term {
                                    transfers.push((place.clone(), curr_term));
                                }
                            }
                        }
                    }
                    transfers.sort_by(|a, b| a.0.local.cmp(&b.0.local));

                    // Parallel copy: evaluate into fresh scratch temporaries first
                    let mut evaled_transfers: Vec<(Place, Place)> = Vec::new();
                    for (dest_place, curr_term) in transfers {
                        let val_place = emit_term_eval(
                            curr_term,
                            &tree.interner,
                            &mut stmts,
                            &mut new_locals,
                            &mut next_temp_id,
                            &phi_remap,
                        );
                        evaled_transfers.push((dest_place, val_place));
                    }
                    // Then assign all dest places from temporaries
                    for (dest_place, val_place) in evaled_transfers {
                        stmts.push(Statement::Assign(dest_place, Rvalue::Use(val_place)));
                    }

                    residual_blocks[b_idx].statements = stmts;
                    residual_blocks[b_idx].terminator = Terminator::Branch {
                        target: target_block,
                    };
                }
                _ => {}
            }
        } else {
            // Check for BranchTrue/BranchFalse pair
            let mut true_target = None;
            let mut false_target = None;
            let mut cond_term = None;

            for edge in &node.edges {
                match edge {
                    ProcessEdge::BranchTrue(tgt, c) if true_target.is_none() => {
                        true_target = Some(node_to_block[tgt].clone());
                        cond_term = Some(*c);
                    }
                    ProcessEdge::BranchFalse(tgt, _) => {
                        false_target = Some(node_to_block[tgt].clone());
                    }
                    _ => {}
                }
            }

            if let (Some(then_t), Some(else_t), Some(c)) =
                (true_target, false_target, cond_term)
            {
                let cond_place = emit_term_eval(
                    c,
                    &tree.interner,
                    &mut stmts,
                    &mut new_locals,
                    &mut next_temp_id,
                    &phi_remap,
                );
                residual_blocks[b_idx].statements = stmts;
                residual_blocks[b_idx].terminator = Terminator::BranchIf {
                    condition: cond_place,
                    then_target: then_t,
                    else_target: else_t,
                };
            } else {
                // Multi-way branch (from Terminator::Switch or multi-branch pattern matches)
                let branch_trues: Vec<(&ProcessNodeId, &SymTermId)> = node
                    .edges
                    .iter()
                    .filter_map(|e| match e {
                        ProcessEdge::BranchTrue(tgt, c) => Some((tgt, c)),
                        _ => None,
                    })
                    .collect();

                let mut switch_val_term = None;
                let mut switch_targets = Vec::new();
                let mut all_switch_cases = true;

                for (tgt, &c) in &branch_trues {
                    if let SymTerm::Binary(BinaryOp::Eq, l, r, _) = tree.interner.get(c) {
                        if let SymTerm::ConstInt(case_val, _) = tree.interner.get(*r) {
                            if switch_val_term.is_none() {
                                switch_val_term = Some(*l);
                            } else if switch_val_term != Some(*l) {
                                all_switch_cases = false;
                                break;
                            }
                            switch_targets.push((*case_val, node_to_block[tgt].clone()));
                        } else {
                            all_switch_cases = false;
                            break;
                        }
                    } else {
                        all_switch_cases = false;
                        break;
                    }
                }

                if all_switch_cases && !switch_targets.is_empty() {
                    if let Some(val_term) = switch_val_term {
                        let val_place = emit_term_eval(
                            val_term,
                            &tree.interner,
                            &mut stmts,
                            &mut new_locals,
                            &mut next_temp_id,
                            &phi_remap,
                        );
                        let default_tgt = switch_targets.last().unwrap().1.clone();
                        residual_blocks[b_idx].statements = stmts;
                        residual_blocks[b_idx].terminator = Terminator::Switch {
                            value: val_place,
                            targets: switch_targets,
                            default: default_tgt,
                        };
                    }
                } else if branch_trues.len() >= 2 {
                    let first_tgt = node_to_block[branch_trues[0].0].clone();
                    let second_tgt = node_to_block[branch_trues[1].0].clone();
                    let cond_place = emit_term_eval(
                        *branch_trues[0].1,
                        &tree.interner,
                        &mut stmts,
                        &mut new_locals,
                        &mut next_temp_id,
                        &phi_remap,
                    );
                    residual_blocks[b_idx].statements = stmts;
                    residual_blocks[b_idx].terminator = Terminator::BranchIf {
                        condition: cond_place,
                        then_target: first_tgt,
                        else_target: second_tgt,
                    };
                }
            }
        }
    }

    MirFunction {
        name: original_func.name.clone(),
        params: original_func.params.clone(),
        return_ty: original_func.return_ty.clone(),
        locals: new_locals,
        blocks: residual_blocks,
    }
}

fn emit_term_eval(
    term_id: SymTermId,
    interner: &TermInterner,
    stmts: &mut Vec<Statement>,
    locals: &mut Vec<MirLocalDecl>,
    next_temp_id: &mut usize,
    phi_remap: &HashMap<BasicBlockId, BasicBlockId>,
) -> Place {
    if let Some(lit) = interner.get(term_id).to_literal() {
        let temp_name = format!("_sc_{}", *next_temp_id);
        *next_temp_id += 1;
        let ty = match &lit {
            crate::typecheck::typed_ast::TypedLiteral::Int(_, t) => t.clone(),
            crate::typecheck::typed_ast::TypedLiteral::Float(_, t) => t.clone(),
            crate::typecheck::typed_ast::TypedLiteral::Bool(_) => Type::Bool,
            crate::typecheck::typed_ast::TypedLiteral::Str(_) => Type::Str,
        };
        locals.push(MirLocalDecl {
            name: temp_name.clone(),
            ty,
            mutable: false,
        });
        let place = Place {
            local: temp_name,
            projections: vec![],
        };
        stmts.push(Statement::Assign(
            place.clone(),
            Rvalue::Constant(lit),
        ));
        return place;
    }
    match interner.get(term_id) {
        SymTerm::ConstInt(..)
        | SymTerm::ConstFloat(..)
        | SymTerm::ConstBool(..)
        | SymTerm::ConstStr(..) => unreachable!("constants handled by to_literal above"),
        SymTerm::Var(p, _) => p.clone(),
        SymTerm::Binary(op, l, r, ty) => {
            let l_place = emit_term_eval(*l, interner, stmts, locals, next_temp_id, phi_remap);
            let r_place = emit_term_eval(*r, interner, stmts, locals, next_temp_id, phi_remap);

            let temp_name = format!("_sc_{}", *next_temp_id);
            *next_temp_id += 1;
            locals.push(MirLocalDecl {
                name: temp_name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            let place = Place {
                local: temp_name,
                projections: vec![],
            };
            stmts.push(Statement::Assign(
                place.clone(),
                Rvalue::BinaryOp(*op, l_place, r_place),
            ));
            place
        }
        SymTerm::Unary(op, inner, ty) => {
            let in_place = emit_term_eval(*inner, interner, stmts, locals, next_temp_id, phi_remap);

            let temp_name = format!("_sc_{}", *next_temp_id);
            *next_temp_id += 1;
            locals.push(MirLocalDecl {
                name: temp_name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            let place = Place {
                local: temp_name,
                projections: vec![],
            };
            stmts.push(Statement::Assign(
                place.clone(),
                Rvalue::UnaryOp(*op, in_place),
            ));
            place
        }
        SymTerm::Select(c, th, _el, ty) => {
            let _c_place = emit_term_eval(*c, interner, stmts, locals, next_temp_id, phi_remap);
            let th_place = emit_term_eval(*th, interner, stmts, locals, next_temp_id, phi_remap);

            let temp_name = format!("_sc_{}", *next_temp_id);
            *next_temp_id += 1;
            locals.push(MirLocalDecl {
                name: temp_name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            let place = Place {
                local: temp_name,
                projections: vec![],
            };
            stmts.push(Statement::Assign(
                place.clone(),
                Rvalue::Use(th_place),
            ));
            place
        }
        SymTerm::Phi(incoming, ty) => {
            let mut ops = Vec::new();
            for (orig_bb, t) in incoming {
                if let Some(target_bb) = phi_remap.get(orig_bb) {
                    let p = emit_term_eval(*t, interner, stmts, locals, next_temp_id, phi_remap);
                    ops.push((target_bb.clone(), p));
                }
            }
            if ops.is_empty() {
                for (orig_bb, t) in incoming {
                    let p = emit_term_eval(*t, interner, stmts, locals, next_temp_id, phi_remap);
                    ops.push((orig_bb.clone(), p));
                }
            }
            let temp_name = format!("_sc_{}", *next_temp_id);
            *next_temp_id += 1;
            locals.push(MirLocalDecl {
                name: temp_name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            let place = Place {
                local: temp_name,
                projections: vec![],
            };
            if ops.len() == 1 {
                stmts.push(Statement::Assign(place.clone(), Rvalue::Use(ops[0].1.clone())));
            } else {
                stmts.push(Statement::Assign(place.clone(), Rvalue::Phi(ops)));
            }
            place
        }
        SymTerm::Constructor(name, tag, fields, ty) => {
            let mut field_places = Vec::new();
            for (idx, &f) in fields.iter().enumerate() {
                let f_p = emit_term_eval(f, interner, stmts, locals, next_temp_id, phi_remap);
                field_places.push((format!("f{}", idx), f_p));
            }
            let temp_name = format!("_sc_{}", *next_temp_id);
            *next_temp_id += 1;
            locals.push(MirLocalDecl {
                name: temp_name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            let place = Place {
                local: temp_name,
                projections: vec![],
            };
            if let Type::Enum(ref enum_name) = ty {
                stmts.push(Statement::Assign(
                    place.clone(),
                    Rvalue::EnumVariant {
                        enum_name: enum_name.clone(),
                        variant_name: name.clone(),
                        tag: *tag,
                        fields: field_places.into_iter().map(|(_, p)| p).collect(),
                    },
                ));
            } else {
                stmts.push(Statement::Assign(
                    place.clone(),
                    Rvalue::Struct(name.clone(), field_places),
                ));
            }
            place
        }
        SymTerm::Call(callee, args, ty) => {
            let mut arg_places = Vec::new();
            for &a in args {
                arg_places.push(emit_term_eval(a, interner, stmts, locals, next_temp_id, phi_remap));
            }
            let temp_name = format!("_sc_{}", *next_temp_id);
            *next_temp_id += 1;
            locals.push(MirLocalDecl {
                name: temp_name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            let place = Place {
                local: temp_name,
                projections: vec![],
            };
            stmts.push(Statement::Assign(
                place.clone(),
                Rvalue::Call(callee.clone(), arg_places),
            ));
            place
        }
        SymTerm::Ref(inner, ty) => {
            let in_place = emit_term_eval(*inner, interner, stmts, locals, next_temp_id, phi_remap);
            let temp_name = format!("_sc_{}", *next_temp_id);
            *next_temp_id += 1;
            locals.push(MirLocalDecl {
                name: temp_name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            let place = Place {
                local: temp_name,
                projections: vec![],
            };
            stmts.push(Statement::Assign(
                place.clone(),
                Rvalue::Alloc(in_place),
            ));
            place
        }
        SymTerm::Deref(ptr, ty) => {
            let p_place = emit_term_eval(*ptr, interner, stmts, locals, next_temp_id, phi_remap);
            let temp_name = format!("_sc_{}", *next_temp_id);
            *next_temp_id += 1;
            locals.push(MirLocalDecl {
                name: temp_name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            let place = Place {
                local: temp_name,
                projections: vec![],
            };
            stmts.push(Statement::Assign(
                place.clone(),
                Rvalue::Load(p_place),
            ));
            place
        }
        SymTerm::Discriminant(inner, ty) => {
            let in_place = emit_term_eval(*inner, interner, stmts, locals, next_temp_id, phi_remap);
            let temp_name = format!("_sc_{}", *next_temp_id);
            *next_temp_id += 1;
            locals.push(MirLocalDecl {
                name: temp_name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            let place = Place {
                local: temp_name,
                projections: vec![],
            };
            stmts.push(Statement::Assign(
                place.clone(),
                Rvalue::Discriminant(in_place),
            ));
            place
        }
    }
}
