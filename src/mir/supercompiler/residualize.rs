use std::collections::HashMap;

use super::drive::{ProcessEdge, ProcessNodeId, ProcessTree};
use super::term::{SymTerm, SymTermId, TermInterner};
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

    // 2. Generate statements and terminators for each block
    for node in &tree.nodes {
        let block_id = &node_to_block[&node.id];
        let mut stmts = Vec::new();

        // If this is a return node
        if let Some(ret_term) = node.return_term {
            let ret_place = emit_term_eval(
                ret_term,
                &tree.interner,
                &mut stmts,
                &mut new_locals,
                &mut next_temp_id,
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
        if node.edges.is_empty() {
            residual_blocks[b_idx].statements = stmts;
            residual_blocks[b_idx].terminator = Terminator::Return { value: None };
        } else if node.edges.len() == 1 {
            match &node.edges[0] {
                ProcessEdge::Step(next_node) | ProcessEdge::Knot(next_node) => {
                    let target_block = node_to_block[next_node].clone();
                    residual_blocks[b_idx].statements = stmts;
                    residual_blocks[b_idx].terminator = Terminator::Branch {
                        target: target_block,
                    };
                }
                _ => {}
            }
        } else if node.edges.len() == 2 {
            // BranchTrue and BranchFalse
            let mut true_target = None;
            let mut false_target = None;
            let mut cond_term = None;

            for edge in &node.edges {
                match edge {
                    ProcessEdge::BranchTrue(tgt, c) => {
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
                );
                residual_blocks[b_idx].statements = stmts;
                residual_blocks[b_idx].terminator = Terminator::BranchIf {
                    condition: cond_place,
                    then_target: then_t,
                    else_target: else_t,
                };
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
            let l_place = emit_term_eval(*l, interner, stmts, locals, next_temp_id);
            let r_place = emit_term_eval(*r, interner, stmts, locals, next_temp_id);

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
            let in_place = emit_term_eval(*inner, interner, stmts, locals, next_temp_id);

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
        SymTerm::Select(c, th, el, ty) => {
            let _c_place = emit_term_eval(*c, interner, stmts, locals, next_temp_id);
            let th_place = emit_term_eval(*th, interner, stmts, locals, next_temp_id);
            let _el_place = emit_term_eval(*el, interner, stmts, locals, next_temp_id);

            // Simplified: return then_place if condition evaluation folded
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
            for (b, t) in incoming {
                let p = emit_term_eval(*t, interner, stmts, locals, next_temp_id);
                ops.push((b.clone(), p));
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
            stmts.push(Statement::Assign(place.clone(), Rvalue::Phi(ops)));
            place
        }
        SymTerm::Constructor(name, tag, fields, ty) => {
            let mut field_places = Vec::new();
            for (idx, &f) in fields.iter().enumerate() {
                let f_p = emit_term_eval(f, interner, stmts, locals, next_temp_id);
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
                arg_places.push(emit_term_eval(a, interner, stmts, locals, next_temp_id));
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
    }
}
