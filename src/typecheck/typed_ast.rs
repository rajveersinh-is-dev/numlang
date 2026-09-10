use crate::ast::{BinaryOp, UnaryOp};
use crate::span::Span;
use crate::typecheck::types::Type;

#[derive(Debug, Clone, PartialEq)]
pub enum TypedLiteral {
    Int(i64, Type),
    Float(f64, Type),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedExpr {
    Literal {
        lit: TypedLiteral,
        ty: Type,
        span: Span,
    },
    Ident {
        name: String,
        ty: Type,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<TypedExpr>,
        ty: Type,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        left: Box<TypedExpr>,
        right: Box<TypedExpr>,
        ty: Type,
        span: Span,
    },
    Call {
        callee: String,
        args: Vec<TypedExpr>,
        ty: Type,
        span: Span,
    },
}

impl TypedExpr {
    pub fn ty(&self) -> Type {
        match self {
            TypedExpr::Literal { ty, .. } => *ty,
            TypedExpr::Ident { ty, .. } => *ty,
            TypedExpr::Unary { ty, .. } => *ty,
            TypedExpr::Binary { ty, .. } => *ty,
            TypedExpr::Call { ty, .. } => *ty,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            TypedExpr::Literal { span, .. } => *span,
            TypedExpr::Ident { span, .. } => *span,
            TypedExpr::Unary { span, .. } => *span,
            TypedExpr::Binary { span, .. } => *span,
            TypedExpr::Call { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedStmt {
    Let {
        name: String,
        is_mutable: bool,
        ty: Type,
        value: TypedExpr,
        span: Span,
    },
    Assign {
        name: String,
        value: TypedExpr,
        span: Span,
    },
    Return(Option<TypedExpr>, Span),
    Expr(TypedExpr),
    If {
        condition: TypedExpr,
        then_branch: TypedBlock,
        else_branch: Option<TypedBlock>,
        span: Span,
    },
    While {
        condition: TypedExpr,
        body: TypedBlock,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedBlock {
    pub stmts: Vec<TypedStmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedParam {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedFunction {
    pub name: String,
    pub params: Vec<TypedParam>,
    pub return_ty: Type,
    pub body: TypedBlock,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TypedProgram {
    pub functions: Vec<TypedFunction>,
}
