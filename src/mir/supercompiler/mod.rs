//! General SSA Process-Tree Supercompiler for NumLang MIR.
//!
//! Evaluates functions with symbolic parameters, propagates positive branch conditions,
//! detects growth via fast homeomorphic embedding, solves recurrence closed forms ($O(N) \to O(1)$),
//! and residualizes back into optimized SSA Control Flow Graphs.

pub mod drive;
pub mod generalize;
pub mod residualize;
pub mod state;
pub mod term;
pub mod whistle;

use crate::mir::lower::{MirFunction, MirProgram};
use drive::SupercompilerDriver;
use residualize::residualize_process_tree;

/// Supercompiles an entire MIR program across all functions.
pub fn supercompile_mir_program(program: &mut MirProgram) {
    for func in &mut program.functions {
        *func = supercompile_mir_function(func);
    }
}

/// Supercompiles a single MIR function using the SSA Process-Tree engine.
pub fn supercompile_mir_function(func: &MirFunction) -> MirFunction {
    let driver = SupercompilerDriver::new(func);
    let tree = driver.run();
    residualize_process_tree(&tree, func)
}
