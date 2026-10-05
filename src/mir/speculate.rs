//! Speculative Type Guards & Type Profile Analysis for MIR.
//!
//! Provides runtime profiling of observed dynamic type tags at polymorphic sites,
//! computes statistical confidence scores, and inserts speculative `Terminator::TypeGuard`
//! fast-path branches in residualized MIR when confidence exceeds configurable thresholds.

use std::collections::HashMap;
use crate::mir::{BasicBlockId, Terminator};
use crate::mir::lower::{MirBasicBlock, MirFunction, MirProgram};

/// Concrete observed type profile for a local variable or parameter at a specialization site.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TypeProfile {
    /// Identifier of the local or parameter.
    pub local: String,
    /// Most frequently observed concrete type tag (e.g. 1 for i64, 2 for f64, 3 for ptr).
    pub observed_tag: i64,
    /// Confidence fraction in the range [0.0, 1.0] (observed_count / total_count).
    pub confidence: f64,
    /// Total number of recorded execution samples.
    pub sample_count: usize,
}

/// Dynamic type profiler tracking observations across execution runs.
#[derive(Debug, Default, Clone)]
pub struct TypeProfiler {
    observations: HashMap<String, HashMap<i64, usize>>,
}

impl TypeProfiler {
    pub fn new() -> Self {
        Self {
            observations: HashMap::new(),
        }
    }

    /// Record a single concrete type tag observation for a given local identifier.
    pub fn record_observation(&mut self, local: &str, tag: i64) {
        *self
            .observations
            .entry(local.to_string())
            .or_default()
            .entry(tag)
            .or_insert(0) += 1;
    }

    /// Compute concrete type profiles for all observed locals.
    pub fn compute_profiles(&self) -> HashMap<String, TypeProfile> {
        let mut profiles = HashMap::new();
        for (local, tag_counts) in &self.observations {
            let total: usize = tag_counts.values().sum();
            if total == 0 {
                continue;
            }
            if let Some((&dominant_tag, &count)) = tag_counts.iter().max_by_key(|(_, &c)| c) {
                let confidence = count as f64 / total as f64;
                profiles.insert(
                    local.clone(),
                    TypeProfile {
                        local: local.clone(),
                        observed_tag: dominant_tag,
                        confidence,
                        sample_count: total,
                    },
                );
            }
        }
        profiles
    }

    /// Clear all recorded observations.
    pub fn clear(&mut self) {
        self.observations.clear();
    }
}

/// Analyze a function and insert speculative type guards at polymorphic/indirect call sites
/// where the observed profile confidence meets or exceeds `confidence_threshold` (default: 0.95).
///
/// Returns the number of speculative type guards successfully inserted into the function.
pub fn insert_speculative_type_guards(
    func: &mut MirFunction,
    profiles: &HashMap<String, TypeProfile>,
    confidence_threshold: f64,
) -> usize {
    if func.blocks.is_empty() {
        return 0;
    }

    let mut guards_inserted = 0;
    let mut new_blocks: Vec<MirBasicBlock> = Vec::new();

    // Iterate through blocks and look for candidates to guard
    for block in &func.blocks {
        let mut split_occurred = false;

        if let Terminator::IndirectCall { callee, args, dest, next } = &block.terminator {
            // Check if any argument or the callee place has a high-confidence type profile
            let candidate_local = if let Some(prof) = profiles.get(&callee.local) {
                if prof.confidence >= confidence_threshold {
                    Some((callee.clone(), prof.observed_tag))
                } else {
                    None
                }
            } else {
                args.iter().find_map(|arg| {
                    profiles.get(&arg.local).and_then(|prof| {
                        if prof.confidence >= confidence_threshold {
                            Some((arg.clone(), prof.observed_tag))
                        } else {
                            None
                        }
                    })
                })
            };

            if let Some((guard_place, expected_tag)) = candidate_local {
                split_occurred = true;
                guards_inserted += 1;

                let fast_id = BasicBlockId(func.blocks.len() + new_blocks.len() + 1);
                let deopt_id = BasicBlockId(func.blocks.len() + new_blocks.len() + 2);

                // Original block now ends with TypeGuard
                let guard_block = MirBasicBlock {
                    id: block.id.clone(),
                    statements: block.statements.clone(),
                    terminator: Terminator::TypeGuard {
                        local: guard_place,
                        expected_tag,
                        fast_path: fast_id.clone(),
                        deopt_stub: deopt_id.clone(),
                    },
                    arguments: vec![],
                };
                new_blocks.push(guard_block);

                // Fast-path block: optimized monomorphic execution continuing to `next`
                let fast_block = MirBasicBlock {
                    id: fast_id,
                    statements: vec![],
                    terminator: Terminator::IndirectCall {
                        callee: callee.clone(),
                        args: args.clone(),
                        dest: dest.clone(),
                        next: next.clone(),
                    },
                    arguments: vec![],
                };
                new_blocks.push(fast_block);

                // Deopt stub block: unspecialized fallback path
                let deopt_block = MirBasicBlock {
                    id: deopt_id,
                    statements: vec![],
                    terminator: Terminator::IndirectCall {
                        callee: callee.clone(),
                        args: args.clone(),
                        dest: dest.clone(),
                        next: next.clone(),
                    },
                    arguments: vec![],
                };
                new_blocks.push(deopt_block);
            }
        }

        if !split_occurred {
            new_blocks.push(block.clone());
        }
    }

    if guards_inserted > 0 {
        func.blocks = new_blocks;
    }

    guards_inserted
}

/// Optimize an entire MirProgram with speculative type guards using profiler statistics.
pub fn speculate_program(
    program: &mut MirProgram,
    profiler: &TypeProfiler,
    confidence_threshold: f64,
) -> usize {
    let profiles = profiler.compute_profiles();
    let mut total_guards = 0;
    for func in &mut program.functions {
        total_guards += insert_speculative_type_guards(func, &profiles, confidence_threshold);
    }
    total_guards
}

/// Extract type profile statistics for a function's parameters and locals.
pub fn collect_function_type_profiles(
    func: &MirFunction,
    profiler: &TypeProfiler,
) -> HashMap<String, TypeProfile> {
    let all_profiles = profiler.compute_profiles();
    let mut func_profiles = HashMap::new();
    for (p_name, _) in &func.params {
        if let Some(prof) = all_profiles.get(p_name) {
            func_profiles.insert(p_name.clone(), prof.clone());
        }
    }
    for local in &func.locals {
        if let Some(prof) = all_profiles.get(&local.name) {
            func_profiles.insert(local.name.clone(), prof.clone());
        }
    }
    func_profiles
}
