//! Residualization for the NumLang supercompiler.
//!
//! Converts a concrete `Value` back into a `TypedExpr` / `TypedBlock`.
//! When a loop is fully evaluated, its body is replaced by a single
//! `return <concrete>` statement.

use crate::span::Span;
use crate::typecheck::typed_ast::{TypedBlock, TypedExpr, TypedLiteral, TypedStmt};
use crate::typecheck::types::Type;
use super::value::Value;

/// A dummy span used when constructing synthetic AST nodes.
pub const DUMMY_SPAN: Span = Span { start: 0, end: 0 };

/// Convert a concrete `Value` into a `TypedExpr`.
/// Panics if given a `Symbolic` value (caller must check).
pub fn value_to_expr(v: &Value) -> TypedExpr {
    match v {
        Value::Int(i) => TypedExpr::Literal {
            lit: TypedLiteral::Int(*i, Type::I64),
            ty: Type::I64,
            span: DUMMY_SPAN,
        },
        Value::Float(f) => TypedExpr::Literal {
            lit: TypedLiteral::Float(*f, Type::F64),
            ty: Type::F64,
            span: DUMMY_SPAN,
        },
        Value::Bool(b) => TypedExpr::Literal {
            lit: TypedLiteral::Bool(*b),
            ty: Type::Bool,
            span: DUMMY_SPAN,
        },
        Value::Void => TypedExpr::Literal {
            lit: TypedLiteral::Int(0, Type::I64),
            ty: Type::Void,
            span: DUMMY_SPAN,
        },
        Value::Symbolic(_) => {
            // Callers should not residualize symbolic values as literals.
            // Return a placeholder 0.
            TypedExpr::Literal {
                lit: TypedLiteral::Int(0, Type::I64),
                ty: Type::I64,
                span: DUMMY_SPAN,
            }
        }
        Value::Array(elems, elem_ty) => {
            TypedExpr::ArrayLiteral {
                elements: elems.iter().map(value_to_expr).collect(),
                ty: Type::Array(Box::new(elem_ty.clone()), elems.len()),
                span: DUMMY_SPAN,
            }
        }
    }
}

/// Produce a `TypedBlock` that simply returns the given concrete value.
pub fn residualize_return(v: &Value) -> TypedBlock {
    TypedBlock {
        stmts: vec![TypedStmt::Return(Some(value_to_expr(v)), DUMMY_SPAN)],
        span: DUMMY_SPAN,
    }
}
