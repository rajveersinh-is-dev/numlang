pub mod array_opt;
pub mod recursion;

use crate::typecheck::typed_ast::TypedProgram;

pub fn optimize_program(program: &mut TypedProgram) {
    array_opt::optimize_arrays(program);
    recursion::optimize_program(program);
}
