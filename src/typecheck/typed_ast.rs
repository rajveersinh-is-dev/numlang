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
    ArrayLiteral {
        elements: Vec<TypedExpr>,
        ty: Type,
        span: Span,
    },
    Index {
        target: Box<TypedExpr>,
        index: Box<TypedExpr>,
        is_safe: bool,
        ty: Type,
        span: Span,
    },
}

impl TypedExpr {
    pub fn ty(&self) -> Type {
        match self {
            TypedExpr::Literal { ty, .. } => ty.clone(),
            TypedExpr::Ident { ty, .. } => ty.clone(),
            TypedExpr::Unary { ty, .. } => ty.clone(),
            TypedExpr::Binary { ty, .. } => ty.clone(),
            TypedExpr::Call { ty, .. } => ty.clone(),
            TypedExpr::ArrayLiteral { ty, .. } => ty.clone(),
            TypedExpr::Index { ty, .. } => ty.clone(),
        }
    }

    pub fn span(&self) -> Span {
        match self {
            TypedExpr::Literal { span, .. } => *span,
            TypedExpr::Ident { span, .. } => *span,
            TypedExpr::Unary { span, .. } => *span,
            TypedExpr::Binary { span, .. } => *span,
            TypedExpr::Call { span, .. } => *span,
            TypedExpr::ArrayLiteral { span, .. } => *span,
            TypedExpr::Index { span, .. } => *span,
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
    IndexAssign {
        target: String,
        index: TypedExpr,
        value: TypedExpr,
        is_safe: bool,
        span: Span,
    },
    Return(Option<TypedExpr>, Span),
    Break(Span),
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

impl TypedStmt {
    pub fn span(&self) -> Span {
        match self {
            TypedStmt::Let { span, .. } => *span,
            TypedStmt::Assign { span, .. } => *span,
            TypedStmt::IndexAssign { span, .. } => *span,
            TypedStmt::Return(_, span) => *span,
            TypedStmt::Break(span) => *span,
            TypedStmt::Expr(e) => e.span(),
            TypedStmt::If { span, .. } => *span,
            TypedStmt::While { span, .. } => *span,
        }
    }
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
