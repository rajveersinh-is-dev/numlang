pub mod array_opt;
pub mod const_args;
pub mod math_elevation;
pub mod recursion;

use crate::typecheck::typed_ast::TypedProgram;

pub fn optimize_program(program: &mut TypedProgram) {
    const_args::optimize_program(program);
    // Literal locals can expose new literal call arguments; run the small,
    // conservative pass once more to specialize those callees as well.
    const_args::optimize_program(program);
    math_elevation::optimize_program(program);
    array_opt::optimize_arrays(program);
    recursion::optimize_program(program);
}
