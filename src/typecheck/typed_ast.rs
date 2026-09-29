use crate::ast::{BinaryOp, UnaryOp};
use crate::span::Span;
use crate::typecheck::types::Type;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TypedLiteral {
    Int(i64, Type),
    Float(f64, Type),
    Bool(bool),
    Str(String),
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
    StructLiteral {
        name: String,
        fields: Vec<(String, TypedExpr)>,
        ty: Type,
        span: Span,
    },
    FieldAccess {
        target: Box<TypedExpr>,
        field: String,
        ty: Type,
        span: Span,
    },
    Match {
        scrutinee: Box<TypedExpr>,
        arms: Vec<TypedMatchArm>,
        ty: Type,
        span: Span,
    },
    EnumConstructor {
        enum_name: String,
        variant_name: String,
        tag: usize,
        args: Vec<TypedExpr>,
        ty: Type,
        span: Span,
    },
    Lambda {
        params: Vec<(String, Type)>,
        body: Box<TypedExpr>,
        captured: Vec<(String, Type)>,
        ty: Type,
        span: Span,
    },
    CallIndirect {
        callee: Box<TypedExpr>,
        args: Vec<TypedExpr>,
        ty: Type,
        span: Span,
    },
    Box {
        inner: Box<TypedExpr>,
        ty: Type,
        span: Span,
    },
    Deref {
        inner: Box<TypedExpr>,
        ty: Type,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedMatchPattern {
    Literal(TypedLiteral),
    Wildcard,
    Variant {
        enum_name: String,
        variant_name: String,
        tag: usize,
        bindings: Vec<(String, Type)>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedMatchArm {
    pub patterns: Vec<TypedMatchPattern>,
    pub body: TypedExpr,
    pub span: Span,
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
            TypedExpr::StructLiteral { ty, .. } => ty.clone(),
            TypedExpr::FieldAccess { ty, .. } => ty.clone(),
            TypedExpr::Match { ty, .. } => ty.clone(),
            TypedExpr::EnumConstructor { ty, .. } => ty.clone(),
            TypedExpr::Lambda { ty, .. } => ty.clone(),
            TypedExpr::CallIndirect { ty, .. } => ty.clone(),
            TypedExpr::Box { ty, .. } => ty.clone(),
            TypedExpr::Deref { ty, .. } => ty.clone(),
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
            TypedExpr::StructLiteral { span, .. } => *span,
            TypedExpr::FieldAccess { span, .. } => *span,
            TypedExpr::Match { span, .. } => *span,
            TypedExpr::EnumConstructor { span, .. } => *span,
            TypedExpr::Lambda { span, .. } => *span,
            TypedExpr::CallIndirect { span, .. } => *span,
            TypedExpr::Box { span, .. } => *span,
            TypedExpr::Deref { span, .. } => *span,
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
    FieldAssign {
        target: String,
        field: String,
        value: TypedExpr,
        span: Span,
    },
    Return(Option<TypedExpr>, Span),
    Break(Span),
    Continue(Span),
    For {
        var: String,
        lo: TypedExpr,
        hi: TypedExpr,
        inclusive: bool,
        body: TypedBlock,
        span: Span,
    },
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
            TypedStmt::FieldAssign { span, .. } => *span,
            TypedStmt::Return(_, span) => *span,
            TypedStmt::Break(span) => *span,
            TypedStmt::Continue(span) => *span,
            TypedStmt::For { span, .. } => *span,
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
    pub type_params: Vec<String>,
    pub params: Vec<TypedParam>,
    pub return_ty: Type,
    pub body: TypedBlock,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedStructDef {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedEnumDef {
    pub name: String,
    pub variants: Vec<TypedEnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedEnumVariant {
    pub name: String,
    pub tag: usize,
    pub payload: Vec<Type>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TypedProgram {
    pub functions: Vec<TypedFunction>,
    pub structs: Vec<TypedStructDef>,
    pub enums: Vec<TypedEnumDef>,
}

impl TypedProgram {
    pub fn desugar_for_loops(&mut self) {
        for func in &mut self.functions {
            func.body.desugar_for_loops();
        }
    }
}

impl TypedBlock {
    pub fn desugar_for_loops(&mut self) {
        let mut i = 0;
        while i < self.stmts.len() {
            match &mut self.stmts[i] {
                TypedStmt::If { then_branch, else_branch, .. } => {
                    then_branch.desugar_for_loops();
                    if let Some(eb) = else_branch {
                        eb.desugar_for_loops();
                    }
                    i += 1;
                }
                TypedStmt::While { body, .. } => {
                    body.desugar_for_loops();
                    i += 1;
                }
                TypedStmt::For { .. } => {
                    let stmt = self.stmts.remove(i);
                    if let TypedStmt::For { var, lo, hi, inclusive, mut body, span } = stmt {
                        body.desugar_for_loops();
                        let var_ty = lo.ty();

                        patch_continue_in_body(&mut body.stmts, &var, &var_ty, span);

                        body.stmts.push(TypedStmt::Assign {
                            name: var.clone(),
                            value: TypedExpr::Binary {
                                op: BinaryOp::Add,
                                left: Box::new(TypedExpr::Ident { name: var.clone(), ty: var_ty.clone(), span }),
                                right: Box::new(TypedExpr::Literal { lit: TypedLiteral::Int(1, var_ty.clone()), ty: var_ty.clone(), span }),
                                ty: var_ty.clone(),
                                span,
                            },
                            span,
                        });

                        let let_stmt = TypedStmt::Let {
                            name: var.clone(),
                            is_mutable: true,
                            ty: var_ty.clone(),
                            value: lo,
                            span,
                        };

                        let cond_op = if inclusive { BinaryOp::Le } else { BinaryOp::Lt };
                        let cond = TypedExpr::Binary {
                            op: cond_op,
                            left: Box::new(TypedExpr::Ident { name: var, ty: var_ty, span }),
                            right: Box::new(hi),
                            ty: Type::Bool,
                            span,
                        };

                        let while_stmt = TypedStmt::While {
                            condition: cond,
                            body,
                            span,
                        };

                        self.stmts.insert(i, while_stmt);
                        self.stmts.insert(i, let_stmt);
                        i += 2;
                    }
                }
                _ => {
                    i += 1;
                }
            }
        }
    }
}

fn patch_continue_in_body(stmts: &mut Vec<TypedStmt>, var: &str, var_ty: &Type, span: Span) {
    let mut i = 0;
    while i < stmts.len() {
        match &mut stmts[i] {
            TypedStmt::Continue(c_span) => {
                let inc_stmt = TypedStmt::Assign {
                    name: var.to_string(),
                    value: TypedExpr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(TypedExpr::Ident { name: var.to_string(), ty: var_ty.clone(), span }),
                        right: Box::new(TypedExpr::Literal { lit: TypedLiteral::Int(1, var_ty.clone()), ty: var_ty.clone(), span }),
                        ty: var_ty.clone(),
                        span,
                    },
                    span: *c_span,
                };
                stmts.insert(i, inc_stmt);
                i += 2;
            }
            TypedStmt::If { then_branch, else_branch, .. } => {
                patch_continue_in_body(&mut then_branch.stmts, var, var_ty, span);
                if let Some(eb) = else_branch {
                    patch_continue_in_body(&mut eb.stmts, var, var_ty, span);
                }
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
}
