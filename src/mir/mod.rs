use crate::typecheck::types::Type;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BasicBlockId(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub enum Projection {
    Deref,
    Field(String),
    Index(Box<Place>),
}

#[derive(Debug, Clone, PartialEq)]
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub arguments: Vec<(String, Type)>,
    pub terminator: Terminator,
}

pub mod dominance;
pub mod lower;
