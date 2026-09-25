use std::collections::{HashMap, HashSet};

use crate::ast::BinaryOp;
use crate::mir::dominance::{compute_dominance, detect_loops};
use crate::mir::lower::{MirFunction, MirLocalDecl, Rvalue, Statement};
use crate::mir::supercompiler::term::{SymTermId, TermInterner};
use crate::mir::{compute_cfg, BasicBlockId, Place, Projection, Terminator};
use crate::typecheck::types::Type;

/// A candidate pair of loops for producer/consumer deforestation and stream fusion.
#[derive(Debug, Clone)]
pub struct FusionCandidate {
    pub producer_header: BasicBlockId,
    pub consumer_header: BasicBlockId,
    pub intermediate_buf: Place,
    pub trip_count: SymTermId,
}

#[derive(Debug, Clone)]
struct LoopDetails {
    header: BasicBlockId,
    blocks: HashSet<BasicBlockId>,
    _iv: Place,
    bound: Place,
    _body_target: BasicBlockId,
    _exit_target: BasicBlockId,
    written_arrays: Vec<(String, Place)>, // (buf_local, full_place_with_index)
    read_arrays: Vec<(String, Place)>,    // (buf_local, full_place_with_index)
}

/// Builds an alias map mapping temporary SSA variables (`_t...`) to their underlying source variable.
pub fn build_alias_map(func: &MirFunction) -> HashMap<String, String> {
    let mut aliases = HashMap::new();
    for b in &func.blocks {
        for stmt in &b.statements {
            if let Statement::Assign(dest, Rvalue::Use(src)) = stmt {
                if dest.projections.is_empty() && src.projections.is_empty() {
                    aliases.insert(dest.local.clone(), src.local.clone());
                }
            }
        }
    }
    // Resolve chains: a -> b -> c
    let keys: Vec<String> = aliases.keys().cloned().collect();
    for k in keys {
        let mut curr = aliases.get(&k).cloned().unwrap();
        let mut depth = 0;
        while let Some(next) = aliases.get(&curr) {
            if *next == curr || depth > 20 {
                break;
            }
            curr = next.clone();
            depth += 1;
        }
        aliases.insert(k, curr);
    }
    aliases
}

/// Resolves a local name through the alias map, returning the underlying source variable.
pub fn resolve_alias<'a>(name: &'a str, aliases: &'a HashMap<String, String>) -> &'a str {
    aliases.get(name).map(|s| s.as_str()).unwrap_or(name)
}

/// Identifies candidate pairs of producer and consumer loops iterating over the same range 0..N
/// where the producer writes to an intermediate buffer and the consumer reads from it with no other uses.
pub fn find_fusion_candidates(func: &MirFunction) -> Vec<FusionCandidate> {
    if func.blocks.len() < 2 {
        return Vec::new();
    }

    let aliases = build_alias_map(func);
    let (preds, succs) = compute_cfg(&func.blocks);
    let all_block_ids: Vec<BasicBlockId> = func.blocks.iter().map(|b| b.id.clone()).collect();
    let entry = func.blocks[0].id.clone();
    let dom = compute_dominance(entry, &preds, &succs, &all_block_ids);
    let loop_info = detect_loops(func.blocks[0].id.clone(), &preds, &succs, &all_block_ids, &dom);

    if loop_info.headers.len() < 2 {
        return Vec::new();
    }

    let block_map: HashMap<BasicBlockId, &crate::mir::lower::MirBasicBlock> =
        func.blocks.iter().map(|b| (b.id.clone(), b)).collect();

    let mut loops = Vec::new();
    let mut interner = TermInterner::new();

    for h in &loop_info.headers {
        if let Some(blocks) = loop_info.natural_loops.get(h) {
            if let Some(hb) = block_map.get(h) {
                if let Terminator::BranchIf {
                    condition,
                    then_target,
                    else_target,
                } = &hb.terminator
                {
                    let (body_target, exit_target) = if blocks.contains(then_target) {
                        (then_target.clone(), else_target.clone())
                    } else if blocks.contains(else_target) {
                        (else_target.clone(), then_target.clone())
                    } else {
                        continue;
                    };

                    // Find induction variable and bound in header block
                    let mut iv_opt = None;
                    let mut bound_opt = None;

                    for stmt in &hb.statements {
                        if let Statement::Assign(dest, Rvalue::BinaryOp(op, l, r)) = stmt {
                            if dest == condition && (*op == BinaryOp::Lt || *op == BinaryOp::Le) {
                                let real_l = resolve_alias(&l.local, &aliases);
                                iv_opt = Some(Place {
                                    local: real_l.to_string(),
                                    projections: vec![],
                                });
                                bound_opt = Some(r.clone());
                                break;
                            }
                        }
                    }

                    let (iv, bound) = match (iv_opt, bound_opt) {
                        (Some(i), Some(b)) => (i, b),
                        _ => continue,
                    };

                    // Collect array writes and reads across all loop blocks
                    let mut written_arrays = Vec::new();
                    let mut read_arrays = Vec::new();

                    for b_id in blocks {
                        if let Some(b) = block_map.get(b_id) {
                            for stmt in &b.statements {
                                let Statement::Assign(dest, rval) = stmt;
                                if !dest.projections.is_empty()
                                    && dest.projections.iter().any(|p| matches!(p, Projection::Index(_)))
                                {
                                    let real_dest = resolve_alias(&dest.local, &aliases);
                                    written_arrays.push((real_dest.to_string(), dest.clone()));
                                }
                                collect_read_arrays_from_rvalue(rval, &mut read_arrays, &aliases);
                            }
                        }
                    }

                    loops.push(LoopDetails {
                        header: h.clone(),
                        blocks: blocks.clone(),
                        _iv: iv,
                        bound,
                        _body_target: body_target,
                        _exit_target: exit_target,
                        written_arrays,
                        read_arrays,
                    });
                }
            }
        }
    }

    let mut candidates = Vec::new();

    // Check all pairs of loops
    for i in 0..loops.len() {
        for j in 0..loops.len() {
            if i == j {
                continue;
            }
            let loop_a = &loops[i];
            let loop_b = &loops[j];

            // Verify bounds match (same variable or constant value)
            if !bounds_match(&loop_a.bound, &loop_b.bound, &block_map, &aliases) {
                continue;
            }

            // Find an intermediate buffer written in loop A and read in loop B
            for (buf_name_a, place_a) in &loop_a.written_arrays {
                // Must be read in loop B
                let is_read_in_b = loop_b.read_arrays.iter().any(|(name, _)| name == buf_name_a);
                if !is_read_in_b {
                    continue;
                }

                // Loop A must NOT read from buf_name_a
                if loop_a.read_arrays.iter().any(|(name, _)| name == buf_name_a) {
                    continue;
                }

                // Loop B must NOT write to buf_name_a
                if loop_b.written_arrays.iter().any(|(name, _)| name == buf_name_a) {
                    continue;
                }

                // Check that buf_name_a has NO OTHER USES outside loop A and loop B
                if has_external_uses(buf_name_a, &loop_a.blocks, &loop_b.blocks, func, &aliases) {
                    continue;
                }

                let trip_count_term = intern_bound(&loop_a.bound, &block_map, &mut interner, &aliases);

                candidates.push(FusionCandidate {
                    producer_header: loop_a.header.clone(),
                    consumer_header: loop_b.header.clone(),
                    intermediate_buf: Place {
                        local: buf_name_a.clone(),
                        projections: place_a.projections.clone(),
                    },
                    trip_count: trip_count_term,
                });
                break;
            }
        }
    }

    candidates
}

fn bounds_match(
    b1: &Place,
    b2: &Place,
    block_map: &HashMap<BasicBlockId, &crate::mir::lower::MirBasicBlock>,
    aliases: &HashMap<String, String>,
) -> bool {
    let r1 = resolve_alias(&b1.local, aliases);
    let r2 = resolve_alias(&b2.local, aliases);
    if r1 == r2 {
        return true;
    }
    // Check if both resolve to the same constant integer
    let c1 = resolve_const_int(b1, block_map, aliases);
    let c2 = resolve_const_int(b2, block_map, aliases);
    match (c1, c2) {
        (Some(v1), Some(v2)) => v1 == v2,
        _ => false,
    }
}

fn resolve_const_int(
    place: &Place,
    block_map: &HashMap<BasicBlockId, &crate::mir::lower::MirBasicBlock>,
    aliases: &HashMap<String, String>,
) -> Option<i64> {
    let target = resolve_alias(&place.local, aliases);
    for b in block_map.values() {
        for stmt in &b.statements {
            if let Statement::Assign(dest, Rvalue::Constant(crate::typecheck::typed_ast::TypedLiteral::Int(v, _))) = stmt {
                let dest_name = resolve_alias(&dest.local, aliases);
                if dest.local == place.local || dest.local == target || dest_name == target {
                    return Some(*v);
                }
            }
        }
    }
    None
}

fn intern_bound(
    bound: &Place,
    block_map: &HashMap<BasicBlockId, &crate::mir::lower::MirBasicBlock>,
    interner: &mut TermInterner,
    aliases: &HashMap<String, String>,
) -> SymTermId {
    if let Some(c) = resolve_const_int(bound, block_map, aliases) {
        interner.intern_int(c)
    } else {
        interner.intern_var(bound.clone(), Type::I64)
    }
}

fn collect_read_arrays_from_rvalue(
    rval: &Rvalue,
    out: &mut Vec<(String, Place)>,
    aliases: &HashMap<String, String>,
) {
    let check_place = |p: &Place, out: &mut Vec<(String, Place)>| {
        if p.projections.iter().any(|proj| matches!(proj, Projection::Index(_))) {
            let real_name = resolve_alias(&p.local, aliases);
            out.push((real_name.to_string(), p.clone()));
        }
    };

    match rval {
        Rvalue::Use(p) => check_place(p, out),
        Rvalue::BinaryOp(_, l, r) => {
            check_place(l, out);
            check_place(r, out);
        }
        Rvalue::UnaryOp(_, p) => check_place(p, out),
        Rvalue::Call(_, args) => {
            for arg in args {
                check_place(arg, out);
            }
        }
        _ => {}
    }
}

fn has_external_uses(
    buf_name: &str,
    loop_a_blocks: &HashSet<BasicBlockId>,
    loop_b_blocks: &HashSet<BasicBlockId>,
    func: &MirFunction,
    aliases: &HashMap<String, String>,
) -> bool {
    for b in &func.blocks {
        if loop_a_blocks.contains(&b.id) || loop_b_blocks.contains(&b.id) {
            continue;
        }
        for stmt in &b.statements {
            let Statement::Assign(dest, rval) = stmt;
            let real_dest = resolve_alias(&dest.local, aliases);
            // Initial array constructor/literal allocation or alias assignment is expected outside loops
            if real_dest == buf_name {
                match rval {
                    Rvalue::Array(_) => continue,
                    Rvalue::Constant(_) => continue,
                    Rvalue::Use(src)
                        if resolve_alias(&src.local, aliases) == buf_name && src.projections.is_empty() =>
                    {
                        continue
                    }
                    _ => {}
                }
            }
            // Any read of buf_name outside both loops means it escapes
            let mut reads = Vec::new();
            collect_read_arrays_from_rvalue(rval, &mut reads, aliases);
            if reads.iter().any(|(name, _)| name == buf_name) {
                return true;
            }
            if real_dest == buf_name && !dest.projections.is_empty() {
                return true;
            }
        }
        // Check terminator uses
        match &b.terminator {
            Terminator::Return { value: Some(p) } if resolve_alias(&p.local, aliases) == buf_name => {
                return true
            }
            Terminator::BranchIf { condition, .. }
                if resolve_alias(&condition.local, aliases) == buf_name =>
            {
                return true
            }
            _ => {}
        }
    }
    false
}

/// Fuses candidate producer and consumer loops into a single loop body,
/// directly threading values via an SSA temporary and eliminating intermediate array allocations.
pub fn fuse_loops(func: &mut MirFunction, candidate: &FusionCandidate) {
    let buf_name = candidate.intermediate_buf.local.clone();
    let aliases = build_alias_map(func);

    // 1. Create a scalar SSA temporary for threading the value
    let elem_ty = func
        .locals
        .iter()
        .find(|l| l.name == buf_name || resolve_alias(&l.name, &aliases) == buf_name)
        .and_then(|l| match &l.ty {
            Type::Array(elem, _) => Some((**elem).clone()),
            _ => None,
        })
        .unwrap_or(Type::I64);

    let val_temp_name = format!("_fuse_val_{}_{}", buf_name, func.locals.len());
    func.locals.push(MirLocalDecl {
        name: val_temp_name.clone(),
        ty: elem_ty,
        mutable: true,
    });
    let val_temp = Place {
        local: val_temp_name,
        projections: vec![],
    };

    // 2. Identify loop A (producer) and loop B (consumer) targets and variables
    let (preds, succs) = compute_cfg(&func.blocks);
    let all_block_ids: Vec<BasicBlockId> = func.blocks.iter().map(|b| b.id.clone()).collect();
    let entry = func.blocks[0].id.clone();
    let dom = compute_dominance(entry, &preds, &succs, &all_block_ids);
    let loop_info = detect_loops(func.blocks[0].id.clone(), &preds, &succs, &all_block_ids, &dom);

    let blocks_a = match loop_info.natural_loops.get(&candidate.producer_header) {
        Some(b) => b.clone(),
        None => return,
    };
    let blocks_b = match loop_info.natural_loops.get(&candidate.consumer_header) {
        Some(b) => b.clone(),
        None => return,
    };

    let mut body_a_id = None;
    let mut exit_a_id = None;
    let mut iv_a = None;

    if let Some(hb_a) = func.blocks.iter().find(|b| b.id == candidate.producer_header) {
        if let Terminator::BranchIf { condition, then_target, else_target } = &hb_a.terminator {
            body_a_id = if blocks_a.contains(then_target) { Some(then_target.clone()) } else { Some(else_target.clone()) };
            exit_a_id = if blocks_a.contains(then_target) { Some(else_target.clone()) } else { Some(then_target.clone()) };
            for stmt in &hb_a.statements {
                if let Statement::Assign(dest, Rvalue::BinaryOp(_, l, _)) = stmt {
                    if dest == condition {
                        let real_l = resolve_alias(&l.local, &aliases);
                        iv_a = Some(Place { local: real_l.to_string(), projections: vec![] });
                        break;
                    }
                }
            }
        }
    }

    let mut body_b_id = None;
    let mut exit_b_id = None;
    let mut iv_b = None;

    if let Some(hb_b) = func.blocks.iter().find(|b| b.id == candidate.consumer_header) {
        if let Terminator::BranchIf { condition, then_target, else_target } = &hb_b.terminator {
            body_b_id = if blocks_b.contains(then_target) { Some(then_target.clone()) } else { Some(else_target.clone()) };
            exit_b_id = if blocks_b.contains(then_target) { Some(else_target.clone()) } else { Some(then_target.clone()) };
            for stmt in &hb_b.statements {
                if let Statement::Assign(dest, Rvalue::BinaryOp(_, l, _)) = stmt {
                    if dest == condition {
                        let real_l = resolve_alias(&l.local, &aliases);
                        iv_b = Some(Place { local: real_l.to_string(), projections: vec![] });
                        break;
                    }
                }
            }
        }
    }

    let (body_a_id, exit_a_id, iv_a) = match (body_a_id, exit_a_id, iv_a) {
        (Some(b), Some(e), Some(i)) => (b, e, i),
        _ => return,
    };
    let (body_b_id, exit_b_id, iv_b) = match (body_b_id, exit_b_id, iv_b) {
        (Some(b), Some(e), Some(i)) => (b, e, i),
        _ => return,
    };

    // 3. Rewrite producer write in body_a: buf[i] = val  =>  val_temp = val
    let mut producer_stmt_idx = None;
    if let Some(body_a_block) = func.blocks.iter_mut().find(|b| b.id == body_a_id) {
        for (idx, stmt) in body_a_block.statements.iter_mut().enumerate() {
            let Statement::Assign(dest, _) = stmt;
            let real_dest = resolve_alias(&dest.local, &aliases);
            if real_dest == buf_name && !dest.projections.is_empty() {
                *dest = val_temp.clone();
                producer_stmt_idx = Some(idx);
                break;
            }
        }
    }

    // 4. Extract and rewrite consumer statements from body_b
    let mut consumer_stmts = Vec::new();
    if let Some(body_b_block) = func.blocks.iter().find(|b| b.id == body_b_id) {
        for stmt in &body_b_block.statements {
            let Statement::Assign(dest, rval) = stmt;
            let real_dest = resolve_alias(&dest.local, &aliases);

            // Skip consumer induction variable increment
            if real_dest == iv_b.local {
                if let Rvalue::BinaryOp(BinaryOp::Add, l, _) = rval {
                    if resolve_alias(&l.local, &aliases) == iv_b.local {
                        continue;
                    }
                }
            }

            // Rewrite uses of buf[k] => val_temp, and iv_b => iv_a
            let mut rewritten_rval = rval.clone();
            rewrite_rval_uses(&mut rewritten_rval, &buf_name, &val_temp, &iv_b, &iv_a, &aliases);
            let mut rewritten_dest = dest.clone();
            for proj in &mut rewritten_dest.projections {
                if let Projection::Index(idx_box) = proj {
                    if resolve_alias(&idx_box.local, &aliases) == iv_b.local {
                        **idx_box = iv_a.clone();
                    }
                }
            }
            consumer_stmts.push(Statement::Assign(rewritten_dest, rewritten_rval));
        }
    }

    // 5. Splice consumer statements into body_a right after the producer statement
    if let Some(body_a_block) = func.blocks.iter_mut().find(|b| b.id == body_a_id) {
        let insert_idx = producer_stmt_idx.map(|idx| idx + 1).unwrap_or(0);
        for (offset, c_stmt) in consumer_stmts.into_iter().enumerate() {
            body_a_block.statements.insert(insert_idx + offset, c_stmt);
        }
    }

    // 6. Hoist initializations between loop A and loop B into loop A's preheader
    let preheader_a_id = preds.get(&candidate.producer_header).and_then(|p_list| {
        p_list.iter().find(|p| !blocks_a.contains(p)).cloned()
    });

    let mut hoisted_stmts = Vec::new();
    if let Some(exit_a_block) = func.blocks.iter_mut().find(|b| b.id == exit_a_id) {
        let mut remaining_stmts = Vec::new();
        for stmt in exit_a_block.statements.drain(..) {
            let Statement::Assign(dest, _) = &stmt;
            let real_dest = resolve_alias(&dest.local, &aliases);
            if real_dest != iv_b.local && real_dest != buf_name {
                hoisted_stmts.push(stmt);
            } else {
                remaining_stmts.push(stmt);
            }
        }
        exit_a_block.statements = remaining_stmts;
    }

    if let Some(pre_id) = preheader_a_id {
        if let Some(pre_block) = func.blocks.iter_mut().find(|b| b.id == pre_id) {
            for h_stmt in hoisted_stmts {
                pre_block.statements.push(h_stmt);
            }
        }
    }

    // 7. Redirect Loop A exit directly to Loop B exit (bypassing loop B entirely)
    if let Some(hb_a) = func.blocks.iter_mut().find(|b| b.id == candidate.producer_header) {
        if let Terminator::BranchIf { then_target, else_target, .. } = &mut hb_a.terminator {
            if *else_target == exit_a_id {
                *else_target = exit_b_id.clone();
            } else if *then_target == exit_a_id {
                *then_target = exit_b_id.clone();
            }
        }
    }

    if let Some(exit_a_block) = func.blocks.iter_mut().find(|b| b.id == exit_a_id) {
        exit_a_block.terminator = Terminator::Branch {
            target: exit_b_id,
        };
    }

    // 8. Eliminate the intermediate buffer allocation
    for b in &mut func.blocks {
        b.statements.retain(|stmt| {
            let Statement::Assign(dest, rval) = stmt;
            let real_dest = resolve_alias(&dest.local, &aliases);
            if real_dest == buf_name {
                return !matches!(rval, Rvalue::Array(_));
            }
            true
        });
    }
}

fn rewrite_rval_uses(
    rval: &mut Rvalue,
    buf_name: &str,
    val_temp: &Place,
    iv_b: &Place,
    iv_a: &Place,
    aliases: &HashMap<String, String>,
) {
    let rewrite_place = |p: &mut Place| {
        let real_name = resolve_alias(&p.local, aliases);
        if real_name == buf_name && !p.projections.is_empty() {
            *p = val_temp.clone();
        } else if real_name == iv_b.local {
            *p = iv_a.clone();
        }
    };

    match rval {
        Rvalue::Use(p) => rewrite_place(p),
        Rvalue::BinaryOp(_, l, r) => {
            rewrite_place(l);
            rewrite_place(r);
        }
        Rvalue::UnaryOp(_, p) => rewrite_place(p),
        Rvalue::Call(_, args) => {
            for arg in args {
                rewrite_place(arg);
            }
        }
        _ => {}
    }
}

/// Detects and fuses `map(f, filter(p, xs))` chains into a single pass,
/// eliminating the intermediate buffer created by conditional filtering.
struct MapFilterPlan {
    tmp_buf: String,
    counter_j: Place,
    filter_then_block: BasicBlockId,
    iv_k: Place,
    h2: BasicBlockId,
    exit2_id: BasicBlockId,
    preheader1_id: BasicBlockId,
    exit1_id: BasicBlockId,
    map_transforms: Vec<Statement>,
}

fn find_map_filter_plan(func: &MirFunction) -> Option<MapFilterPlan> {
    if func.blocks.len() < 3 {
        return None;
    }

    let aliases = build_alias_map(func);
    let (preds, succs) = compute_cfg(&func.blocks);
    let all_block_ids: Vec<BasicBlockId> = func.blocks.iter().map(|b| b.id.clone()).collect();
    let entry = func.blocks[0].id.clone();
    let dom = compute_dominance(entry, &preds, &succs, &all_block_ids);
    let loop_info = detect_loops(func.blocks[0].id.clone(), &preds, &succs, &all_block_ids, &dom);

    if loop_info.headers.len() < 2 {
        return None;
    }

    let block_map: HashMap<BasicBlockId, &crate::mir::lower::MirBasicBlock> =
        func.blocks.iter().map(|b| (b.id.clone(), b)).collect();

    // Look for filter loop (writes to tmp[j] conditionally) and map loop (iterates k < j reading tmp[k])
    for h1 in &loop_info.headers {
        let blocks1 = match loop_info.natural_loops.get(h1) {
            Some(b) => b,
            None => continue,
        };

        // Find secondary counter j and intermediate buffer written in loop 1
        let mut tmp_buf_opt = None;
        let mut filter_then_block = None;

        for b_id in blocks1 {
            if let Some(b) = block_map.get(b_id) {
                for stmt in &b.statements {
                    let Statement::Assign(dest, _) = stmt;
                    if dest.projections.len() == 1 {
                        if let Projection::Index(idx_place) = &dest.projections[0] {
                            let real_counter = resolve_alias(&idx_place.local, &aliases);
                            let real_buf = resolve_alias(&dest.local, &aliases);
                            tmp_buf_opt = Some((
                                real_buf.to_string(),
                                Place {
                                    local: real_counter.to_string(),
                                    projections: vec![],
                                },
                            ));
                            filter_then_block = Some(b_id.clone());
                        }
                    }
                }
            }
        }

        let (tmp_buf, counter_j) = match tmp_buf_opt {
            Some(pair) => pair,
            None => continue,
        };
        let filter_then_block = match filter_then_block {
            Some(b) => b,
            None => continue,
        };

        let exit1_id = match block_map.get(h1) {
            Some(hb1) => {
                if let Terminator::BranchIf { then_target, else_target, .. } = &hb1.terminator {
                    if blocks1.contains(then_target) {
                        else_target.clone()
                    } else {
                        then_target.clone()
                    }
                } else {
                    continue;
                }
            }
            None => continue,
        };

        let preheader1_id = preds.get(h1).and_then(|p_list| {
            p_list.iter().find(|p| !blocks1.contains(p)).cloned()
        }).unwrap_or(BasicBlockId(0));

        // Look for map loop (h2) where bound is counter_j and reads from tmp_buf
        for h2 in &loop_info.headers {
            if h1 == h2 {
                continue;
            }
            let blocks2 = match loop_info.natural_loops.get(h2) {
                Some(b) => b,
                None => continue,
            };

            let hb2 = match block_map.get(h2) {
                Some(b) => b,
                None => continue,
            };

            if let Terminator::BranchIf { condition, then_target, else_target } = &hb2.terminator {
                let mut bound_is_j = false;
                let mut iv_k_opt = None;

                for stmt in &hb2.statements {
                    if let Statement::Assign(dest, Rvalue::BinaryOp(op, l, r)) = stmt {
                        let real_r = resolve_alias(&r.local, &aliases);
                        let real_l = resolve_alias(&l.local, &aliases);
                        if dest == condition
                            && (*op == BinaryOp::Lt || *op == BinaryOp::Le)
                            && real_r == counter_j.local
                        {
                            bound_is_j = true;
                            iv_k_opt = Some(Place {
                                local: real_l.to_string(),
                                projections: vec![],
                            });
                            break;
                        }
                    }
                }

                if !bound_is_j {
                    continue;
                }
                let iv_k = match iv_k_opt {
                    Some(k) => k,
                    None => continue,
                };

                let body2_id = if blocks2.contains(then_target) { then_target.clone() } else { else_target.clone() };
                let exit2_id = if blocks2.contains(then_target) { else_target.clone() } else { then_target.clone() };

                let body2_block = match block_map.get(&body2_id) {
                    Some(b) => b,
                    None => continue,
                };

                let mut map_transforms = Vec::new();
                for stmt in &body2_block.statements {
                    let Statement::Assign(dest, rval) = stmt;
                    let real_dest = resolve_alias(&dest.local, &aliases);
                    if real_dest == iv_k.local {
                        continue;
                    }
                    map_transforms.push(Statement::Assign(dest.clone(), rval.clone()));
                }

                if map_transforms.is_empty() {
                    continue;
                }

                // Check no external uses of tmp_buf
                if has_external_uses(&tmp_buf, blocks1, blocks2, func, &aliases) {
                    continue;
                }

                return Some(MapFilterPlan {
                    tmp_buf,
                    counter_j,
                    filter_then_block,
                    iv_k,
                    h2: h2.clone(),
                    exit2_id,
                    preheader1_id,
                    exit1_id,
                    map_transforms,
                });
            }
        }
    }

    None
}

/// Detects and fuses `map(f, filter(p, xs))` chains into a single pass,
/// eliminating the intermediate buffer created by conditional filtering.
pub fn fuse_map_filter(func: &mut MirFunction) -> bool {
    let plan = match find_map_filter_plan(func) {
        Some(p) => p,
        None => return false,
    };
    let aliases = build_alias_map(func);

    // 1. Fuse map into filter's then block
    if let Some(then_b) = func.blocks.iter_mut().find(|b| b.id == plan.filter_then_block) {
        let mut src_val_opt = None;
        let mut assign_idx = None;

        for (idx, stmt) in then_b.statements.iter().enumerate() {
            let Statement::Assign(dest, rval) = stmt;
            let real_dest = resolve_alias(&dest.local, &aliases);
            if real_dest == plan.tmp_buf && !dest.projections.is_empty() {
                src_val_opt = match rval {
                    Rvalue::Use(p) => Some(p.clone()),
                    _ => None,
                };
                assign_idx = Some(idx);
                break;
            }
        }

        if let (Some(src_val), Some(idx)) = (src_val_opt, assign_idx) {
            then_b.statements.remove(idx);
            for (offset, m_stmt) in plan.map_transforms.into_iter().enumerate() {
                let Statement::Assign(mut dest_rewritten, mut rval) = m_stmt;
                rewrite_rval_uses(&mut rval, &plan.tmp_buf, &src_val, &plan.iv_k, &plan.counter_j, &aliases);
                for proj in &mut dest_rewritten.projections {
                    if let Projection::Index(idx_box) = proj {
                        let real_idx = resolve_alias(&idx_box.local, &aliases);
                        if real_idx == plan.iv_k.local {
                            **idx_box = plan.counter_j.clone();
                        }
                    }
                }
                then_b.statements.insert(idx + offset, Statement::Assign(dest_rewritten, rval));
            }
        }
    }

    // 1b. Hoist initializations between loop 1 and loop 2 into loop 1's preheader
    let mut hoisted_stmts = Vec::new();
    if let Some(exit1_b) = func.blocks.iter_mut().find(|b| b.id == plan.exit1_id) {
        let mut remaining_stmts = Vec::new();
        for stmt in exit1_b.statements.drain(..) {
            let Statement::Assign(dest, _) = &stmt;
            let real_dest = resolve_alias(&dest.local, &aliases);
            if real_dest != plan.iv_k.local && real_dest != plan.tmp_buf {
                hoisted_stmts.push(stmt);
            } else {
                remaining_stmts.push(stmt);
            }
        }
        exit1_b.statements = remaining_stmts;
    }

    if let Some(pre_block) = func.blocks.iter_mut().find(|b| b.id == plan.preheader1_id) {
        for h_stmt in hoisted_stmts {
            pre_block.statements.push(h_stmt);
        }
    }

    // 2. Bypass loop 2
    for b in &mut func.blocks {
        match &mut b.terminator {
            Terminator::Branch { target } if *target == plan.h2 => {
                *target = plan.exit2_id.clone();
            }
            Terminator::BranchIf { then_target, else_target, .. } => {
                if *then_target == plan.h2 {
                    *then_target = plan.exit2_id.clone();
                }
                if *else_target == plan.h2 {
                    *else_target = plan.exit2_id.clone();
                }
            }
            _ => {}
        }
    }

    // 3. Remove tmp_buf allocation
    for b in &mut func.blocks {
        b.statements.retain(|stmt| {
            let Statement::Assign(dest, rval) = stmt;
            let real_dest = resolve_alias(&dest.local, &aliases);
            if real_dest == plan.tmp_buf {
                return !matches!(rval, Rvalue::Array(_));
            }
            true
        });
    }

    true
}
