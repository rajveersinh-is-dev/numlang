use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::ast::BinaryOp;
use crate::mir::dominance::compute_dominance;
use crate::mir::lower::{MirBasicBlock, MirFunction, Rvalue, Statement};
use crate::mir::{compute_cfg, BasicBlockId, Place, Projection, Terminator};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryVersionId(pub usize);

impl MemoryVersionId {
    pub const LIVE_ON_ENTRY: MemoryVersionId = MemoryVersionId(0);

    pub fn is_entry(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for MemoryVersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            write!(f, "v0(entry)")
        } else {
            write!(f, "v{}", self.0)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryDef {
    pub id: MemoryVersionId,
    pub incoming: MemoryVersionId,
    pub place: Place,
    pub block: BasicBlockId,
    pub statement_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryUse {
    pub reaching: MemoryVersionId,
    pub place: Place,
    pub block: BasicBlockId,
    pub statement_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryPhi {
    pub id: MemoryVersionId,
    pub block: BasicBlockId,
    pub incoming: Vec<(BasicBlockId, MemoryVersionId)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryAccess {
    Def(MemoryDef),
    Use(MemoryUse),
    Phi(MemoryPhi),
}

impl MemoryAccess {
    pub fn version_id(&self) -> MemoryVersionId {
        match self {
            MemoryAccess::Def(d) => d.id,
            MemoryAccess::Use(u) => u.reaching,
            MemoryAccess::Phi(p) => p.id,
        }
    }

    pub fn block(&self) -> &BasicBlockId {
        match self {
            MemoryAccess::Def(d) => &d.block,
            MemoryAccess::Use(u) => &u.block,
            MemoryAccess::Phi(p) => &p.block,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemorySSA {
    pub num_versions: usize,
    pub block_phis: HashMap<BasicBlockId, MemoryPhi>,
    pub stmt_defs: HashMap<(BasicBlockId, usize), MemoryDef>,
    pub stmt_uses: HashMap<(BasicBlockId, usize), Vec<MemoryUse>>,
    pub term_uses: HashMap<BasicBlockId, Vec<MemoryUse>>,
    pub block_entry_versions: HashMap<BasicBlockId, MemoryVersionId>,
    pub block_exit_versions: HashMap<BasicBlockId, MemoryVersionId>,
}

impl MemorySSA {
    pub fn build(func: &MirFunction) -> Self {
        if func.blocks.is_empty() {
            return MemorySSA {
                num_versions: 1,
                block_phis: HashMap::new(),
                stmt_defs: HashMap::new(),
                stmt_uses: HashMap::new(),
                term_uses: HashMap::new(),
                block_entry_versions: HashMap::new(),
                block_exit_versions: HashMap::new(),
            };
        }

        let entry = func.blocks[0].id.clone();
        let (preds, succs) = compute_cfg(&func.blocks);

        // Find reachable blocks
        let mut reachable = HashSet::new();
        let mut queue = vec![entry.clone()];
        reachable.insert(entry.clone());
        while let Some(curr) = queue.pop() {
            if let Some(s) = succs.get(&curr) {
                for next in s {
                    if reachable.insert(next.clone()) {
                        queue.push(next.clone());
                    }
                }
            }
        }

        let all_block_ids: Vec<BasicBlockId> = func.blocks.iter().map(|b| b.id.clone()).collect();
        let dom = compute_dominance(entry.clone(), &preds, &succs, &all_block_ids);

        // Build dominator tree children
        let mut dom_children: HashMap<BasicBlockId, Vec<BasicBlockId>> = HashMap::new();
        for b in &all_block_ids {
            if !reachable.contains(b) || *b == entry {
                continue;
            }
            if let Some(parent) = dom.idom.get(b) {
                dom_children.entry(parent.clone()).or_default().push(b.clone());
            }
        }

        // Identify join blocks that require MemoryPhi (predecessors >= 2)
        let mut block_phis: HashMap<BasicBlockId, MemoryPhi> = HashMap::new();
        let mut next_version = 1;

        for b in &all_block_ids {
            if !reachable.contains(b) {
                continue;
            }
            let valid_preds_count = preds
                .get(b)
                .map(|p| p.iter().filter(|pred| reachable.contains(*pred)).count())
                .unwrap_or(0);

            if valid_preds_count >= 2 {
                let phi_id = MemoryVersionId(next_version);
                next_version += 1;
                block_phis.insert(
                    b.clone(),
                    MemoryPhi {
                        id: phi_id,
                        block: b.clone(),
                        incoming: Vec::new(),
                    },
                );
            }
        }

        // Map blocks by ID for fast lookup
        let block_map: HashMap<BasicBlockId, &MirBasicBlock> =
            func.blocks.iter().map(|b| (b.id.clone(), b)).collect();

        let mut stmt_defs: HashMap<(BasicBlockId, usize), MemoryDef> = HashMap::new();
        let mut stmt_uses: HashMap<(BasicBlockId, usize), Vec<MemoryUse>> = HashMap::new();
        let mut term_uses: HashMap<BasicBlockId, Vec<MemoryUse>> = HashMap::new();
        let mut block_entry_versions: HashMap<BasicBlockId, MemoryVersionId> = HashMap::new();
        let mut block_exit_versions: HashMap<BasicBlockId, MemoryVersionId> = HashMap::new();

        // Recursive dominator tree traversal
        let mut traversal_stack = vec![(entry.clone(), MemoryVersionId::LIVE_ON_ENTRY)];
        let mut visited = HashSet::new();

        while let Some((curr_block_id, reaching_ver)) = traversal_stack.pop() {
            if !visited.insert(curr_block_id.clone()) {
                continue;
            }

            let mut current_ver = reaching_ver;

            // If block has a MemoryPhi, its phi defines the starting version for statements
            if let Some(phi) = block_phis.get(&curr_block_id) {
                current_ver = phi.id;
            }
            block_entry_versions.insert(curr_block_id.clone(), current_ver);

            // Traverse statements
            if let Some(b) = block_map.get(&curr_block_id) {
                for (stmt_idx, stmt) in b.statements.iter().enumerate() {
                    let Statement::Assign(dest_place, rvalue) = stmt;

                    // 1. Collect reads from rvalue and index projections
                    let reads = collect_reads(rvalue, dest_place);
                    let mut uses = Vec::with_capacity(reads.len());
                    for read_place in reads {
                        uses.push(MemoryUse {
                            reaching: current_ver,
                            place: read_place,
                            block: curr_block_id.clone(),
                            statement_index: stmt_idx,
                        });
                    }
                    if !uses.is_empty() {
                        stmt_uses.insert((curr_block_id.clone(), stmt_idx), uses);
                    }

                    // 2. Define new memory version for dest_place
                    let def_ver = MemoryVersionId(next_version);
                    next_version += 1;
                    let def = MemoryDef {
                        id: def_ver,
                        incoming: current_ver,
                        place: dest_place.clone(),
                        block: curr_block_id.clone(),
                        statement_index: stmt_idx,
                    };
                    stmt_defs.insert((curr_block_id.clone(), stmt_idx), def);
                    current_ver = def_ver;
                }

                // 3. Collect reads from terminator
                let term_reads = collect_terminator_reads(&b.terminator);
                if !term_reads.is_empty() {
                    let uses = term_reads
                        .into_iter()
                        .map(|p| MemoryUse {
                            reaching: current_ver,
                            place: p,
                            block: curr_block_id.clone(),
                            statement_index: b.statements.len(),
                        })
                        .collect();
                    term_uses.insert(curr_block_id.clone(), uses);
                }
            }

            block_exit_versions.insert(curr_block_id.clone(), current_ver);

            // Propagate exit version to successor phis
            if let Some(successors) = succs.get(&curr_block_id) {
                for succ in successors {
                    if let Some(phi) = block_phis.get_mut(succ) {
                        phi.incoming.push((curr_block_id.clone(), current_ver));
                    }
                }
            }

            // Push dom tree children to traversal stack
            if let Some(children) = dom_children.get(&curr_block_id) {
                for child in children {
                    traversal_stack.push((child.clone(), current_ver));
                }
            }
        }

        // Trivial Phi simplification:
        // If a phi has all non-self incoming operands equal to same version V,
        // it can be simplified to V.
        let mut replacements: HashMap<MemoryVersionId, MemoryVersionId> = HashMap::new();
        let mut changed = true;
        while changed {
            changed = false;
            let mut remove_phi_ids = Vec::new();

            for (bid, phi) in &block_phis {
                if phi.incoming.is_empty() {
                    continue;
                }

                let mut unique_incomings: Vec<MemoryVersionId> = phi
                    .incoming
                    .iter()
                    .map(|(_, v)| {
                        let mut curr = *v;
                        while let Some(r) = replacements.get(&curr) {
                            curr = *r;
                        }
                        curr
                    })
                    .filter(|&v| v != phi.id)
                    .collect();

                unique_incomings.sort();
                unique_incomings.dedup();

                if unique_incomings.len() == 1 {
                    let target_ver = unique_incomings[0];
                    replacements.insert(phi.id, target_ver);
                    remove_phi_ids.push(bid.clone());
                    changed = true;
                }
            }

            for bid in remove_phi_ids {
                block_phis.remove(&bid);
            }
        }

        // Apply replacements across all defs, uses, and phis
        if !replacements.is_empty() {
            let canonical = |mut v: MemoryVersionId| -> MemoryVersionId {
                while let Some(r) = replacements.get(&v) {
                    v = *r;
                }
                v
            };

            for def in stmt_defs.values_mut() {
                def.incoming = canonical(def.incoming);
            }
            for use_list in stmt_uses.values_mut() {
                for u in use_list {
                    u.reaching = canonical(u.reaching);
                }
            }
            for use_list in term_uses.values_mut() {
                for u in use_list {
                    u.reaching = canonical(u.reaching);
                }
            }
            for phi in block_phis.values_mut() {
                for (_, v) in &mut phi.incoming {
                    *v = canonical(*v);
                }
            }
            for v in block_entry_versions.values_mut() {
                *v = canonical(*v);
            }
            for v in block_exit_versions.values_mut() {
                *v = canonical(*v);
            }
        }

        MemorySSA {
            num_versions: next_version,
            block_phis,
            stmt_defs,
            stmt_uses,
            term_uses,
            block_entry_versions,
            block_exit_versions,
        }
    }

    pub fn get_stmt_def(&self, block: &BasicBlockId, stmt_idx: usize) -> Option<&MemoryDef> {
        self.stmt_defs.get(&(block.clone(), stmt_idx))
    }

    pub fn get_stmt_uses(&self, block: &BasicBlockId, stmt_idx: usize) -> &[MemoryUse] {
        self.stmt_uses
            .get(&(block.clone(), stmt_idx))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn get_block_phi(&self, block: &BasicBlockId) -> Option<&MemoryPhi> {
        self.block_phis.get(block)
    }

    pub fn display(&self, func: &MirFunction) -> String {
        let mut out = String::new();
        out.push_str(&format!("// MemorySSA for function '{}'\n", func.name));
        out.push_str(&format!(
            "// Total memory versions allocated: {}\n",
            self.num_versions
        ));

        for block in &func.blocks {
            out.push_str(&format!("\nbb{}:\n", block.id.0));

            // Block phi
            if let Some(phi) = self.block_phis.get(&block.id) {
                let incoming_str: Vec<String> = phi
                    .incoming
                    .iter()
                    .map(|(b, v)| format!("[bb{}: {}]", b.0, v))
                    .collect();
                out.push_str(&format!(
                    "  {} = MemoryPhi({})\n",
                    phi.id,
                    incoming_str.join(", ")
                ));
            } else if let Some(entry_ver) = self.block_entry_versions.get(&block.id) {
                out.push_str(&format!("  // Entry memory: {}\n", entry_ver));
            }

            // Statements
            for (idx, stmt) in block.statements.iter().enumerate() {
                // Uses
                if let Some(uses) = self.stmt_uses.get(&(block.id.clone(), idx)) {
                    for u in uses {
                        out.push_str(&format!(
                            "  // MemoryUse({}, place={})\n",
                            u.reaching,
                            format_place(&u.place)
                        ));
                    }
                }

                // Statement itself
                match stmt {
                    Statement::Assign(p, rv) => {
                        out.push_str(&format!(
                            "  {} = {};",
                            format_place(p),
                            format_rvalue(rv)
                        ));
                    }
                }

                // Def
                if let Some(def) = self.stmt_defs.get(&(block.id.clone(), idx)) {
                    out.push_str(&format!(
                        " // MemoryDef({}, incoming={}, place={})\n",
                        def.id,
                        def.incoming,
                        format_place(&def.place)
                    ));
                } else {
                    out.push('\n');
                }
            }

            // Terminator uses
            if let Some(t_uses) = self.term_uses.get(&block.id) {
                for u in t_uses {
                    out.push_str(&format!(
                        "  // Terminator MemoryUse({}, place={})\n",
                        u.reaching,
                        format_place(&u.place)
                    ));
                }
            }

            // Terminator
            out.push_str(&format!("  {:?}\n", block.terminator));
            if let Some(exit_ver) = self.block_exit_versions.get(&block.id) {
                out.push_str(&format!("  // Exit memory: {}\n", exit_ver));
            }
        }

        out
    }
}

fn collect_reads(rv: &Rvalue, dest: &Place) -> Vec<Place> {
    let mut reads = Vec::new();

    // Any indices in dest projections are reads
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

fn collect_terminator_reads(term: &Terminator) -> Vec<Place> {
    match term {
        Terminator::Branch { .. } | Terminator::Unreachable | Terminator::Fork { .. } => vec![],
        Terminator::BranchIf { condition, .. } => vec![condition.clone()],
        Terminator::Switch { value, .. } => vec![value.clone()],
        Terminator::Return { value } => value.iter().cloned().collect(),
        Terminator::IndirectCall { callee, args, .. } => {
            let mut r = vec![callee.clone()];
            r.extend(args.iter().cloned());
            r
        }
        Terminator::Force { thunk, .. } => {
            vec![Place { local: thunk.clone(), projections: vec![] }]
        }
        Terminator::TypeGuard { local, .. } => {
            vec![local.clone()]
        }
    }
}

fn format_place(place: &Place) -> String {
    let mut s = place.local.clone();
    for proj in &place.projections {
        match proj {
            Projection::Deref => s = format!("(*{})", s),
            Projection::Field(f) => s = format!("{}.{}", s, f),
            Projection::Index(idx) => s = format!("{}[{}]", s, format_place(idx)),
            Projection::Payload(idx) => s = format!("{}.payload_{}", s, idx),
        }
    }
    s
}

fn format_rvalue(rv: &Rvalue) -> String {
    match rv {
        Rvalue::Use(p) => format_place(p),
        Rvalue::BinaryOp(op, p1, p2) => {
            let op_str = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "%",
                BinaryOp::Pow => "**",
                BinaryOp::Eq => "==",
                BinaryOp::Ne => "!=",
                BinaryOp::Lt => "<",
                BinaryOp::Le => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::Ge => ">=",
                BinaryOp::BitAnd => "&",
                BinaryOp::BitOr => "|",
                BinaryOp::BitXor => "^",
                BinaryOp::Shl => "<<",
                BinaryOp::Shr => ">>",
            };
            format!("{} {} {}", format_place(p1), op_str, format_place(p2))
        }
        Rvalue::UnaryOp(op, p) => format!("{:?} {}", op, format_place(p)),
        Rvalue::Constant(c) => format!("{:?}", c),
        Rvalue::Call(name, args) => {
            let args_str: Vec<String> = args.iter().map(format_place).collect();
            format!("{}({})", name, args_str.join(", "))
        }
        Rvalue::Array(elems) => {
            let elems_str: Vec<String> = elems.iter().map(format_place).collect();
            format!("[{}]", elems_str.join(", "))
        }
        Rvalue::Struct(name, fields) => {
            let fields_str: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_place(v)))
                .collect();
            format!("{} {{ {} }}", name, fields_str.join(", "))
        }
        Rvalue::EnumVariant { enum_name, variant_name, fields, .. } => {
            let fields_str: Vec<String> = fields.iter().map(format_place).collect();
            format!("{}::{}({})", enum_name, variant_name, fields_str.join(", "))
        }
        Rvalue::Discriminant(p) => {
            format!("discriminant({})", format_place(p))
        }
        Rvalue::Phi(incoming) => {
            let inc_str: Vec<String> = incoming
                .iter()
                .map(|(b, p)| format!("[bb{}: {}]", b.0, format_place(p)))
                .collect();
            format!("phi({})", inc_str.join(", "))
        }
        Rvalue::FnPtr(name) => format!("fn_ptr({})", name),
        Rvalue::ClosureAlloc { fn_name, captured } => {
            let cap_str: Vec<String> = captured.iter().map(format_place).collect();
            format!("closure_alloc({}, [{}])", fn_name, cap_str.join(", "))
        }
        Rvalue::Alloc(p) => format!("alloc({})", format_place(p)),
        Rvalue::Load(p) => format!("load({})", format_place(p)),
        Rvalue::Thunk { body, env } => {
            format!("thunk({}; env=[{}])", body, env.join(", "))
        }
    }
}
