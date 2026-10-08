use std::collections::{HashMap, HashSet};

use crate::mir::alias::AliasAnalysis;
use crate::mir::dominance::compute_dominance;
use crate::mir::lower::{MirFunction, Rvalue, Statement};
use crate::mir::{compute_cfg, BasicBlockId, Place, Projection};
use crate::typecheck::types::Type;

/// Computes the Iterated Dominance Frontier (IDF) for a set of defining blocks.
pub fn compute_idf(
    defs: &HashSet<BasicBlockId>,
    dominance_frontiers: &HashMap<BasicBlockId, HashSet<BasicBlockId>>,
) -> HashSet<BasicBlockId> {
    let mut idf = HashSet::new();
    let mut worklist: Vec<BasicBlockId> = defs.iter().cloned().collect();

    while let Some(b) = worklist.pop() {
        if let Some(df_b) = dominance_frontiers.get(&b) {
            for y in df_b {
                if idf.insert(y.clone()) && !defs.contains(y) {
                    worklist.push(y.clone());
                }
            }
        }
    }

    idf
}

/// Identifies candidate memory places suitable for scalar SSA register promotion.
pub fn find_promotion_candidates(func: &MirFunction) -> Vec<Place> {
    let mut non_promotable = HashSet::new();
    let mut all_places = HashSet::new();

    // Any place that uses dynamic array indexing or deref is not a candidate
    for block in &func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(dest, rval) = stmt;
            check_place_promotability(dest, &mut all_places, &mut non_promotable);
            for read_p in collect_all_places(rval) {
                check_place_promotability(&read_p, &mut all_places, &mut non_promotable);
            }
        }
    }

    let mut candidates: Vec<Place> = all_places
        .into_iter()
        .filter(|p| {
            !non_promotable.contains(&p.local)
                && (p.projections.is_empty()
                    || (p.projections.len() == 1
                        && matches!(p.projections[0], Projection::Field(_))))
        })
        .collect();

    candidates.sort_by(|a, b| {
        a.local
            .cmp(&b.local)
            .then_with(|| format!("{:?}", a.projections).cmp(&format!("{:?}", b.projections)))
    });
    candidates
}

fn check_place_promotability(
    p: &Place,
    all_places: &mut HashSet<Place>,
    non_promotable: &mut HashSet<String>,
) {
    for proj in &p.projections {
        match proj {
            Projection::Index(_) | Projection::Deref => {
                non_promotable.insert(p.local.clone());
            }
            Projection::Field(_) | Projection::Payload(_) => {}
        }
    }
    all_places.insert(p.clone());
}

fn collect_all_places(rv: &Rvalue) -> Vec<Place> {
    let mut places = Vec::new();
    match rv {
        Rvalue::Use(p) => places.push(p.clone()),
        Rvalue::BinaryOp(_, p1, p2) => {
            places.push(p1.clone());
            places.push(p2.clone());
        }
        Rvalue::UnaryOp(_, p) => places.push(p.clone()),
        Rvalue::Constant(_) => {}
        Rvalue::Call(_, args) => places.extend(args.iter().cloned()),
        Rvalue::Array(elems) => places.extend(elems.iter().cloned()),
        Rvalue::Struct(_, fields) => {
            for (_, p) in fields {
                places.push(p.clone());
            }
        }
        Rvalue::EnumVariant { fields, .. } => {
            places.extend(fields.iter().cloned());
        }
        Rvalue::Discriminant(p) => {
            places.push(p.clone());
        }
        Rvalue::Phi(incoming) => {
            for (_, p) in incoming {
                places.push(p.clone());
            }
        }
        Rvalue::FnPtr(_) => {}
        Rvalue::ClosureAlloc { captured, .. } => {
            places.extend(captured.iter().cloned());
        }
        Rvalue::Alloc(p) | Rvalue::Load(p) => {
            places.push(p.clone());
        }
        Rvalue::Thunk { env, .. } => {
            for e in env {
                places.push(Place {
                    local: e.clone(),
                    projections: vec![],
                });
            }
        }
    }
    places
}

/// Executes Mem2Reg SSA promotion: promotes non-aliased stack variables and struct fields
/// to pure SSA registers, inserting block arguments / Phi nodes at iterated dominance frontiers.
pub fn promote_memory_to_registers(func: &mut MirFunction) {
    if func.blocks.is_empty() {
        return;
    }

    let entry = func.blocks[0].id.clone();
    let (preds, succs) = compute_cfg(&func.blocks);
    let all_block_ids: Vec<BasicBlockId> = func.blocks.iter().map(|b| b.id.clone()).collect();
    let dom = compute_dominance(entry.clone(), &preds, &succs, &all_block_ids);

    let candidates = find_promotion_candidates(func);
    if candidates.is_empty() {
        return;
    }

    // Map each candidate place to the set of blocks where it is assigned
    let mut place_defs: HashMap<Place, HashSet<BasicBlockId>> = HashMap::new();
    for block in &func.blocks {
        for stmt in &block.statements {
            let Statement::Assign(dest, _) = stmt;
            if candidates.contains(dest) {
                place_defs
                    .entry(dest.clone())
                    .or_default()
                    .insert(block.id.clone());
            }
        }
    }

    // Determine blocks requiring Phi nodes for each candidate
    let mut phi_nodes: HashMap<BasicBlockId, Vec<(Place, Place)>> = HashMap::new(); // block -> vec![(candidate, phi_temp)]
    let mut next_phi_id = 0;

    for candidate in &candidates {
        if let Some(defs) = place_defs.get(candidate) {
            let idf = compute_idf(defs, &dom.dominance_frontiers);
            for b in idf {
                let phi_temp_name = format!("_phi_{}_{}", candidate.local, next_phi_id);
                next_phi_id += 1;
                let phi_place = Place {
                    local: phi_temp_name.clone(),
                    projections: vec![],
                };

                // Add to block arguments
                if let Some(target_block) = func.blocks.iter_mut().find(|blk| blk.id == b) {
                    target_block
                        .arguments
                        .push((phi_temp_name.clone(), Type::I64)); // Type::I64 default scalar
                }

                phi_nodes
                    .entry(b)
                    .or_default()
                    .push((candidate.clone(), phi_place));
            }
        }
    }

    // Build dominator tree children
    let mut dom_children: HashMap<BasicBlockId, Vec<BasicBlockId>> = HashMap::new();
    for b in &all_block_ids {
        if *b == entry {
            continue;
        }
        if let Some(parent) = dom.idom.get(b) {
            dom_children
                .entry(parent.clone())
                .or_default()
                .push(b.clone());
        }
    }

    // Dominator tree traversal with renaming stacks
    let mut reaching_values: HashMap<Place, Vec<Place>> = HashMap::new();
    let mut phi_incoming_map: HashMap<(BasicBlockId, Place), Vec<(BasicBlockId, Place)>> =
        HashMap::new();

    let mut stmts_to_remove: HashSet<(BasicBlockId, usize)> = HashSet::new();
    let mut temp_replacements: HashMap<Place, Place> = HashMap::new();

    // Map block positions
    let block_indices: HashMap<BasicBlockId, usize> = func
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id.clone(), i))
        .collect();

    rename_in_dom_tree(
        &entry,
        &dom_children,
        &succs,
        &phi_nodes,
        &candidates,
        func,
        &block_indices,
        &mut reaching_values,
        &mut phi_incoming_map,
        &mut stmts_to_remove,
        &mut temp_replacements,
    );

    // Apply statement removal (stores that have been promoted into SSA values)
    for (bid, b_idx) in &block_indices {
        let block = &mut func.blocks[*b_idx];
        let mut new_stmts = Vec::new();

        // 1. Prepend Phi instructions if any were inserted for this block
        if let Some(phis) = phi_nodes.get(bid) {
            for (candidate, phi_place) in phis {
                let incoming = phi_incoming_map
                    .remove(&(bid.clone(), candidate.clone()))
                    .unwrap_or_default();
                new_stmts.push(Statement::Assign(phi_place.clone(), Rvalue::Phi(incoming)));
            }
        }

        // 2. Retain non-removed statements with rewritten uses
        for (stmt_idx, mut stmt) in block.statements.drain(..).enumerate() {
            if !stmts_to_remove.contains(&(bid.clone(), stmt_idx)) {
                rewrite_statement_places(&mut stmt, &temp_replacements);
                new_stmts.push(stmt);
            }
        }

        block.statements = new_stmts;
    }
}

#[allow(clippy::too_many_arguments)]
fn rename_in_dom_tree(
    u: &BasicBlockId,
    dom_children: &HashMap<BasicBlockId, Vec<BasicBlockId>>,
    succs: &HashMap<BasicBlockId, Vec<BasicBlockId>>,
    phi_nodes: &HashMap<BasicBlockId, Vec<(Place, Place)>>,
    candidates: &[Place],
    func: &MirFunction,
    block_indices: &HashMap<BasicBlockId, usize>,
    reaching_values: &mut HashMap<Place, Vec<Place>>,
    phi_incoming_map: &mut HashMap<(BasicBlockId, Place), Vec<(BasicBlockId, Place)>>,
    stmts_to_remove: &mut HashSet<(BasicBlockId, usize)>,
    temp_replacements: &mut HashMap<Place, Place>,
) {
    let mut pushed_counts: HashMap<Place, usize> = HashMap::new();

    // 1. Phis defined in this block become the current reaching values
    if let Some(phis) = phi_nodes.get(u) {
        for (candidate, phi_place) in phis {
            reaching_values
                .entry(candidate.clone())
                .or_default()
                .push(phi_place.clone());
            *pushed_counts.entry(candidate.clone()).or_default() += 1;
        }
    }

    // 2. Traverse statements in block u
    let block = &func.blocks[block_indices[u]];
    for (stmt_idx, stmt) in block.statements.iter().enumerate() {
        let Statement::Assign(dest, rval) = stmt;

        // If statement is a load from a candidate place: _t = Use(candidate)
        if let Rvalue::Use(src_place) = rval {
            if candidates.contains(src_place) {
                if let Some(stack) = reaching_values.get(src_place) {
                    if let Some(reaching) = stack.last() {
                        temp_replacements.insert(dest.clone(), reaching.clone());
                        stmts_to_remove.insert((u.clone(), stmt_idx));
                        continue;
                    }
                }
            }
        }

        // If statement is a store to a candidate place: candidate = ...
        if candidates.contains(dest) {
            // Reaching value is the assigned value
            let val_place = match rval {
                Rvalue::Use(p) => {
                    let mut p_curr = p.clone();
                    while let Some(r) = temp_replacements.get(&p_curr) {
                        p_curr = r.clone();
                    }
                    p_curr
                }
                _ => dest.clone(),
            };

            reaching_values
                .entry(dest.clone())
                .or_default()
                .push(val_place);
            *pushed_counts.entry(dest.clone()).or_default() += 1;

            if matches!(rval, Rvalue::Use(_)) {
                stmts_to_remove.insert((u.clone(), stmt_idx));
            }
        }
    }

    // 3. For each successor of u, record incoming phi operands
    if let Some(successors) = succs.get(u) {
        for succ in successors {
            if let Some(phis) = phi_nodes.get(succ) {
                for (candidate, _) in phis {
                    let reaching = reaching_values
                        .get(candidate)
                        .and_then(|s| s.last())
                        .cloned()
                        .unwrap_or_else(|| candidate.clone());

                    phi_incoming_map
                        .entry((succ.clone(), candidate.clone()))
                        .or_default()
                        .push((u.clone(), reaching));
                }
            }
        }
    }

    // 4. Recurse down dominator tree
    if let Some(children) = dom_children.get(u) {
        for child in children {
            rename_in_dom_tree(
                child,
                dom_children,
                succs,
                phi_nodes,
                candidates,
                func,
                block_indices,
                reaching_values,
                phi_incoming_map,
                stmts_to_remove,
                temp_replacements,
            );
        }
    }

    // 5. Pop reaching values pushed in this block
    for (candidate, count) in pushed_counts {
        if let Some(stack) = reaching_values.get_mut(&candidate) {
            for _ in 0..count {
                stack.pop();
            }
        }
    }
}

fn rewrite_statement_places(stmt: &mut Statement, replacements: &HashMap<Place, Place>) {
    let Statement::Assign(_, rval) = stmt;
    match rval {
        Rvalue::Use(p) => {
            if let Some(r) = replacements.get(p) {
                *p = r.clone();
            }
        }
        Rvalue::BinaryOp(_, p1, p2) => {
            if let Some(r) = replacements.get(p1) {
                *p1 = r.clone();
            }
            if let Some(r) = replacements.get(p2) {
                *p2 = r.clone();
            }
        }
        Rvalue::UnaryOp(_, p) => {
            if let Some(r) = replacements.get(p) {
                *p = r.clone();
            }
        }
        Rvalue::Call(_, args) => {
            for arg in args {
                if let Some(r) = replacements.get(arg) {
                    *arg = r.clone();
                }
            }
        }
        Rvalue::Array(elems) => {
            for elem in elems {
                if let Some(r) = replacements.get(elem) {
                    *elem = r.clone();
                }
            }
        }
        Rvalue::Struct(_, fields) => {
            for (_, p) in fields {
                if let Some(r) = replacements.get(p) {
                    *p = r.clone();
                }
            }
        }
        Rvalue::EnumVariant { fields, .. } => {
            for p in fields {
                if let Some(r) = replacements.get(p) {
                    *p = r.clone();
                }
            }
        }
        Rvalue::Discriminant(p) => {
            if let Some(r) = replacements.get(p) {
                *p = r.clone();
            }
        }
        Rvalue::Phi(incoming) => {
            for (_, p) in incoming {
                if let Some(r) = replacements.get(p) {
                    *p = r.clone();
                }
            }
        }
        Rvalue::Constant(_) => {}
        Rvalue::FnPtr(_) => {}
        Rvalue::ClosureAlloc { captured, .. } => {
            for p in captured {
                if let Some(r) = replacements.get(p) {
                    *p = r.clone();
                }
            }
        }
        Rvalue::Alloc(p) | Rvalue::Load(p) => {
            if let Some(r) = replacements.get(p) {
                *p = r.clone();
            }
        }
        Rvalue::Thunk { env, .. } => {
            for e in env {
                let p = Place {
                    local: e.clone(),
                    projections: vec![],
                };
                if let Some(r) = replacements.get(&p) {
                    *e = r.local.clone();
                }
            }
        }
    }
}

/// Dead Store Elimination (DSE) & Redundant Load Elimination (RLE)
/// Optimized pass utilizing field-sensitive alias analysis.
pub fn eliminate_dead_stores_and_redundant_loads(func: &mut MirFunction) {
    let aa = AliasAnalysis::new(func);

    for block in &mut func.blocks {
        let mut new_stmts = Vec::new();
        let mut available_loads: Vec<(Place, Place)> = Vec::new(); // (target_place, value_place)

        for stmt in block.statements.drain(..) {
            let mut keep_stmt = true;

            match &stmt {
                Statement::Assign(dest, Rvalue::Use(src)) => {
                    // Check if this load is redundant
                    let mut forwarded_place = None;
                    for (avail_p, val_p) in &available_loads {
                        if aa.alias(src, avail_p).is_must_alias() {
                            forwarded_place = Some(val_p.clone());
                            break;
                        }
                    }

                    if let Some(forwarded) = forwarded_place {
                        new_stmts.push(Statement::Assign(dest.clone(), Rvalue::Use(forwarded)));
                        keep_stmt = false;
                    } else {
                        available_loads.push((src.clone(), dest.clone()));
                    }
                }
                Statement::Assign(dest, _) => {
                    // Invalidate available loads that alias this written destination
                    available_loads.retain(|(p, _)| aa.alias(dest, p).is_no_alias());
                }
            }

            if keep_stmt {
                new_stmts.push(stmt);
            }
        }

        // Dead store elimination within block
        let mut final_stmts = Vec::new();
        let n = new_stmts.len();
        let mut dead_indices = HashSet::new();

        for i in 0..n {
            let Statement::Assign(dest_i, _) = &new_stmts[i];

            // Look ahead to see if overwritten before being read
            let mut is_overwritten = false;
            for stmt_j in &new_stmts[(i + 1)..] {
                let modref = aa.modref(stmt_j, dest_i);
                if modref.has_ref() {
                    // Place was read! Cannot eliminate store i.
                    break;
                }
                let Statement::Assign(dest_j, _) = stmt_j;
                if aa.alias(dest_i, dest_j).is_must_alias() {
                    // Overwritten!
                    is_overwritten = true;
                    break;
                }
                if modref.has_mod() {
                    break;
                }
            }

            if is_overwritten {
                dead_indices.insert(i);
            }
        }

        for (idx, stmt) in new_stmts.into_iter().enumerate() {
            if !dead_indices.contains(&idx) {
                final_stmts.push(stmt);
            }
        }

        block.statements = final_stmts;
    }
}
