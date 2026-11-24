//! General "does this program depend on the outside world?" analysis.
//!
//! A program is *closed* when every value it computes is derived from source
//! literals and the program's own control flow.  Such a program must be
//! compiled as executable computation: folding its whole computation into a
//! literal answer at compile time would make the emitted binary a lookup table
//! rather than a program that performs the calculation on execution.
//!
//! The analysis is deliberately structural and name-agnostic apart from the
//! runtime-input intrinsics themselves (`read_i64` and friends).  It never
//! looks at benchmark names, literal values, loop bounds or call counts.
//! When in doubt it answers "has runtime input" (the conservative answer that
//! keeps the existing optimizer behaviour).

use crate::mir::lower::{MirProgram, Rvalue, Statement};
use crate::typecheck::typed_ast::{TypedExpr, TypedProgram, TypedStmt};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

/// True only inside the `numlang` compiler driver process.  Library callers of
/// the codegen API keep the legacy optimisation behaviour.
static COMPILER_DRIVER: AtomicBool = AtomicBool::new(false);

pub fn mark_compiler_driver() {
    COMPILER_DRIVER.store(true, Ordering::Relaxed);
}

pub fn is_compiler_driver() -> bool {
    COMPILER_DRIVER.load(Ordering::Relaxed)
}

thread_local! {
    /// Set by `compiler::distill_and_optimize`, i.e. only on programs that go
    /// through the real compiler pipeline.  Direct library callers that lower
    /// a typed AST themselves keep the legacy optimiser behaviour.
    static AST_RUNTIME_COMPUTATION_GUARD: Cell<bool> = const { Cell::new(false) };
}

/// Enables the closed-program guard for the current compilation unit.
pub fn set_ast_runtime_computation_guard(enabled: bool) -> bool {
    AST_RUNTIME_COMPUTATION_GUARD.with(|c| {
        let previous = c.get();
        c.set(enabled);
        previous
    })
}

/// True when the typed AST currently being compiled must keep its computation
/// executable (no runtime input available to the supercompiler).
pub fn ast_runtime_computation_guard() -> bool {
    AST_RUNTIME_COMPUTATION_GUARD.with(|c| c.get())
}

/// True for calls that obtain a value from outside the compiled program.
///
/// NumLang currently exposes `read_i64()`; the additional spellings cover
/// obvious future/equivalent intrinsics and are matched by prefix so that a
/// new `read_*` input primitive is recognised without another code change.
pub fn is_runtime_input_call(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("read_")
        || lower.starts_with("input")
        || matches!(
            lower.as_str(),
            "getenv" | "argv" | "env" | "clock" | "now" | "random" | "rand"
        )
}

/// True when the typed program can observe a runtime-supplied value.
pub fn typed_program_has_runtime_input(program: &TypedProgram) -> bool {
    // A parameter on `main` would be supplied by the host, not by source code.
    if program
        .functions
        .iter()
        .any(|f| f.name == "main" && !f.params.is_empty())
    {
        return true;
    }
    program
        .functions
        .iter()
        .any(|f| scan_typed_block(&f.body))
}

fn scan_typed_block(block: &crate::typecheck::typed_ast::TypedBlock) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        TypedStmt::Let { value, .. }
        | TypedStmt::Assign { value, .. }
        | TypedStmt::Expr(value) => scan_typed_expr(value),
        TypedStmt::IndexAssign { index, value, .. } => {
            scan_typed_expr(index) || scan_typed_expr(value)
        }
        TypedStmt::FieldAssign { value, .. } => scan_typed_expr(value),
        TypedStmt::Return(Some(value), _) => scan_typed_expr(value),
        TypedStmt::Return(None, _)
        | TypedStmt::Break(_)
        | TypedStmt::Continue(_) => false,
        TypedStmt::For { lo, hi, body, .. } => {
            scan_typed_expr(lo) || scan_typed_expr(hi) || scan_typed_block(body)
        }
        TypedStmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            scan_typed_expr(condition)
                || scan_typed_block(then_branch)
                || else_branch.as_ref().is_some_and(scan_typed_block)
        }
        TypedStmt::While { condition, body, .. } => {
            scan_typed_expr(condition) || scan_typed_block(body)
        }
    })
}

fn scan_typed_expr(expr: &TypedExpr) -> bool {
    match expr {
        TypedExpr::Call { callee, args, .. } => {
            is_runtime_input_call(callee) || args.iter().any(scan_typed_expr)
        }
        TypedExpr::Unary { expr, .. } | TypedExpr::Box { inner: expr, .. } => scan_typed_expr(expr),
        TypedExpr::Deref { inner, .. } => scan_typed_expr(inner),
        TypedExpr::Binary { left, right, .. } => {
            scan_typed_expr(left) || scan_typed_expr(right)
        }
        TypedExpr::ArrayLiteral { elements, .. } => elements.iter().any(scan_typed_expr),
        TypedExpr::EnumConstructor { args, .. } => args.iter().any(scan_typed_expr),
        TypedExpr::Index { target, index, .. } => {
            scan_typed_expr(target) || scan_typed_expr(index)
        }
        TypedExpr::StructLiteral { fields, .. } => {
            fields.iter().any(|(_, e)| scan_typed_expr(e))
        }
        TypedExpr::FieldAccess { target, .. } => scan_typed_expr(target),
        TypedExpr::Match { scrutinee, arms, .. } => {
            scan_typed_expr(scrutinee) || arms.iter().any(|arm| scan_typed_expr(&arm.body))
        }
        TypedExpr::Lambda { body, .. } => scan_typed_expr(body),
        TypedExpr::CallIndirect { callee, args, .. } => {
            scan_typed_expr(callee) || args.iter().any(scan_typed_expr)
        }
        TypedExpr::Literal { .. } | TypedExpr::Ident { .. } => false,
    }
}

/// True when the lowered MIR program can observe a runtime-supplied value.
pub fn mir_program_has_runtime_input(program: &MirProgram) -> bool {
    program.functions.iter().any(|f| {
        let main_has_params = f.name == "main" && !f.params.is_empty();
        main_has_params
            || f.blocks.iter().any(|b| {
                b.statements.iter().any(
                    |Statement::Assign(_, rv)| matches!(rv, Rvalue::Call(name, _) if is_runtime_input_call(name)),
                ) || matches!(
                    b.terminator,
                    crate::mir::Terminator::IndirectCall { .. }
                )
            })
    })
}
