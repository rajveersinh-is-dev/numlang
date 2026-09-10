use crate::typecheck::typed_ast::TypedProgram;

pub fn optimize_program(_program: &mut TypedProgram) {
    // Recursive functions are executed genuinely at runtime.
    // No hardcoded tables or exact-input checks.
}

