//! Parallel Multi-Threaded Supercompiler Engine.
//!
//! Concurrently drives and supercompiles independent MIR functions across CPU cores
//! using scoped threads, accelerating project-wide optimization throughput.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use super::drive::SupercompilerStats;
use super::{supercompile_mir_function_with_program, SupercompileMode};
use crate::mir::lower::{MirFunction, MirProgram};

/// Supercompiles an entire MIR program across multiple CPU threads concurrently.
pub fn supercompile_mir_program_parallel(
    program: &mut MirProgram,
    mode: SupercompileMode,
    objective: &str,
    num_threads: usize,
) -> SupercompilerStats {
    let threads_count = if num_threads == 0 {
        thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
    } else {
        num_threads
    };

    // If small number of functions, use standard path
    if program.functions.len() <= 1 || threads_count <= 1 {
        return super::supercompile_mir_program_with_mode(program, mode, objective);
    }

    let program_funcs_snapshot = Arc::new(program.functions.clone());
    let nodes_explored = Arc::new(AtomicUsize::new(0));
    let branches_pruned = Arc::new(AtomicUsize::new(0));
    let loops_collapsed = Arc::new(AtomicUsize::new(0));
    let knots_tied = Arc::new(AtomicUsize::new(0));

    // Parallel pass over functions using chunked threads
    let funcs = std::mem::take(&mut program.functions);
    let mut chunks: Vec<Vec<MirFunction>> = vec![Vec::new(); threads_count];
    for (i, func) in funcs.into_iter().enumerate() {
        chunks[i % threads_count].push(func);
    }

    let objective_str = objective.to_string();

    let processed_chunks: Vec<Vec<MirFunction>> = thread::scope(|s| {
        let mut handles = Vec::new();
        for chunk in chunks {
            let snapshot = Arc::clone(&program_funcs_snapshot);
            let n_exp = Arc::clone(&nodes_explored);
            let b_prun = Arc::clone(&branches_pruned);
            let l_coll = Arc::clone(&loops_collapsed);
            let k_tied = Arc::clone(&knots_tied);
            let obj = objective_str.clone();

            let handle = s.spawn(move || {
                let mut processed = Vec::new();
                for mut func in chunk {
                    if !super::func_is_impure(&func) {
                        let (new_func, stats) = match mode {
                            SupercompileMode::Classic => {
                                supercompile_mir_function_with_program(&func, &snapshot)
                            }
                            SupercompileMode::Distill => {
                                let driver = super::SupercompilerDriver::new(&func).with_program_functions(&snapshot);
                                let mut tree = driver.run();
                                let mut interner_clone = tree.interner.clone();
                                let mut distill = super::distill::DistillationEngine::new(&func, &mut interner_clone);
                                let folds = distill.distill_process_tree(&mut tree);
                                let mut s = tree.stats.clone();
                                s.loops_collapsed += folds;
                                (super::residualize::residualize_process_tree(&tree, &func), s)
                            }
                            SupercompileMode::Mrsc => {
                                let mrsc_engine = super::mrsc::MultiResultEngine::new(&func, &snapshot);
                                let (best_res, best_tree, _) = match obj.as_str() {
                                    "branch" => mrsc_engine.explore_and_select(&super::mrsc::MinDynamicBranchObjective),
                                    "pareto" => mrsc_engine.explore_and_select(&super::mrsc::ParetoObjective),
                                    _ => mrsc_engine.explore_and_select(&super::mrsc::MinCodeSizeObjective),
                                };
                                (best_res, best_tree.stats.clone())
                            }
                        };

                        let has_uncollapsed_array_loops = super::func_has_array_writes(&func) && stats.loops_collapsed == 0;
                        if !has_uncollapsed_array_loops && stats.knots_tied == 0 {
                            func = new_func;
                        }

                        n_exp.fetch_add(stats.nodes_explored, Ordering::Relaxed);
                        b_prun.fetch_add(stats.branches_pruned, Ordering::Relaxed);
                        l_coll.fetch_add(stats.loops_collapsed, Ordering::Relaxed);
                        k_tied.fetch_add(stats.knots_tied, Ordering::Relaxed);
                    }
                    processed.push(func);
                }
                processed
            });
            handles.push(handle);
        }

        let mut results = Vec::new();
        for h in handles {
            results.push(h.join().unwrap_or_default());
        }
        results
    });

    for chunk in processed_chunks {
        program.functions.extend(chunk);
    }

    SupercompilerStats {
        nodes_explored: nodes_explored.load(Ordering::Relaxed),
        branches_pruned: branches_pruned.load(Ordering::Relaxed),
        loops_collapsed: loops_collapsed.load(Ordering::Relaxed),
        knots_tied: knots_tied.load(Ordering::Relaxed),
    }
}
