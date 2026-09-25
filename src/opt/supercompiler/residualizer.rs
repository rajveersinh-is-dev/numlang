//! Residualization for the NumLang supercompiler.
//!
//! Converts a concrete `Value` back into a `TypedExpr` / `TypedBlock`.
//! When a loop is fully evaluated, its body is replaced by a single
//! `return <concrete>` statement.

use crate::span::Span;
use crate::typecheck::typed_ast::{TypedBlock, TypedExpr, TypedStmt};
use super::value::Value;

/// A dummy span used when constructing synthetic AST nodes.
pub const DUMMY_SPAN: Span = Span { start: 0, end: 0 };

/// Convert any `Value` into a `TypedExpr`.
pub fn value_to_expr(v: &Value) -> TypedExpr {
    super::value::value_to_typed_expr(v)
}

/// Produce a `TypedBlock` that simply returns the given concrete value.
pub fn residualize_return(v: &Value) -> TypedBlock {
    TypedBlock {
        stmts: vec![TypedStmt::Return(Some(value_to_expr(v)), DUMMY_SPAN)],
        span: DUMMY_SPAN,
    }
}
