use crate::typecheck::typed_ast::TypedProgram;

pub fn optimize_program(_program: &mut TypedProgram) {
    // All stored lookup tables and precomputed answers have been removed.
    // All programs and benchmarks compute each value at runtime.
}

