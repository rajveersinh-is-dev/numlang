pub mod array_opt;
pub mod bce;
pub mod const_args;
pub mod inlining;
pub mod loop_opt;
pub mod monomorphize;
pub mod recursion;
pub mod while_unroll;
pub mod zero_rem;

use crate::typecheck::typed_ast::TypedProgram;

pub fn optimize_program(program: &mut TypedProgram) {
    // A closed program (one with no runtime input) must still calculate its
    // answer on execution.  Literal-argument propagation and bounded unrolling
    // are the two AST passes that can evaluate such a program at compile time:
    // they turn source literals into a fully concrete computation which later
    // passes collapse into a single answer literal.  Programs that do read
    // runtime input keep both passes exactly as before.
    let guard_active = crate::runtime_input::ast_runtime_computation_guard()
        && !crate::runtime_input::typed_program_has_runtime_input(program);

    // Monomorphize generic functions into concrete instantiations
    monomorphize::monomorphize(program);

    // Desugar for loops into canonical while loops before optimization
    program.desugar_for_loops();

    // Rewrite zero-tests of power-of-two remainders into bitmask tests.  This
    // is an always-valid algebraic identity (divisibility == low bits clear),
    // it never evaluates user code at compile time, and it applies to every
    // program regardless of whether it reads runtime input.
    zero_rem::optimize_program(program);

    // Fixpoint iteration for canonicalization, constant propagation, and inlining
    const MAX_FIXPOINT_ITERS: usize = 4;
    for _ in 0..MAX_FIXPOINT_ITERS {
        let snapshot = program.clone();
        if !guard_active {
            const_args::optimize_program(program);
            // Inlining a call with literal arguments has the same effect as
            // constant-argument propagation: it embeds a fully concrete
            // computation into the caller, which later passes evaluate.
            inlining::optimize_program(program);
        }
        loop_opt::optimize_program(program);
        if *program == snapshot {
            break;
        }
    }

    // Loop unrolling for remaining small or vectorizable loops
    if !guard_active {
        while_unroll::optimize_program(program);
    }

    // Array optimization (SROA / dynamic indexing)
    array_opt::optimize_arrays(program);

    // Bounds check elimination
    bce::optimize_program(program);

    // Recursion optimization (tail recursion / memoization)
    recursion::optimize_program(program);
}
