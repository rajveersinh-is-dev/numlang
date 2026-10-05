use crate::typecheck::types::Type;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct BasicBlockId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Projection {
    Deref,
    Field(String),
    Index(Box<Place>),
    Payload(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Place {
    pub local: String,
    pub projections: Vec<Projection>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Terminator {
    Branch { target: BasicBlockId },
    BranchIf { condition: Place, then_target: BasicBlockId, else_target: BasicBlockId },
    Switch { value: Place, targets: Vec<(i64, BasicBlockId)>, default: BasicBlockId },
    Return { value: Option<Place> },
    Unreachable,
    /// Indirect call through a runtime function pointer or closure fat pointer
    IndirectCall {
        callee: Place,
        args: Vec<Place>,
        dest: Place,
        next: BasicBlockId,
    },
    /// Parallel fork: spawn two independent MIR regions, reconverge at `join`.
    /// Only emitted when --parallel-residualize is active.
    Fork {
        left: BasicBlockId,
        right: BasicBlockId,
        join: BasicBlockId,
    },
    /// Force a thunk: evaluate `thunk` local, bind result to `result`, continue to `cont`.
    Force {
        thunk: String,
        result: String,
        cont: BasicBlockId,
    },
    /// Speculative type guard: check if `local` has type tag `expected_tag`.
    /// If equal, branch to `fast_path`. Otherwise, branch to `deopt_stub`.
    TypeGuard {
        local: Place,
        expected_tag: i64,
        fast_path: BasicBlockId,
        deopt_stub: BasicBlockId,
    },
}

impl Terminator {
    pub fn successors(&self) -> Vec<BasicBlockId> {
        match self {
            Terminator::Branch { target } => vec![target.clone()],
            Terminator::BranchIf { then_target, else_target, .. } => {
                vec![then_target.clone(), else_target.clone()]
            }
            Terminator::Switch { targets, default, .. } => {
                let mut succs = Vec::with_capacity(targets.len() + 1);
                for (_, t) in targets {
                    succs.push(t.clone());
                }
                succs.push(default.clone());
                succs
            }
            Terminator::IndirectCall { next, .. } => vec![next.clone()],
            Terminator::Fork { left, right, join } => {
                vec![left.clone(), right.clone(), join.clone()]
            }
            Terminator::Force { cont, .. } => vec![cont.clone()],
            Terminator::TypeGuard { fast_path, deopt_stub, .. } => {
                vec![fast_path.clone(), deopt_stub.clone()]
            }
            Terminator::Return { .. } | Terminator::Unreachable => vec![],
        }
    }
}

pub fn compute_cfg(
    blocks: &[lower::MirBasicBlock],
) -> (
    std::collections::HashMap<BasicBlockId, Vec<BasicBlockId>>,
    std::collections::HashMap<BasicBlockId, Vec<BasicBlockId>>,
) {
    let mut preds: std::collections::HashMap<BasicBlockId, Vec<BasicBlockId>> =
        std::collections::HashMap::new();
    let mut succs: std::collections::HashMap<BasicBlockId, Vec<BasicBlockId>> =
        std::collections::HashMap::new();

    for b in blocks {
        preds.entry(b.id.clone()).or_default();
        succs.entry(b.id.clone()).or_default();
    }

    for b in blocks {
        let targets = b.terminator.successors();
        for target in &targets {
            preds.entry(target.clone()).or_default().push(b.id.clone());
        }
        succs.insert(b.id.clone(), targets);
    }

    (preds, succs)
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub arguments: Vec<(String, Type)>,
    pub terminator: Terminator,
}

pub struct MirPrinter;

impl MirPrinter {
    pub fn print_place(place: &Place) -> String {
        let mut s = place.local.clone();
        for proj in &place.projections {
            match proj {
                Projection::Deref => s = format!("(*{})", s),
                Projection::Field(f) => s = format!("{}.{}", s, f),
                Projection::Index(idx) => s = format!("{}[{}]", s, Self::print_place(idx)),
                Projection::Payload(idx) => s = format!("{}.payload_{}", s, idx),
            }
        }
        s
    }

    pub fn print_rvalue(rval: &lower::Rvalue) -> String {
        match rval {
            lower::Rvalue::Use(p) => Self::print_place(p),
            lower::Rvalue::BinaryOp(op, l, r) => {
                format!("{:?} {}, {}", op, Self::print_place(l), Self::print_place(r))
            }
            lower::Rvalue::UnaryOp(op, p) => format!("{:?} {}", op, Self::print_place(p)),
            lower::Rvalue::Constant(c) => format!("{:?}", c),
            lower::Rvalue::Call(name, args) => {
                let arg_strs: Vec<_> = args.iter().map(Self::print_place).collect();
                format!("{}({})", name, arg_strs.join(", "))
            }
            lower::Rvalue::Array(elems) => {
                let elem_strs: Vec<_> = elems.iter().map(Self::print_place).collect();
                format!("[{}]", elem_strs.join(", "))
            }
            lower::Rvalue::Struct(name, fields) => {
                let f_strs: Vec<_> = fields
                    .iter()
                    .map(|(f, p)| format!("{}: {}", f, Self::print_place(p)))
                    .collect();
                format!("{} {{ {} }}", name, f_strs.join(", "))
            }
            lower::Rvalue::EnumVariant { enum_name, variant_name, fields, .. } => {
                let f_strs: Vec<_> = fields.iter().map(Self::print_place).collect();
                format!("{}::{}({})", enum_name, variant_name, f_strs.join(", "))
            }
            lower::Rvalue::Discriminant(p) => format!("discriminant({})", Self::print_place(p)),
            lower::Rvalue::Phi(incoming) => {
                let in_strs: Vec<_> = incoming
                    .iter()
                    .map(|(bb, p)| format!("bb{}: {}", bb.0, Self::print_place(p)))
                    .collect();
                format!("phi({})", in_strs.join(", "))
            }
            lower::Rvalue::FnPtr(name) => format!("fn_ptr({})", name),
            lower::Rvalue::ClosureAlloc { fn_name, captured } => {
                let cap_strs: Vec<_> = captured.iter().map(Self::print_place).collect();
                format!("closure_alloc({}; env=[{}])", fn_name, cap_strs.join(", "))
            }
            lower::Rvalue::Alloc(p) => format!("alloc({})", Self::print_place(p)),
            lower::Rvalue::Load(p) => format!("load({})", Self::print_place(p)),
            lower::Rvalue::Thunk { body, env } => {
                format!("thunk({}; env=[{}])", body, env.join(", "))
            }
        }
    }

    pub fn print_terminator(term: &Terminator) -> String {
        match term {
            Terminator::Branch { target } => format!("br bb{}", target.0),
            Terminator::BranchIf { condition, then_target, else_target } => {
                format!("br_if {}, bb{}, bb{}", Self::print_place(condition), then_target.0, else_target.0)
            }
            Terminator::Switch { value, targets, default } => {
                let t_strs: Vec<_> = targets.iter().map(|(v, bb)| format!("{} => bb{}", v, bb.0)).collect();
                format!("switch {}, [{}], default: bb{}", Self::print_place(value), t_strs.join(", "), default.0)
            }
            Terminator::Return { value } => match value {
                Some(p) => format!("ret {}", Self::print_place(p)),
                None => "ret void".to_string(),
            },
            Terminator::Unreachable => "unreachable".to_string(),
            Terminator::IndirectCall { callee, args, dest, next } => {
                let arg_strs: Vec<_> = args.iter().map(Self::print_place).collect();
                format!("{} = indirect_call {}({}), next: bb{}", Self::print_place(dest), Self::print_place(callee), arg_strs.join(", "), next.0)
            }
            Terminator::Fork { left, right, join } => {
                format!("fork bb{}, bb{}, join bb{}", left.0, right.0, join.0)
            }
            Terminator::Force { thunk, result, cont } => {
                format!("{} = force {}, cont: bb{}", result, thunk, cont.0)
            }
            Terminator::TypeGuard { local, expected_tag, fast_path, deopt_stub } => {
                format!("type_guard {} == {}, fast: bb{}, deopt: bb{}", Self::print_place(local), expected_tag, fast_path.0, deopt_stub.0)
            }
        }
    }

    pub fn print_statement(stmt: &lower::Statement) -> String {
        let lower::Statement::Assign(dest, rval) = stmt;
        format!("{} = {}", Self::print_place(dest), Self::print_rvalue(rval))
    }

    pub fn print_function(func: &lower::MirFunction) -> String {
        let mut out = format!("fn {}(...) -> {:?} {{\n", func.name, func.return_ty);
        for block in &func.blocks {
            out.push_str(&format!("  bb{}:\n", block.id.0));
            for stmt in &block.statements {
                out.push_str(&format!("    {};\n", Self::print_statement(stmt)));
            }
            out.push_str(&format!("    {}\n", Self::print_terminator(&block.terminator)));
        }
        out.push_str("}\n");
        out
    }
}

pub fn validate_mir_function(func: &lower::MirFunction) -> Result<(), String> {
    if func.blocks.is_empty() {
        return Err(format!("Function {} has no basic blocks", func.name));
    }
    let block_set: std::collections::HashSet<BasicBlockId> =
        func.blocks.iter().map(|b| b.id.clone()).collect();
    let mut local_set: std::collections::HashSet<String> =
        func.locals.iter().map(|l| l.name.clone()).collect();
    for (p_name, _) in &func.params {
        local_set.insert(p_name.clone());
    }

    for block in &func.blocks {
        for stmt in &block.statements {
            let lower::Statement::Assign(dest, rval) = stmt;
            if !local_set.contains(&dest.local) {
                return Err(format!("Undefined local {} in assignment", dest.local));
            }
            if let lower::Rvalue::Thunk { env, .. } = rval {
                for e in env {
                    if !local_set.contains(e) {
                        return Err(format!("Thunk references undeclared env local {}", e));
                    }
                }
            }
        }
        match &block.terminator {
            Terminator::Force { thunk, result, cont } => {
                if !local_set.contains(thunk) {
                    return Err(format!("Force references undeclared thunk local {}", thunk));
                }
                if !local_set.contains(result) {
                    return Err(format!("Force assigns to undeclared result local {}", result));
                }
                if !block_set.contains(cont) {
                    return Err(format!("Force continuation bb{} does not exist", cont.0));
                }
            }
            Terminator::TypeGuard { local, fast_path, deopt_stub, .. } => {
                if !local_set.contains(&local.local) {
                    return Err(format!("TypeGuard references undeclared local {}", local.local));
                }
                if !block_set.contains(fast_path) {
                    return Err(format!("TypeGuard fast_path bb{} does not exist", fast_path.0));
                }
                if !block_set.contains(deopt_stub) {
                    return Err(format!("TypeGuard deopt_stub bb{} does not exist", deopt_stub.0));
                }
            }
            _ => {
                for succ in block.terminator.successors() {
                    if !block_set.contains(&succ) {
                        return Err(format!("Successor bb{} does not exist", succ.0));
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn validate_mir_program(program: &lower::MirProgram) -> Result<(), String> {
    for func in &program.functions {
        validate_mir_function(func)?;
    }
    Ok(())
}

pub mod alias;
pub mod defunctionalize;
pub mod dominance;
pub mod lower;
pub mod mem2reg;
pub mod memory_ssa;
pub mod speculate;
pub mod supercompiler;
pub mod thunk_analysis;

pub use defunctionalize::{defunctionalize_program, DefunctionalizeStats};
