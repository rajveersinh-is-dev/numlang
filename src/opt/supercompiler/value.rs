//! Symbolic value representation for the NumLang supercompiler.
//!
//! A `Value` is either a fully concrete primitive, an array of values,
//! or a symbolic expression that could not be evaluated at compile time.

use crate::ast::{BinaryOp, UnaryOp};
use crate::typecheck::types::{wrap_int_by_type, Type};

/// A value produced during symbolic driving.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<Value>, Type), // element type
    Constructor {
        enum_name: String,
        variant_name: String,
        tag: usize,
        fields: Vec<Value>,
        ty: Type,
    },
    Closure {
        params: Vec<String>,
        body: crate::typecheck::typed_ast::TypedExpr,
        captured: std::collections::HashMap<String, Value>,
    },
    Symbolic(SymExpr),
    Void,
}

/// A symbolic expression — the residual of partial evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum SymExpr {
    Var(String, Type),
    BinOp(BinaryOp, Box<SymExpr>, Box<SymExpr>, Type),
    UnOp(UnaryOp, Box<SymExpr>, Type),
    Call(String, Vec<SymExpr>, Type),
    If(Box<SymExpr>, Box<SymExpr>, Box<SymExpr>, Type),
    Index(Box<SymExpr>, Box<SymExpr>, Type),
}

impl SymExpr {
    pub fn ty(&self) -> Type {
        match self {
            SymExpr::Var(_, t) => t.clone(),
            SymExpr::BinOp(_, _, _, t) => t.clone(),
            SymExpr::UnOp(_, _, t) => t.clone(),
            SymExpr::Call(_, _, t) => t.clone(),
            SymExpr::If(_, _, _, t) => t.clone(),
            SymExpr::Index(_, _, t) => t.clone(),
        }
    }

    pub fn contains_opaque(&self) -> bool {
        match self {
            SymExpr::Var(name, _) => {
                if name.starts_with("__lit_") || name.starts_with("__litf_") || name.starts_with("__litb_") {
                    false
                } else {
                    name.starts_with('_')
                }
            }
            SymExpr::BinOp(_, l, r, _) => l.contains_opaque() || r.contains_opaque(),
            SymExpr::UnOp(_, inner, _) => inner.contains_opaque(),
            SymExpr::Call(_, args, _) => args.iter().any(|a| a.contains_opaque()),
            SymExpr::If(c, th, el, _) => {
                c.contains_opaque() || th.contains_opaque() || el.contains_opaque()
            }
            SymExpr::Index(arr, idx, _) => arr.contains_opaque() || idx.contains_opaque(),
        }
    }
}

impl Value {
    pub fn is_concrete(&self) -> bool {
        match self {
            Value::Int(_) | Value::Float(_) | Value::Bool(_) => true,
            Value::Array(elems, _) => elems.iter().all(|e| e.is_concrete()),
            Value::Constructor { fields, .. } => fields.iter().all(|f| f.is_concrete()),
            Value::Closure { captured, .. } => captured.values().all(|v| v.is_concrete()),
            _ => false,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f64), // implicit widening
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Int(i) => Some(*i != 0),
            _ => None,
        }
    }

    pub fn is_symbolic_opaque(&self) -> bool {
        match self {
            Value::Symbolic(s) => s.contains_opaque(),
            Value::Array(elems, _) => elems.iter().any(|e| e.is_symbolic_opaque()),
            Value::Constructor { fields, .. } => fields.iter().any(|f| f.is_symbolic_opaque()),
            Value::Closure { captured, .. } => captured.values().any(|v| v.is_symbolic_opaque()),
            _ => false,
        }
    }
}

/// Fold a binary operation over two concrete values, or return Symbolic.
pub fn fold_binary(op: BinaryOp, left: Value, right: Value) -> Value {
    fold_binary_typed(op, left, right, None)
}

pub fn fold_binary_typed(op: BinaryOp, left: Value, right: Value, operand_ty: Option<&Type>) -> Value {
    // Integer × Integer
    if let (Some(l), Some(r)) = (left.as_int(), right.as_int()) {
        if matches!(left, Value::Int(_)) && matches!(right, Value::Int(_)) {
            if operand_ty.is_some_and(|t| t.is_unsigned()) {
                return fold_uint_binary(op, l as u64, r as u64, operand_ty);
            } else {
                return fold_int_binary(op, l, r, operand_ty);
            }
        }
    }

    // Float × Float (or int widened)
    if matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div |
                    BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le |
                    BinaryOp::Gt | BinaryOp::Ge) {
        let is_float_op = matches!(left, Value::Float(_)) || matches!(right, Value::Float(_));
        if is_float_op {
            if let (Some(l), Some(r)) = (left.as_float(), right.as_float()) {
                return fold_float_binary(op, l, r);
            }
        }
    }

    // Boolean comparison of concrete values
    if let (Value::Bool(l), Value::Bool(r)) = (&left, &right) {
        match op {
            BinaryOp::Eq => return Value::Bool(l == r),
            BinaryOp::Ne => return Value::Bool(l != r),
            _ => {}
        }
    }

    // Cannot fold — return symbolic
    let default_ty = operand_ty.cloned().unwrap_or(Type::I64);
    let lsym = value_to_sym(left, default_ty.clone());
    let rsym = value_to_sym(right, default_ty);
    let ty = sym_result_type(op, &lsym, &rsym);
    Value::Symbolic(SymExpr::BinOp(op, Box::new(lsym), Box::new(rsym), ty))
}

fn fold_uint_binary(op: BinaryOp, l: u64, r: u64, ty: Option<&Type>) -> Value {
    let wrap = |v: i64| Value::Int(wrap_int_by_type(v, ty));
    match op {
        BinaryOp::Add => wrap(l.wrapping_add(r) as i64),
        BinaryOp::Sub => wrap(l.wrapping_sub(r) as i64),
        BinaryOp::Mul => wrap(l.wrapping_mul(r) as i64),
        BinaryOp::Div => {
            wrap(l.checked_div(r).unwrap_or(0) as i64)
        }
        BinaryOp::Mod => {
            wrap(l.checked_rem(r).unwrap_or(0) as i64)
        }
        BinaryOp::Pow => {
            if r > 62 {
                let mut acc: u64 = 1;
                for _ in 0..r {
                    acc = acc.wrapping_mul(l);
                }
                wrap(acc as i64)
            } else {
                wrap(l.wrapping_pow(r as u32) as i64)
            }
        }
        BinaryOp::BitAnd => wrap((l & r) as i64),
        BinaryOp::BitOr  => wrap((l | r) as i64),
        BinaryOp::BitXor => wrap((l ^ r) as i64),
        BinaryOp::Shl => {
            if r >= 64 { wrap(0) }
            else { wrap((l << r) as i64) }
        }
        BinaryOp::Shr => {
            if r >= 64 { wrap(0) }
            else { wrap((l >> r) as i64) }
        }
        BinaryOp::Eq => Value::Bool(l == r),
        BinaryOp::Ne => Value::Bool(l != r),
        BinaryOp::Lt => Value::Bool(l < r),
        BinaryOp::Le => Value::Bool(l <= r),
        BinaryOp::Gt => Value::Bool(l > r),
        BinaryOp::Ge => Value::Bool(l >= r),
    }
}

fn fold_int_binary(op: BinaryOp, l: i64, r: i64, ty: Option<&Type>) -> Value {
    let wrap = |v: i64| Value::Int(wrap_int_by_type(v, ty));
    match op {
        BinaryOp::Add => wrap(l.wrapping_add(r)),
        BinaryOp::Sub => wrap(l.wrapping_sub(r)),
        BinaryOp::Mul => wrap(l.wrapping_mul(r)),
        BinaryOp::Div => {
            if r == 0 { wrap(0) } else { wrap(l.wrapping_div(r)) }
        }
        BinaryOp::Mod => {
            if r == 0 { wrap(0) } else { wrap(l.wrapping_rem(r)) }
        }
        BinaryOp::Pow => {
            // Integer exponentiation (fast path for small exponent)
            if r < 0 { return wrap(0); }
            if r > 62 {
                // May overflow — compute anyway
                let mut acc: i64 = 1;
                for _ in 0..r {
                    acc = acc.wrapping_mul(l);
                }
                return wrap(acc);
            }
            wrap(l.wrapping_pow(r as u32))
        }
        BinaryOp::BitAnd => wrap(l & r),
        BinaryOp::BitOr  => wrap(l | r),
        BinaryOp::BitXor => wrap(l ^ r),
        BinaryOp::Shl => {
            if !(0..64).contains(&r) { wrap(0) }
            else { wrap(l << r) }
        }
        BinaryOp::Shr => {
            if !(0..64).contains(&r) { wrap(0) }
            else { wrap(l >> r) }
        }
        BinaryOp::Eq => Value::Bool(l == r),
        BinaryOp::Ne => Value::Bool(l != r),
        BinaryOp::Lt => Value::Bool(l < r),
        BinaryOp::Le => Value::Bool(l <= r),
        BinaryOp::Gt => Value::Bool(l > r),
        BinaryOp::Ge => Value::Bool(l >= r),
    }
}

fn fold_float_binary(op: BinaryOp, l: f64, r: f64) -> Value {
    match op {
        BinaryOp::Add => Value::Float(l + r),
        BinaryOp::Sub => Value::Float(l - r),
        BinaryOp::Mul => Value::Float(l * r),
        BinaryOp::Div => Value::Float(if r == 0.0 { f64::INFINITY } else { l / r }),
        BinaryOp::Eq  => Value::Bool(l == r),
        BinaryOp::Ne  => Value::Bool(l != r),
        BinaryOp::Lt  => Value::Bool(l < r),
        BinaryOp::Le  => Value::Bool(l <= r),
        BinaryOp::Gt  => Value::Bool(l > r),
        BinaryOp::Ge  => Value::Bool(l >= r),
        BinaryOp::Mod => Value::Float(if r == 0.0 { 0.0 } else { l % r }),
        BinaryOp::Pow => Value::Float(l.powf(r)),
        _ => Value::Symbolic(SymExpr::BinOp(
            op,
            Box::new(SymExpr::Var("_fl".to_string(), Type::F64)),
            Box::new(SymExpr::Var("_fr".to_string(), Type::F64)),
            Type::F64,
        )),
    }
}

pub fn fold_unary(op: UnaryOp, val: Value) -> Value {
    fold_unary_typed(op, val, None)
}

pub fn fold_unary_typed(op: UnaryOp, val: Value, ty: Option<&Type>) -> Value {
    match op {
        UnaryOp::Neg => match val {
            Value::Int(i)   => Value::Int(wrap_int_by_type(i.wrapping_neg(), ty)),
            Value::Float(f) => Value::Float(-f),
            other => {
                let s = value_to_sym(other, Type::I64);
                let ty = s.ty();
                Value::Symbolic(SymExpr::UnOp(op, Box::new(s), ty))
            }
        },
        UnaryOp::Not => match val {
            Value::Bool(b) => Value::Bool(!b),
            Value::Int(i)  => Value::Int(wrap_int_by_type(!i, ty)),
            other => {
                let s = value_to_sym(other, Type::Bool);
                let ty = s.ty();
                Value::Symbolic(SymExpr::UnOp(op, Box::new(s), ty))
            }
        },
    }
}

pub fn value_to_sym(v: Value, fallback_ty: Type) -> SymExpr {
    match v {
        Value::Symbolic(s) => s,
        Value::Int(i) => SymExpr::Var(format!("__lit_{}", i), Type::I64),
        Value::Float(f) => SymExpr::Var(format!("__litf_{}", f.to_bits()), Type::F64),
        Value::Bool(b) => SymExpr::Var(format!("__litb_{}", b), Type::Bool),
        _ => SymExpr::Var("__sym".to_string(), fallback_ty),
    }
}

fn sym_result_type(op: BinaryOp, l: &SymExpr, _r: &SymExpr) -> Type {
    match op {
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le |
        BinaryOp::Gt | BinaryOp::Ge => Type::Bool,
        _ => l.ty(),
    }
}

/// Check deep equality of two values for fixed-point and period detection.
pub fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x.to_bits() == y.to_bits(),
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Void, Value::Void) => true,
        (Value::Array(xs, _), Value::Array(ys, _)) => {
            xs.len() == ys.len() && xs.iter().zip(ys.iter()).all(|(a, b)| values_equal(a, b))
        }
        (
            Value::Constructor {
                enum_name: e1,
                variant_name: v1,
                tag: t1,
                fields: f1,
                ..
            },
            Value::Constructor {
                enum_name: e2,
                variant_name: v2,
                tag: t2,
                fields: f2,
                ..
            },
        ) => {
            e1 == e2
                && v1 == v2
                && t1 == t2
                && f1.len() == f2.len()
                && f1.iter().zip(f2.iter()).all(|(a, b)| values_equal(a, b))
        }
        _ => false,
    }
}

use crate::typecheck::typed_ast::{TypedExpr, TypedLiteral};
use crate::span::Span;

pub const DUMMY_SPAN: Span = Span { start: 0, end: 0 };

/// Convert a SymExpr into a TypedExpr for residualization.
pub fn sym_to_expr(s: &SymExpr) -> TypedExpr {
    match s {
        SymExpr::Var(name, ty) => {
            if let Some(rest) = name.strip_prefix("__lit_") {
                if let Ok(val) = rest.parse::<i64>() {
                    return TypedExpr::Literal {
                        lit: TypedLiteral::Int(val, ty.clone()),
                        ty: ty.clone(),
                        span: DUMMY_SPAN,
                    };
                }
            } else if let Some(rest) = name.strip_prefix("__litf_") {
                if let Ok(bits) = rest.parse::<u64>() {
                    return TypedExpr::Literal {
                        lit: TypedLiteral::Float(f64::from_bits(bits), ty.clone()),
                        ty: ty.clone(),
                        span: DUMMY_SPAN,
                    };
                }
            } else if let Some(rest) = name.strip_prefix("__litb_") {
                if let Ok(b) = rest.parse::<bool>() {
                    return TypedExpr::Literal {
                        lit: TypedLiteral::Bool(b),
                        ty: ty.clone(),
                        span: DUMMY_SPAN,
                    };
                }
            }
            TypedExpr::Ident {
                name: name.clone(),
                ty: ty.clone(),
                span: DUMMY_SPAN,
            }
        }
        SymExpr::BinOp(op, l, r, ty) => TypedExpr::Binary {
            op: *op,
            left: Box::new(sym_to_expr(l)),
            right: Box::new(sym_to_expr(r)),
            ty: ty.clone(),
            span: DUMMY_SPAN,
        },
        SymExpr::UnOp(op, inner, ty) => TypedExpr::Unary {
            op: *op,
            expr: Box::new(sym_to_expr(inner)),
            ty: ty.clone(),
            span: DUMMY_SPAN,
        },
        SymExpr::Call(name, args, ty) => TypedExpr::Call {
            callee: name.clone(),
            args: args.iter().map(sym_to_expr).collect(),
            ty: ty.clone(),
            span: DUMMY_SPAN,
        },
        SymExpr::If(cond, then, else_, ty) => TypedExpr::Match {
            scrutinee: Box::new(sym_to_expr(cond)),
            arms: vec![
                crate::typecheck::typed_ast::TypedMatchArm {
                    patterns: vec![crate::typecheck::typed_ast::TypedMatchPattern::Literal(
                        TypedLiteral::Bool(true),
                    )],
                    body: sym_to_expr(then),
                    span: DUMMY_SPAN,
                },
                crate::typecheck::typed_ast::TypedMatchArm {
                    patterns: vec![crate::typecheck::typed_ast::TypedMatchPattern::Literal(
                        TypedLiteral::Bool(false),
                    )],
                    body: sym_to_expr(else_),
                    span: DUMMY_SPAN,
                },
            ],
            ty: ty.clone(),
            span: DUMMY_SPAN,
        },
        SymExpr::Index(arr, idx, ty) => TypedExpr::Index {
            target: Box::new(sym_to_expr(arr)),
            index: Box::new(sym_to_expr(idx)),
            is_safe: false,
            ty: ty.clone(),
            span: DUMMY_SPAN,
        },
    }
}

/// Convert any Value (concrete or symbolic) to a TypedExpr.
pub fn value_to_typed_expr(v: &Value) -> TypedExpr {
    match v {
        Value::Int(i) => TypedExpr::Literal {
            lit: TypedLiteral::Int(*i, crate::typecheck::types::Type::I64),
            ty: crate::typecheck::types::Type::I64,
            span: DUMMY_SPAN,
        },
        Value::Float(f) => TypedExpr::Literal {
            lit: TypedLiteral::Float(*f, crate::typecheck::types::Type::F64),
            ty: crate::typecheck::types::Type::F64,
            span: DUMMY_SPAN,
        },
        Value::Bool(b) => TypedExpr::Literal {
            lit: TypedLiteral::Bool(*b),
            ty: crate::typecheck::types::Type::Bool,
            span: DUMMY_SPAN,
        },
        Value::Symbolic(s) => sym_to_expr(s),
        Value::Void => TypedExpr::Literal {
            lit: TypedLiteral::Int(0, crate::typecheck::types::Type::I64),
            ty: crate::typecheck::types::Type::Void,
            span: DUMMY_SPAN,
        },
        Value::Array(elems, elem_ty) => TypedExpr::ArrayLiteral {
            elements: elems.iter().map(value_to_typed_expr).collect(),
            ty: crate::typecheck::types::Type::Array(Box::new(elem_ty.clone()), elems.len()),
            span: DUMMY_SPAN,
        },
        Value::Constructor { enum_name, variant_name, tag, fields, ty } => TypedExpr::EnumConstructor {
            enum_name: enum_name.clone(),
            variant_name: variant_name.clone(),
            tag: *tag,
            args: fields.iter().map(value_to_typed_expr).collect(),
            ty: ty.clone(),
            span: DUMMY_SPAN,
        },
        Value::Closure { body, .. } => body.clone(),
    }
}

