pub mod array_opt;
pub mod bce;
pub mod const_args;
pub mod inlining;
pub mod loop_opt;
pub mod recursion;
pub mod supercompiler;
pub mod while_unroll;

use crate::typecheck::typed_ast::TypedProgram;

pub fn optimize_program(program: &mut TypedProgram) {
    // Fixpoint iteration for canonicalization, constant propagation, and inlining
    const MAX_FIXPOINT_ITERS: usize = 4;
    for _ in 0..MAX_FIXPOINT_ITERS {
        let snapshot = program.clone();
        const_args::optimize_program(program);
        inlining::optimize_program(program);
        loop_opt::optimize_program(program);
        if *program == snapshot {
            break;
        }
    }

    // Supercompiler derives closed forms for pure functions and bounded loops
    supercompiler::supercompile_program(program, None);

    // Loop unrolling for remaining small or vectorizable loops
    while_unroll::optimize_program(program);

    // Array optimization (SROA / dynamic indexing)
    array_opt::optimize_arrays(program);

    // Bounds check elimination
    bce::optimize_program(program);

    // Recursion optimization (tail recursion / memoization)
    recursion::optimize_program(program);
}
