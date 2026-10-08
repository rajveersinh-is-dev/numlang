//! Parallel Multi-Threaded Supercompiler Engine.
//!
//! Concurrently drives and supercompiles independent MIR functions across CPU cores
//! using scoped threads and dynamic work-stealing, accelerating project-wide optimization throughput.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use super::drive::SupercompilerStats;
use super::{supercompile_mir_function_with_program, SupercompileMode};
use crate::mir::lower::{MirFunction, MirProgram};

/// Dynamically drives supercompilation across functions using a work-stealing queue.
pub fn supercompile_mir_functions_work_stealing(
    functions: Vec<MirFunction>,
    program_snapshot: Arc<Vec<MirFunction>>,
    mode: SupercompileMode,
    objective: &str,
    num_threads: usize,
) -> (Vec<MirFunction>, SupercompilerStats) {
    let threads_count = if num_threads == 0 {
        thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    } else {
        num_threads
    };

    let total_funcs = functions.len();
    if total_funcs == 0 {
        return (Vec::new(), SupercompilerStats::default());
    }

    let nodes_explored = Arc::new(AtomicUsize::new(0));
    let branches_pruned = Arc::new(AtomicUsize::new(0));
    let loops_collapsed = Arc::new(AtomicUsize::new(0));
    let knots_tied = Arc::new(AtomicUsize::new(0));

    let work_items: VecDeque<(usize, MirFunction)> = functions.into_iter().enumerate().collect();
    let work_queue = Arc::new(Mutex::new(work_items));
    let output_slots: Arc<Mutex<Vec<Option<MirFunction>>>> =
        Arc::new(Mutex::new((0..total_funcs).map(|_| None).collect()));

    let objective_str = objective.to_string();

    thread::scope(|s| {
        let mut handles = Vec::new();
        for _ in 0..threads_count {
            let snapshot = Arc::clone(&program_snapshot);
            let queue = Arc::clone(&work_queue);
            let out_slots = Arc::clone(&output_slots);
            let n_exp = Arc::clone(&nodes_explored);
            let b_prun = Arc::clone(&branches_pruned);
            let l_coll = Arc::clone(&loops_collapsed);
            let k_tied = Arc::clone(&knots_tied);
            let obj = objective_str.clone();

            let handle = s.spawn(move || loop {
                let next_item = {
                    match queue.lock() {
                        Ok(mut q) => q.pop_front(),
                        Err(poisoned) => poisoned.into_inner().pop_front(),
                    }
                };

                let Some((idx, mut func)) = next_item else {
                    break;
                };

                if !super::func_is_impure(&func) {
                    let (new_func, stats) = match mode {
                        SupercompileMode::Classic => {
                            supercompile_mir_function_with_program(&func, &snapshot)
                        }
                        SupercompileMode::Distill => {
                            let driver = super::SupercompilerDriver::new(&func)
                                .with_program_functions(&snapshot);
                            let mut tree = driver.run();
                            let mut interner_clone = tree.interner.clone();
                            let mut distill =
                                super::distill::DistillationEngine::new(&func, &mut interner_clone);
                            let folds = distill.distill_process_tree(&mut tree);
                            let mut s = tree.stats.clone();
                            s.loops_collapsed += folds;
                            (
                                super::residualize::residualize_process_tree(&tree, &func),
                                s,
                            )
                        }
                        SupercompileMode::Mrsc => {
                            let mrsc_engine = super::mrsc::MultiResultEngine::new(&func, &snapshot);
                            let (best_res, best_tree, _) = match obj.as_str() {
                                "branch" => mrsc_engine
                                    .explore_and_select(&super::mrsc::MinDynamicBranchObjective),
                                "pareto" => {
                                    mrsc_engine.explore_and_select(&super::mrsc::ParetoObjective)
                                }
                                _ => mrsc_engine
                                    .explore_and_select(&super::mrsc::MinCodeSizeObjective),
                            };
                            (best_res, best_tree.stats.clone())
                        }
                        SupercompileMode::MrscExhaustive => {
                            let mrsc_obj = match obj.as_str() {
                                "speed" => super::mrsc::MrscObjective::Speed,
                                "size" => super::mrsc::MrscObjective::Size,
                                _ => super::mrsc::MrscObjective::Balanced,
                            };
                            let config = super::mrsc_oracle::IddfsOracleConfig {
                                min_depth: 2,
                                max_depth: 24,
                                step_depth: 2,
                                objective: mrsc_obj,
                                exhaustive: true,
                            };
                            let oracle =
                                super::mrsc_oracle::MrscOracleEngine::new(&func, &snapshot, config);
                            let (_frontier, winner) = oracle.explore_iddfs();
                            (winner.residual, winner.tree.stats.clone())
                        }
                    };

                    let has_uncollapsed_array_loops =
                        super::func_has_array_writes(&func) && stats.loops_collapsed == 0;
                    if !has_uncollapsed_array_loops && stats.knots_tied == 0 {
                        func = new_func;
                    }

                    n_exp.fetch_add(stats.nodes_explored, Ordering::Relaxed);
                    b_prun.fetch_add(stats.branches_pruned, Ordering::Relaxed);
                    l_coll.fetch_add(stats.loops_collapsed, Ordering::Relaxed);
                    k_tied.fetch_add(stats.knots_tied, Ordering::Relaxed);
                }

                match out_slots.lock() {
                    Ok(mut slots) => slots[idx] = Some(func),
                    Err(poisoned) => poisoned.into_inner()[idx] = Some(func),
                }
            });
            handles.push(handle);
        }

        for h in handles {
            let _ = h.join();
        }
    });

    let raw_results = match output_slots.lock() {
        Ok(mut s) => std::mem::take(&mut *s),
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    };

    let processed: Vec<MirFunction> = raw_results.into_iter().flatten().collect();

    let stats = SupercompilerStats {
        nodes_explored: nodes_explored.load(Ordering::Relaxed),
        branches_pruned: branches_pruned.load(Ordering::Relaxed),
        loops_collapsed: loops_collapsed.load(Ordering::Relaxed),
        knots_tied: knots_tied.load(Ordering::Relaxed),
        calls_inlined: 0,
        sc_bce_eliminated: 0,
        residual_block_count: 0,
        residual_stmt_count: 0,
    };

    (processed, stats)
}

/// Supercompiles an entire MIR program across multiple CPU threads concurrently using dynamic work-stealing.
pub fn supercompile_mir_program_parallel(
    program: &mut MirProgram,
    mode: SupercompileMode,
    objective: &str,
    num_threads: usize,
) -> SupercompilerStats {
    let threads_count = if num_threads == 0 {
        thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    } else {
        num_threads
    };

    // If small number of functions, use standard path
    if program.functions.len() <= 1 || threads_count <= 1 {
        return super::supercompile_mir_program_with_mode(program, mode, objective);
    }

    let program_funcs_snapshot = Arc::new(program.functions.clone());
    let funcs = std::mem::take(&mut program.functions);

    let (processed, stats) = supercompile_mir_functions_work_stealing(
        funcs,
        program_funcs_snapshot,
        mode,
        objective,
        threads_count,
    );

    program.functions = processed;
    stats
}
