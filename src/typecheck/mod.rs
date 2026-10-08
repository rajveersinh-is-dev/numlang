pub mod checker;
pub mod symtab;
pub mod typed_ast;
pub mod types;

pub use checker::{typecheck, TypeChecker, TypeError};
pub use symtab::{FunctionSig, ScopeEnvironment, Symbol};
pub use typed_ast::{
    TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedParam, TypedProgram, TypedStmt,
};
pub use types::Type;
