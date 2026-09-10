pub mod lower;

use crate::typecheck::Type;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub usize);

impl fmt::Display for ValueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Value(ValueId),
    IntConst(i64, Type),
    FloatConst(f64, Type),
    BoolConst(bool),
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operand::Value(v) => write!(f, "{}", v),
            Operand::IntConst(n, ty) => write!(f, "{}:{}", n, ty),
            Operand::FloatConst(fl, ty) => write!(f, "{}:{}", fl, ty),
            Operand::BoolConst(b) => write!(f, "{}", b),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Neg,
    Not,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl fmt::Display for IrOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrOp::Add => write!(f, "add"),
            IrOp::Sub => write!(f, "sub"),
            IrOp::Mul => write!(f, "mul"),
            IrOp::Div => write!(f, "div"),
            IrOp::Mod => write!(f, "mod"),
            IrOp::Pow => write!(f, "pow"),
            IrOp::Neg => write!(f, "neg"),
            IrOp::Not => write!(f, "not"),
            IrOp::Eq => write!(f, "eq"),
            IrOp::Ne => write!(f, "ne"),
            IrOp::Lt => write!(f, "lt"),
            IrOp::Le => write!(f, "le"),
            IrOp::Gt => write!(f, "gt"),
            IrOp::Ge => write!(f, "ge"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    Binary {
        dest: ValueId,
        op: IrOp,
        ty: Type,
        left: Operand,
        right: Operand,
    },
    Unary {
        dest: ValueId,
        op: IrOp,
        ty: Type,
        operand: Operand,
    },
    Call {
        dest: Option<ValueId>,
        callee: String,
        args: Vec<Operand>,
        return_ty: Type,
    },
    Assign {
        dest: ValueId,
        operand: Operand,
        ty: Type,
    },
    Branch {
        target: BlockId,
    },
    BranchIf {
        cond: Operand,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return {
        val: Option<Operand>,
    },
    IndexLoad {
        dest: ValueId,
        target: Operand,
        index: Operand,
        ty: Type,
    },
    IndexStore {
        target: String,
        index: Operand,
        value: Operand,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct BasicBlock {
    pub id: BlockId,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrParam {
    pub name: String,
    pub val: ValueId,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<IrParam>,
    pub return_ty: Type,
    pub blocks: Vec<BasicBlock>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct IrProgram {
    pub functions: Vec<IrFunction>,
}

pub fn format_ir(program: &IrProgram) -> String {
    let mut out = String::new();
    for func in &program.functions {
        out.push_str(&format!("function {}(", func.name));
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!("{}: {} ({})", param.name, param.ty, param.val));
        }
        out.push_str(&format!(") -> {} {{\n", func.return_ty));

        for block in &func.blocks {
            out.push_str(&format!("  {}:\n", block.id));
            for inst in &block.instructions {
                out.push_str("    ");
                match inst {
                    Instruction::Binary {
                        dest,
                        op,
                        ty,
                        left,
                        right,
                    } => {
                        out.push_str(&format!("{} = {}.{} {}, {}\n", dest, ty, op, left, right));
                    }
                    Instruction::Unary {
                        dest,
                        op,
                        ty,
                        operand,
                    } => {
                        out.push_str(&format!("{} = {}.{} {}\n", dest, ty, op, operand));
                    }
                    Instruction::Call {
                        dest,
                        callee,
                        args,
                        return_ty,
                    } => {
                        if let Some(d) = dest {
                            out.push_str(&format!("{} = ", d));
                        }
                        out.push_str(&format!("call {}(", callee));
                        for (j, arg) in args.iter().enumerate() {
                            if j > 0 {
                                out.push_str(", ");
                            }
                            out.push_str(&format!("{}", arg));
                        }
                        out.push_str(&format!(") : {}\n", return_ty));
                    }
                    Instruction::Assign { dest, operand, ty } => {
                        out.push_str(&format!("{} = assign:{}, {}\n", dest, ty, operand));
                    }
                    Instruction::Branch { target } => {
                        out.push_str(&format!("br {}\n", target));
                    }
                    Instruction::BranchIf {
                        cond,
                        then_block,
                        else_block,
                    } => {
                        out.push_str(&format!(
                            "brif {}, {}, {}\n",
                            cond, then_block, else_block
                        ));
                    }
                    Instruction::Return { val } => match val {
                        Some(op) => out.push_str(&format!("ret {}\n", op)),
                        None => out.push_str("ret\n"),
                    },
                    Instruction::IndexLoad {
                        dest,
                        target,
                        index,
                        ty,
                    } => {
                        out.push_str(&format!("{} = {}.index {}, {}\n", dest, ty, target, index));
                    }
                    Instruction::IndexStore {
                        target,
                        index,
                        value,
                    } => {
                        out.push_str(&format!("index_store {}[{}], {}\n", target, index, value));
                    }
                }
            }
        }
        out.push_str("}\n\n");
    }
    out
}
