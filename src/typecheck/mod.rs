pub mod types;
pub mod symtab;
pub mod typed_ast;
pub mod checker;

pub use types::Type;
pub use symtab::{FunctionSig, ScopeEnvironment, Symbol};
pub use typed_ast::{TypedBlock, TypedExpr, TypedFunction, TypedLiteral, TypedParam, TypedProgram, TypedStmt};
pub use checker::{TypeChecker, TypeError, typecheck};
