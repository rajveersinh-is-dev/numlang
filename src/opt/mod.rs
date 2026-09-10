pub mod array_opt;
pub mod math_elevation;
pub mod recursion;

use crate::typecheck::typed_ast::TypedProgram;

pub fn optimize_program(program: &mut TypedProgram) {
    math_elevation::optimize_program(program);
    array_opt::optimize_arrays(program);
    recursion::optimize_program(program);
}
