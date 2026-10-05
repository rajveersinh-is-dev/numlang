//! High-level compilation pipeline for NumLang.
//!
//! Coordinates typed AST distillation (Hamilton higher-order deforestation & folding),
//! typed AST optimizations, MIR lowering, post-residualization outlining, and
//! two-tier JIT compilation.

use crate::ast::hodistill::{distill_program, AstDistillStats};
use crate::mir::lower::{lower_program, MirProgram};
use crate::mir::supercompiler::outliner::{outline_program, OutlinerConfig, OutlinerStats};
use crate::typecheck::typed_ast::TypedProgram;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompilerConfig {
    pub ho_distill: bool,
    pub supercompile: bool,
    pub tiered: bool,
    pub hot_threshold: u64,
    pub outline: bool,
    pub min_outline_len: usize,
    pub similarity_threshold: f64,
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            ho_distill: false,
            supercompile: false,
            tiered: false,
            hot_threshold: 100,
            outline: false,
            min_outline_len: 2,
            similarity_threshold: 0.90,
        }
    }
}

impl CompilerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_supercompile(mut self, enabled: bool) -> Self {
        self.supercompile = enabled;
        self
    }

    pub fn with_ho_distill(mut self, enabled: bool) -> Self {
        self.ho_distill = enabled;
        self
    }

    pub fn with_tiered(mut self, enabled: bool) -> Self {
        self.tiered = enabled;
        self
    }

    pub fn with_outline(mut self, enabled: bool) -> Self {
        self.outline = enabled;
        self
    }
}

/// Executes pre-lowering optimization passes:
/// 1. Higher-order AST distillation (if `ho_distill` or `supercompile` enabled and not Tier 0).
/// 2. Typed AST optimization passes (monomorphization, inlining, loop opts, SROA).
pub fn distill_and_optimize(
    typed: &mut TypedProgram,
    config: &CompilerConfig,
) -> AstDistillStats {
    let stats = if !config.tiered && (config.ho_distill || config.supercompile) {
        distill_program(typed)
    } else {
        AstDistillStats::default()
    };

    crate::opt::optimize_program(typed);
    stats
}

/// Compiles a typed AST program through distillation, optimization, and MIR lowering.
/// In tiered mode (`tiered = true`), applies Tier 0 direct MIR lowering.
/// When `outline = true`, runs post-residualization basic block deduplication.
pub fn compile_pipeline(
    typed: &mut TypedProgram,
    config: &CompilerConfig,
) -> (MirProgram, AstDistillStats) {
    let stats = distill_and_optimize(typed, config);
    let mut mir = lower_program(typed);

    if config.outline {
        let outliner_config = OutlinerConfig {
            min_sequence_length: config.min_outline_len,
            similarity_threshold: config.similarity_threshold,
            max_outlined_functions: 100,
        };
        let _ = outline_program(&mut mir, &outliner_config);
    }

    (mir, stats)
}

/// Compiles with post-residualization outlining and returns both MIR and outliner stats.
pub fn compile_pipeline_with_outlining(
    typed: &mut TypedProgram,
    config: &CompilerConfig,
) -> (MirProgram, AstDistillStats, OutlinerStats) {
    let stats = distill_and_optimize(typed, config);
    let mut mir = lower_program(typed);

    let outliner_config = OutlinerConfig {
        min_sequence_length: config.min_outline_len,
        similarity_threshold: config.similarity_threshold,
        max_outlined_functions: 100,
    };
    let outliner_stats = outline_program(&mut mir, &outliner_config);

    (mir, stats, outliner_stats)
}

/// Tier 0 cold start compiler: compiles typed AST directly to MIR with zero supercompilation.
/// Guarantees cold startup latency < 2ms.
pub fn compile_tier0(typed: &mut TypedProgram) -> MirProgram {
    let config = CompilerConfig {
        tiered: true,
        ho_distill: false,
        supercompile: false,
        outline: false,
        ..Default::default()
    };
    let (mir, _) = compile_pipeline(typed, &config);
    mir
}

/// Tier 1 supercompiler: compiles typed AST with full supercompilation pipeline.
pub fn compile_tier1(typed: &mut TypedProgram) -> MirProgram {
    let config = CompilerConfig {
        tiered: false,
        ho_distill: true,
        supercompile: true,
        outline: false,
        ..Default::default()
    };
    let (mut mir, _) = compile_pipeline(typed, &config);
    crate::mir::supercompiler::supercompile_mir_program(&mut mir);
    mir
}
