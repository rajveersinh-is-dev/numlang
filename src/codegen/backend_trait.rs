//! Shared code generation trait for NumLang backend compilers.

use crate::mir::lower::MirProgram;
use crate::typecheck::typed_ast::TypedProgram;

/// Common interface implemented by all code generation backends in NumLang.
pub trait BackendCompiler {
    /// Return the human-readable identifier of the backend (e.g. "cranelift", "llvm").
    fn name(&self) -> &'static str;

    /// Compile a typed AST program directly to object file bytes.
    fn compile_to_obj_bytes(&mut self, program: &TypedProgram) -> Result<Vec<u8>, String>;

    /// Compile an SSA MIR program to object file bytes.
    fn compile_mir_to_obj_bytes(&mut self, mir: &MirProgram) -> Result<Vec<u8>, String>;
}
