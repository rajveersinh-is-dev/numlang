use crate::ast::{BinaryOp, Expr, Function, Literal, Program, Stmt, UnaryOp};
use crate::span::Span;
use crate::typecheck::symtab::{FunctionSig, ScopeEnvironment, Symbol};
use crate::typecheck::typed_ast::{
    TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedParam, TypedProgram, TypedStmt,
};
use crate::typecheck::types::Type;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum TypeError {
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch {
        expected: Type,
        found: Type,
        span: Span,
    },

    #[error("Undeclared variable '{name}'")]
    UndeclaredVariable {
        name: String,
        span: Span,
    },

    #[error("Undeclared function '{name}'")]
    UndeclaredFunction {
        name: String,
        span: Span,
    },

    #[error("Cannot mutate immutable variable '{name}'")]
    CannotMutateImmutable {
        name: String,
        span: Span,
    },

    #[error("Identifier '{name}' is already declared in this scope")]
    DuplicateDeclaration {
        name: String,
        span: Span,
    },

    #[error("Condition must evaluate to bool, found {found}")]
    InvalidConditionType {
        found: Type,
        span: Span,
    },

    #[error("Invalid binary operands for '{op:?}': left is {left}, right is {right}")]
    InvalidBinaryOperands {
        op: BinaryOp,
        left: Type,
        right: Type,
        span: Span,
    },

    #[error("Invalid unary operand for '{op:?}': found {found}")]
    InvalidUnaryOperand {
        op: UnaryOp,
        found: Type,
        span: Span,
    },

    #[error("Function '{name}' expected {expected} arguments, but received {found}")]
    ArityMismatch {
        name: String,
        expected: usize,
        found: usize,
        span: Span,
    },

    #[error("Unknown type '{name}'")]
    UnknownType {
        name: String,
        span: Span,
    },

    #[error("Function return type mismatch: expected {expected}, found {found}")]
    InvalidReturn {
        expected: Type,
        found: Type,
        span: Span,
    },
}

impl TypeError {
    pub fn span(&self) -> Span {
        match self {
            TypeError::TypeMismatch { span, .. } => *span,
            TypeError::UndeclaredVariable { span, .. } => *span,
            TypeError::UndeclaredFunction { span, .. } => *span,
            TypeError::CannotMutateImmutable { span, .. } => *span,
            TypeError::DuplicateDeclaration { span, .. } => *span,
            TypeError::InvalidConditionType { span, .. } => *span,
            TypeError::InvalidBinaryOperands { span, .. } => *span,
            TypeError::InvalidUnaryOperand { span, .. } => *span,
            TypeError::ArityMismatch { span, .. } => *span,
            TypeError::UnknownType { span, .. } => *span,
            TypeError::InvalidReturn { span, .. } => *span,
        }
    }
}

pub struct TypeChecker {
    env: ScopeEnvironment,
    current_fn_return_ty: Type,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: ScopeEnvironment::new(),
            current_fn_return_ty: Type::Void,
        }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<TypedProgram, TypeError> {
        // Pass 1: Collect and validate all function signatures
        for func in &program.functions {
            let return_ty = match &func.return_ty {
                Some(s) => Type::from_name(s).ok_or_else(|| TypeError::UnknownType {
                    name: s.clone(),
                    span: func.span,
                })?,
                None => Type::Void,
            };

            let mut param_names = Vec::new();
            let mut param_types = Vec::new();
            for param in &func.params {
                let pty = Type::from_name(&param.ty).ok_or_else(|| TypeError::UnknownType {
                    name: param.ty.clone(),
                    span: param.span,
                })?;
                param_names.push(param.name.clone());
                param_types.push(pty);
            }

            let sig = FunctionSig {
                name: func.name.clone(),
                param_names,
                param_types,
                return_ty,
                span: func.span,
            };

            if self.env.define_function(sig).is_err() {
                return Err(TypeError::DuplicateDeclaration {
                    name: func.name.clone(),
                    span: func.span,
                });
            }
        }

        // Pass 2: Type check each function body
        let mut typed_functions = Vec::new();
        for func in &program.functions {
            typed_functions.push(self.check_function(func)?);
        }

        Ok(TypedProgram {
            functions: typed_functions,
        })
    }

    fn check_function(&mut self, func: &Function) -> Result<TypedFunction, TypeError> {
        let sig = self
            .env
            .lookup_function(&func.name)
            .cloned()
            .expect("Function must exist in scope");

        self.current_fn_return_ty = sig.return_ty;
        self.env.enter_scope();

        let mut typed_params = Vec::new();
        for (i, param) in func.params.iter().enumerate() {
            let pty = sig.param_types[i];
            let sym = Symbol {
                name: param.name.clone(),
                ty: pty,
                is_mutable: false,
                span: param.span,
            };
            if self.env.define_variable(sym).is_err() {
                return Err(TypeError::DuplicateDeclaration {
                    name: param.name.clone(),
                    span: param.span,
                });
            }
            typed_params.push(TypedParam {
                name: param.name.clone(),
                ty: pty,
                span: param.span,
            });
        }

        let mut typed_stmts = Vec::new();
        for stmt in &func.body.stmts {
            typed_stmts.push(self.check_stmt(stmt)?);
        }

        self.env.exit_scope();

        Ok(TypedFunction {
            name: func.name.clone(),
            params: typed_params,
            return_ty: sig.return_ty,
            body: TypedBlock {
                stmts: typed_stmts,
                span: func.body.span,
            },
            span: func.span,
        })
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<TypedStmt, TypeError> {
        match stmt {
            Stmt::Let {
                name,
                is_mutable,
                ty,
                value,
                span,
            } => {
                let declared_ty = if let Some(t_str) = ty {
                    let parsed = Type::from_name(t_str).ok_or_else(|| TypeError::UnknownType {
                        name: t_str.clone(),
                        span: *span,
                    })?;
                    Some(parsed)
                } else {
                    None
                };

                let typed_value = self.check_expr(value, declared_ty)?;
                let final_ty = match declared_ty {
                    Some(dt) => {
                        if typed_value.ty() != dt {
                            return Err(TypeError::TypeMismatch {
                                expected: dt,
                                found: typed_value.ty(),
                                span: typed_value.span(),
                            });
                        }
                        dt
                    }
                    None => {
                        let inf = typed_value.ty();
                        if inf == Type::Void {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::I64,
                                found: Type::Void,
                                span: typed_value.span(),
                            });
                        }
                        inf
                    }
                };

                let sym = Symbol {
                    name: name.clone(),
                    ty: final_ty,
                    is_mutable: *is_mutable,
                    span: *span,
                };

                if self.env.define_variable(sym).is_err() {
                    return Err(TypeError::DuplicateDeclaration {
                        name: name.clone(),
                        span: *span,
                    });
                }

                Ok(TypedStmt::Let {
                    name: name.clone(),
                    is_mutable: *is_mutable,
                    ty: final_ty,
                    value: typed_value,
                    span: *span,
                })
            }

            Stmt::Assign { name, value, span } => {
                let sym = match self.env.lookup_variable(name) {
                    Some(s) => s.clone(),
                    None => {
                        return Err(TypeError::UndeclaredVariable {
                            name: name.clone(),
                            span: *span,
                        });
                    }
                };

                if !sym.is_mutable {
                    return Err(TypeError::CannotMutateImmutable {
                        name: name.clone(),
                        span: *span,
                    });
                }

                let typed_value = self.check_expr(value, Some(sym.ty))?;
                if typed_value.ty() != sym.ty {
                    return Err(TypeError::TypeMismatch {
                        expected: sym.ty,
                        found: typed_value.ty(),
                        span: typed_value.span(),
                    });
                }

                Ok(TypedStmt::Assign {
                    name: name.clone(),
                    value: typed_value,
                    span: *span,
                })
            }

            Stmt::Return(opt_expr, span) => match opt_expr {
                Some(expr) => {
                    let typed_expr = self.check_expr(expr, Some(self.current_fn_return_ty))?;
                    if typed_expr.ty() != self.current_fn_return_ty {
                        return Err(TypeError::InvalidReturn {
                            expected: self.current_fn_return_ty,
                            found: typed_expr.ty(),
                            span: *span,
                        });
                    }
                    Ok(TypedStmt::Return(Some(typed_expr), *span))
                }
                None => {
                    if self.current_fn_return_ty != Type::Void {
                        return Err(TypeError::InvalidReturn {
                            expected: self.current_fn_return_ty,
                            found: Type::Void,
                            span: *span,
                        });
                    }
                    Ok(TypedStmt::Return(None, *span))
                }
            },

            Stmt::Expr(expr) => {
                let typed_expr = self.check_expr(expr, None)?;
                Ok(TypedStmt::Expr(typed_expr))
            }

            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let typed_cond = self.check_expr(condition, Some(Type::Bool))?;
                if typed_cond.ty() != Type::Bool {
                    return Err(TypeError::InvalidConditionType {
                        found: typed_cond.ty(),
                        span: typed_cond.span(),
                    });
                }

                self.env.enter_scope();
                let mut then_stmts = Vec::new();
                for s in &then_branch.stmts {
                    then_stmts.push(self.check_stmt(s)?);
                }
                self.env.exit_scope();

                let typed_else = if let Some(eb) = else_branch {
                    self.env.enter_scope();
                    let mut else_stmts = Vec::new();
                    for s in &eb.stmts {
                        else_stmts.push(self.check_stmt(s)?);
                    }
                    self.env.exit_scope();
                    Some(TypedBlock {
                        stmts: else_stmts,
                        span: eb.span,
                    })
                } else {
                    None
                };

                Ok(TypedStmt::If {
                    condition: typed_cond,
                    then_branch: TypedBlock {
                        stmts: then_stmts,
                        span: then_branch.span,
                    },
                    else_branch: typed_else,
                    span: *span,
                })
            }

            Stmt::While {
                condition,
                body,
                span,
            } => {
                let typed_cond = self.check_expr(condition, Some(Type::Bool))?;
                if typed_cond.ty() != Type::Bool {
                    return Err(TypeError::InvalidConditionType {
                        found: typed_cond.ty(),
                        span: typed_cond.span(),
                    });
                }

                self.env.enter_scope();
                let mut body_stmts = Vec::new();
                for s in &body.stmts {
                    body_stmts.push(self.check_stmt(s)?);
                }
                self.env.exit_scope();

                Ok(TypedStmt::While {
                    condition: typed_cond,
                    body: TypedBlock {
                        stmts: body_stmts,
                        span: body.span,
                    },
                    span: *span,
                })
            }
        }
    }

    pub fn check_expr(
        &mut self,
        expr: &Expr,
        expected_hint: Option<Type>,
    ) -> Result<TypedExpr, TypeError> {
        match expr {
            Expr::Literal(lit, span) => match lit {
                Literal::Int(n) => {
                    let ty = match expected_hint {
                        Some(Type::I32) => Type::I32,
                        _ => Type::I64,
                    };
                    Ok(TypedExpr::Literal {
                        lit: TypedLiteral::Int(*n, ty),
                        ty,
                        span: *span,
                    })
                }
                Literal::Float(f) => {
                    let ty = match expected_hint {
                        Some(Type::F32) => Type::F32,
                        _ => Type::F64,
                    };
                    Ok(TypedExpr::Literal {
                        lit: TypedLiteral::Float(*f, ty),
                        ty,
                        span: *span,
                    })
                }
                Literal::Bool(b) => Ok(TypedExpr::Literal {
                    lit: TypedLiteral::Bool(*b),
                    ty: Type::Bool,
                    span: *span,
                }),
            },

            Expr::Ident(name, span) => {
                let sym = self
                    .env
                    .lookup_variable(name)
                    .ok_or_else(|| TypeError::UndeclaredVariable {
                        name: name.clone(),
                        span: *span,
                    })?;
                Ok(TypedExpr::Ident {
                    name: name.clone(),
                    ty: sym.ty,
                    span: *span,
                })
            }

            Expr::Group(inner, _) => self.check_expr(inner, expected_hint),

            Expr::Unary { op, expr, span } => {
                let typed_inner = self.check_expr(expr, expected_hint)?;
                match op {
                    UnaryOp::Not => {
                        if typed_inner.ty() != Type::Bool {
                            return Err(TypeError::InvalidUnaryOperand {
                                op: *op,
                                found: typed_inner.ty(),
                                span: *span,
                            });
                        }
                        Ok(TypedExpr::Unary {
                            op: *op,
                            expr: Box::new(typed_inner),
                            ty: Type::Bool,
                            span: *span,
                        })
                    }
                    UnaryOp::Neg => {
                        if !typed_inner.ty().is_numeric() {
                            return Err(TypeError::InvalidUnaryOperand {
                                op: *op,
                                found: typed_inner.ty(),
                                span: *span,
                            });
                        }
                        let ty = typed_inner.ty();
                        Ok(TypedExpr::Unary {
                            op: *op,
                            expr: Box::new(typed_inner),
                            ty,
                            span: *span,
                        })
                    }
                }
            }

            Expr::Binary {
                op,
                left,
                right,
                span,
            } => {
                let typed_left = self.check_expr(left, expected_hint)?;
                let typed_right = self.check_expr(right, Some(typed_left.ty()))?;

                let lty = typed_left.ty();
                let rty = typed_right.ty();

                if lty != rty {
                    return Err(TypeError::InvalidBinaryOperands {
                        op: *op,
                        left: lty,
                        right: rty,
                        span: *span,
                    });
                }

                match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod
                    | BinaryOp::Pow => {
                        if !lty.is_numeric() {
                            return Err(TypeError::InvalidBinaryOperands {
                                op: *op,
                                left: lty,
                                right: rty,
                                span: *span,
                            });
                        }
                        Ok(TypedExpr::Binary {
                            op: *op,
                            left: Box::new(typed_left),
                            right: Box::new(typed_right),
                            ty: lty,
                            span: *span,
                        })
                    }
                    BinaryOp::Eq | BinaryOp::Ne => Ok(TypedExpr::Binary {
                        op: *op,
                        left: Box::new(typed_left),
                        right: Box::new(typed_right),
                        ty: Type::Bool,
                        span: *span,
                    }),
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                        if !lty.is_numeric() {
                            return Err(TypeError::InvalidBinaryOperands {
                                op: *op,
                                left: lty,
                                right: rty,
                                span: *span,
                            });
                        }
                        Ok(TypedExpr::Binary {
                            op: *op,
                            left: Box::new(typed_left),
                            right: Box::new(typed_right),
                            ty: Type::Bool,
                            span: *span,
                        })
                    }
                }
            }

            Expr::Call { callee, args, span } => {
                let sig = self
                    .env
                    .lookup_function(callee)
                    .cloned()
                    .ok_or_else(|| TypeError::UndeclaredFunction {
                        name: callee.clone(),
                        span: *span,
                    })?;

                if args.len() != sig.param_types.len() {
                    return Err(TypeError::ArityMismatch {
                        name: callee.clone(),
                        expected: sig.param_types.len(),
                        found: args.len(),
                        span: *span,
                    });
                }

                let mut typed_args = Vec::new();
                for (arg, &param_ty) in args.iter().zip(&sig.param_types) {
                    let typed_arg = self.check_expr(arg, Some(param_ty))?;
                    if typed_arg.ty() != param_ty {
                        return Err(TypeError::TypeMismatch {
                            expected: param_ty,
                            found: typed_arg.ty(),
                            span: typed_arg.span(),
                        });
                    }
                    typed_args.push(typed_arg);
                }

                Ok(TypedExpr::Call {
                    callee: callee.clone(),
                    args: typed_args,
                    ty: sig.return_ty,
                    span: *span,
                })
            }
        }
    }
}

pub fn typecheck(program: &Program) -> Result<TypedProgram, TypeError> {
    let mut checker = TypeChecker::new();
    checker.check_program(program)
}
