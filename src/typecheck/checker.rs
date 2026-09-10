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

    #[error("Cannot index non-array type '{found}'")]
    CannotIndexNonArray {
        found: Type,
        span: Span,
    },

    #[error("Array index must be an integer, found '{found}'")]
    InvalidIndexType {
        found: Type,
        span: Span,
    },

    #[error("Array literal cannot be empty")]
    EmptyArrayLiteral {
        span: Span,
    },

    #[error("Array index {index} out of bounds for array of length {len}")]
    IndexOutOfBounds {
        index: i64,
        len: usize,
        span: Span,
    },

    #[error("Array element type mismatch: expected {expected}, found {found}")]
    ArrayElementMismatch {
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
            TypeError::CannotIndexNonArray { span, .. } => *span,
            TypeError::InvalidIndexType { span, .. } => *span,
            TypeError::EmptyArrayLiteral { span, .. } => *span,
            TypeError::IndexOutOfBounds { span, .. } => *span,
            TypeError::ArrayElementMismatch { span, .. } => *span,
        }
    }
}

use std::collections::HashMap;

pub struct TypeChecker {
    env: ScopeEnvironment,
    current_fn_return_ty: Type,
    active_loop_bounds: HashMap<String, i64>,
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
            active_loop_bounds: HashMap::new(),
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

        self.current_fn_return_ty = sig.return_ty.clone();
        self.env.enter_scope();

        let mut typed_params = Vec::new();
        for (i, param) in func.params.iter().enumerate() {
            let pty = sig.param_types[i].clone();
            let sym = Symbol {
                name: param.name.clone(),
                ty: pty.clone(),
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

                let typed_value = self.check_expr(value, declared_ty.clone())?;
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
                    ty: final_ty.clone(),
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

                let typed_value = self.check_expr(value, Some(sym.ty.clone()))?;
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

            Stmt::IndexAssign {
                target,
                index,
                value,
                span,
            } => {
                let sym = match self.env.lookup_variable(target) {
                    Some(s) => s.clone(),
                    None => {
                        return Err(TypeError::UndeclaredVariable {
                            name: target.clone(),
                            span: *span,
                        });
                    }
                };

                if !sym.is_mutable {
                    return Err(TypeError::CannotMutateImmutable {
                        name: target.clone(),
                        span: *span,
                    });
                }

                let (elem_ty, len) = match &sym.ty {
                    Type::Array(elem, len) => ((**elem).clone(), *len),
                    _ => {
                        return Err(TypeError::CannotIndexNonArray {
                            found: sym.ty.clone(),
                            span: *span,
                        });
                    }
                };

                let typed_index = self.check_expr(index, Some(Type::I64))?;
                if !typed_index.ty().is_integer() {
                    return Err(TypeError::InvalidIndexType {
                        found: typed_index.ty(),
                        span: typed_index.span(),
                    });
                }

                let mut is_safe = false;
                if let TypedExpr::Literal {
                    lit: TypedLiteral::Int(n, _),
                    span: idx_span,
                    ..
                } = &typed_index
                {
                    if *n < 0 || (*n as usize) >= len {
                        return Err(TypeError::IndexOutOfBounds {
                            index: *n,
                            len,
                            span: *idx_span,
                        });
                    }
                    is_safe = true;
                } else if let TypedExpr::Ident { name: idx_var, .. } = &typed_index {
                    if let Some(&bound) = self.active_loop_bounds.get(idx_var) {
                        if bound <= len as i64 {
                            is_safe = true;
                        }
                    }
                }

                let typed_value = self.check_expr(value, Some(elem_ty.clone()))?;
                if typed_value.ty() != elem_ty {
                    return Err(TypeError::TypeMismatch {
                        expected: elem_ty,
                        found: typed_value.ty(),
                        span: typed_value.span(),
                    });
                }

                Ok(TypedStmt::IndexAssign {
                    target: target.clone(),
                    index: typed_index,
                    value: typed_value,
                    is_safe,
                    span: *span,
                })
            }

            Stmt::Return(opt_expr, span) => match opt_expr {
                Some(expr) => {
                    let typed_expr =
                        self.check_expr(expr, Some(self.current_fn_return_ty.clone()))?;
                    if typed_expr.ty() != self.current_fn_return_ty {
                        return Err(TypeError::InvalidReturn {
                            expected: self.current_fn_return_ty.clone(),
                            found: typed_expr.ty(),
                            span: *span,
                        });
                    }
                    Ok(TypedStmt::Return(Some(typed_expr), *span))
                }
                None => {
                    if self.current_fn_return_ty != Type::Void {
                        return Err(TypeError::InvalidReturn {
                            expected: self.current_fn_return_ty.clone(),
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

                let loop_bound = match condition {
                    Expr::Binary { op: BinaryOp::Lt, left, right, .. } => {
                        if let (Expr::Ident(name, _), Expr::Literal(Literal::Int(n), _)) = (&**left, &**right) {
                            Some((name.clone(), *n))
                        } else {
                            None
                        }
                    }
                    Expr::Binary { op: BinaryOp::Le, left, right, .. } => {
                        if let (Expr::Ident(name, _), Expr::Literal(Literal::Int(n), _)) = (&**left, &**right) {
                            Some((name.clone(), *n + 1))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                if let Some((ref var_name, bound)) = loop_bound {
                    self.active_loop_bounds.insert(var_name.clone(), bound);
                }

                self.env.enter_scope();
                let mut body_stmts = Vec::new();
                for s in &body.stmts {
                    body_stmts.push(self.check_stmt(s)?);
                }
                self.env.exit_scope();

                if let Some((ref var_name, _)) = loop_bound {
                    self.active_loop_bounds.remove(var_name);
                }

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
                        lit: TypedLiteral::Int(*n, ty.clone()),
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
                        lit: TypedLiteral::Float(*f, ty.clone()),
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
                    ty: sym.ty.clone(),
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

            Expr::ArrayLiteral { elements, span } => {
                if elements.is_empty() {
                    return Err(TypeError::EmptyArrayLiteral { span: *span });
                }
                let expected_elem_hint = match &expected_hint {
                    Some(Type::Array(elem, _)) => Some((**elem).clone()),
                    _ => None,
                };
                let first_typed = self.check_expr(&elements[0], expected_elem_hint)?;
                let elem_ty = first_typed.ty();
                let mut typed_elements = vec![first_typed];
                for el in &elements[1..] {
                    let typed_el = self.check_expr(el, Some(elem_ty.clone()))?;
                    if typed_el.ty() != elem_ty {
                        return Err(TypeError::ArrayElementMismatch {
                            expected: elem_ty.clone(),
                            found: typed_el.ty(),
                            span: typed_el.span(),
                        });
                    }
                    typed_elements.push(typed_el);
                }
                let len = typed_elements.len();
                let arr_ty = Type::Array(Box::new(elem_ty), len);
                Ok(TypedExpr::ArrayLiteral {
                    elements: typed_elements,
                    ty: arr_ty,
                    span: *span,
                })
            }

            Expr::Index { target, index, span } => {
                let typed_target = self.check_expr(target, None)?;
                let target_ty = typed_target.ty();
                let (elem_ty, len) = match &target_ty {
                    Type::Array(elem, len) => ((**elem).clone(), *len),
                    _ => {
                        return Err(TypeError::CannotIndexNonArray {
                            found: target_ty,
                            span: target.span(),
                        });
                    }
                };

                let typed_index = self.check_expr(index, Some(Type::I64))?;
                if !typed_index.ty().is_integer() {
                    return Err(TypeError::InvalidIndexType {
                        found: typed_index.ty(),
                        span: typed_index.span(),
                    });
                }

                let mut is_safe = false;
                if let TypedExpr::Literal {
                    lit: TypedLiteral::Int(n, _),
                    span: idx_span,
                    ..
                } = &typed_index
                {
                    if *n < 0 || (*n as usize) >= len {
                        return Err(TypeError::IndexOutOfBounds {
                            index: *n,
                            len,
                            span: *idx_span,
                        });
                    }
                    is_safe = true;
                } else if let TypedExpr::Ident { name: idx_var, .. } = &typed_index {
                    if let Some(&bound) = self.active_loop_bounds.get(idx_var) {
                        if bound <= len as i64 {
                            is_safe = true;
                        }
                    }
                }

                Ok(TypedExpr::Index {
                    target: Box::new(typed_target),
                    index: Box::new(typed_index),
                    is_safe,
                    ty: elem_ty,
                    span: *span,
                })
            }

            Expr::Call { callee, args, span } => {
                // Built-in intrinsics
                match callee.as_str() {
                    "sqrt" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "sqrt".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], Some(Type::F64))?;
                        if !typed_arg.ty().is_float() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        let ty = typed_arg.ty();
                        return Ok(TypedExpr::Call {
                            callee: "sqrt".to_string(),
                            args: vec![typed_arg],
                            ty,
                            span: *span,
                        });
                    }
                    "abs" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "abs".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], expected_hint)?;
                        if !typed_arg.ty().is_numeric() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        let ty = typed_arg.ty();
                        return Ok(TypedExpr::Call {
                            callee: "abs".to_string(),
                            args: vec![typed_arg],
                            ty,
                            span: *span,
                        });
                    }
                    "to_i64" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "to_i64".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let typed_arg = self.check_expr(&args[0], None)?;
                        if !typed_arg.ty().is_numeric() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: typed_arg.ty(),
                                span: typed_arg.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "to_i64".to_string(),
                            args: vec![typed_arg],
                            ty: Type::I64,
                            span: *span,
                        });
                    }
                    "dot" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "dot".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        let (elem_ty_a, len_a) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => {
                                return Err(TypeError::CannotIndexNonArray {
                                    found: a.ty(),
                                    span: a.span(),
                                });
                            }
                        };
                        let (elem_ty_b, len_b) = match b.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => {
                                return Err(TypeError::CannotIndexNonArray {
                                    found: b.ty(),
                                    span: b.span(),
                                });
                            }
                        };
                        if elem_ty_a != elem_ty_b || len_a != len_b {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty_a.clone()), len_a),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        if !elem_ty_a.is_numeric() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: elem_ty_a,
                                span: a.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "dot".to_string(),
                            args: vec![a, b],
                            ty: elem_ty_a,
                            span: *span,
                        });
                    }
                    "vec_add" => {
                        if args.len() != 2 {
                            return Err(TypeError::ArityMismatch {
                                name: "vec_add".to_string(),
                                expected: 2,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let b = self.check_expr(&args[1], Some(a.ty()))?;
                        let (elem_ty_a, len_a) = match a.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => {
                                return Err(TypeError::CannotIndexNonArray {
                                    found: a.ty(),
                                    span: a.span(),
                                });
                            }
                        };
                        let (elem_ty_b, len_b) = match b.ty() {
                            Type::Array(elem, len) => (*elem, len),
                            _ => {
                                return Err(TypeError::CannotIndexNonArray {
                                    found: b.ty(),
                                    span: b.span(),
                                });
                            }
                        };
                        if elem_ty_a != elem_ty_b || len_a != len_b {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::Array(Box::new(elem_ty_a.clone()), len_a),
                                found: b.ty(),
                                span: b.span(),
                            });
                        }
                        let res_ty = Type::Array(Box::new(elem_ty_a), len_a);
                        return Ok(TypedExpr::Call {
                            callee: "vec_add".to_string(),
                            args: vec![a, b],
                            ty: res_ty,
                            span: *span,
                        });
                    }
                    "sum" => {
                        if args.len() != 1 {
                            return Err(TypeError::ArityMismatch {
                                name: "sum".to_string(),
                                expected: 1,
                                found: args.len(),
                                span: *span,
                            });
                        }
                        let a = self.check_expr(&args[0], None)?;
                        let elem_ty = match a.ty() {
                            Type::Array(elem, _) => *elem,
                            _ => {
                                return Err(TypeError::CannotIndexNonArray {
                                    found: a.ty(),
                                    span: a.span(),
                                });
                            }
                        };
                        if !elem_ty.is_numeric() {
                            return Err(TypeError::TypeMismatch {
                                expected: Type::F64,
                                found: elem_ty,
                                span: a.span(),
                            });
                        }
                        return Ok(TypedExpr::Call {
                            callee: "sum".to_string(),
                            args: vec![a],
                            ty: elem_ty,
                            span: *span,
                        });
                    }
                    _ => {}
                }

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
                for (arg, param_ty) in args.iter().zip(&sig.param_types) {
                    let typed_arg = self.check_expr(arg, Some(param_ty.clone()))?;
                    if typed_arg.ty() != *param_ty {
                        return Err(TypeError::TypeMismatch {
                            expected: param_ty.clone(),
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
