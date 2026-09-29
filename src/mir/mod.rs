use crate::typecheck::types::Type;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BasicBlockId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Projection {
    Deref,
    Field(String),
    Index(Box<Place>),
    Payload(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Place {
    pub local: String,
    pub projections: Vec<Projection>,
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub arguments: Vec<(String, Type)>,
    pub terminator: Terminator,
}

pub mod alias;
pub mod dominance;
pub mod lower;
pub mod mem2reg;
pub mod memory_ssa;
pub mod supercompiler;
