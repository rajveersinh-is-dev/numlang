use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, Result};
use numlang::diagnostic::{format_ast, format_tokens, format_typed_ast, CompilerDiagnostic};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::typed_ast::TypedProgram;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(
    name = "numlang",
    version = "0.1.0",
    about = "numlang mathematical systems programming language compiler"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(
        short = 't',
        long = "emit-tokens",
        help = "Emit tokenized output with source span coordinates"
    )]
    pub emit_tokens: bool,

    #[arg(
        short = 'a',
        long = "emit-ast",
        help = "Emit parsed Abstract Syntax Tree representation"
    )]
    pub emit_ast: bool,

    #[arg(
        short = 'c',
        long = "check",
        help = "Type check the source program and report semantic errors"
    )]
    pub check: bool,

    #[arg(
        long = "emit-typed-ast",
        help = "Emit type-checked Abstract Syntax Tree representation"
    )]
    pub emit_typed_ast: bool,

    #[arg(
        short = 'i',
        long = "emit-ir",
        help = "Emit Intermediate Representation (IR) text"
    )]
    pub emit_ir: bool,

    #[arg(
        long = "emit-mir",
        help = "Emit Mid-Level Intermediate Representation (MIR) text"
    )]
    pub emit_mir: bool,

    #[arg(
        long = "emit-memory-ssa",
        help = "Emit MemorySSA token graph representation text"
    )]
    pub emit_memory_ssa: bool,

    #[arg(
        long = "emit-supercompiled-mir",
        help = "Emit supercompiled MIR SSA representation"
    )]
    pub emit_supercompiled_mir: bool,

    #[arg(
        long = "emit-process-tree",
        help = "Emit supercompiler process tree graph"
    )]
    pub emit_process_tree: bool,

    #[arg(
        long = "supercompile-stats",
        help = "Display metrics and statistics from supercompilation"
    )]
    pub supercompile_stats: bool,

    #[arg(
        long = "emit-termination-proof",
        help = "Emit the supercompiler termination witness as JSON (one entry per function)"
    )]
    pub emit_termination_proof: bool,

    #[arg(
        long = "emit-obj",
        help = "Emit compiled native COFF object file (.obj)"
    )]
    pub emit_obj: Option<PathBuf>,

    #[arg(
        short = 'o',
        long = "output",
        help = "Compile and link into native standalone Windows executable (.exe)"
    )]
    pub output: Option<PathBuf>,

    #[arg(
        long = "bench",
        help = "Build with high-resolution in-process benchmarking entry"
    )]
    pub bench: bool,

    #[arg(
        long = "explain",
        help = "Explain a compiler error code (e.g. --explain E001)"
    )]
    pub explain: Option<String>,

    #[arg(
        long = "supercompile",
        help = "Supercompile MIR and compile via selected backend (Cranelift or LLVM)"
    )]
    pub supercompile: bool,

    #[arg(
        long = "mode",
        value_enum,
        default_value_t = SupercompileCliMode::Classic,
        help = "Supercompiler mode (classic | distill | mrsc)"
    )]
    pub mode: SupercompileCliMode,

    #[arg(
        long = "mrsc-objective",
        default_value = "size",
        help = "MRSC optimization objective (size | branch | pareto | speed | balanced)"
    )]
    pub mrsc_objective: String,

    #[arg(
        long = "mrsc-exhaustive",
        help = "Enable exhaustive MRSC supercompilation with IDDFS and Pareto cost optimization"
    )]
    pub mrsc_exhaustive: bool,

    #[arg(
        long = "ho-distill",
        help = "Enable pre-defunctionalization higher-order AST distillation (Hamilton fold/unfold)"
    )]
    pub ho_distill: bool,

    #[arg(
        long = "use-mir",
        help = "Compile MIR directly via Cranelift without AST codegen"
    )]
    pub use_mir: bool,

    #[arg(
        long = "backend",
        value_enum,
        default_value_t = Backend::Cranelift,
        help = "Codegen backend to use (cranelift | llvm)"
    )]
    pub backend: Backend,

    #[arg(
        long = "opt-level",
        default_value = "2",
        help = "Optimization level (0, 1, 2, 3)"
    )]
    pub opt_level: String,

    #[arg(
        long = "verify-equivalence",
        help = "Perform formal translation validation verifying semantic equivalence"
    )]
    pub verify_equivalence: bool,

    #[arg(
        long = "threads",
        default_value = "0",
        help = "Number of worker threads for parallel supercompilation (0 for auto)"
    )]
    pub threads: usize,

    #[arg(
        long = "parallel-residualize",
        help = "Emit parallel Fork/Join regions for data-independent loop residuals"
    )]
    pub parallel_residualize: bool,

    #[arg(
        long = "cache-dir",
        help = "Path to specialization cache directory (default: .numlang_cache/)",
        default_value = ".numlang_cache"
    )]
    pub cache_dir: PathBuf,

    #[arg(
        long = "no-cache",
        help = "Disable the specialization cache for this invocation"
    )]
    pub no_cache: bool,

    #[arg(
        long = "incremental",
        help = "Enable incremental modular supercompilation with fine-grained cache invalidation"
    )]
    pub incremental: bool,

    #[arg(
        long = "futamura2",
        help = "Execute 2nd Futamura projection to generate minspec_cogen executable"
    )]
    pub futamura2: bool,

    #[arg(
        long = "futamura2-out",
        help = "Output path for the generated minspec_cogen binary"
    )]
    pub futamura2_out: Option<PathBuf>,

    #[arg(help = "Path to source file (.nl)")]
    pub file: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Default)]
pub enum SupercompileCliMode {
    #[default]
    Classic,
    Distill,
    Mrsc,
    MrscExhaustive,
}

impl From<SupercompileCliMode> for numlang::mir::supercompiler::SupercompileMode {
    fn from(m: SupercompileCliMode) -> Self {
        match m {
            SupercompileCliMode::Classic => numlang::mir::supercompiler::SupercompileMode::Classic,
            SupercompileCliMode::Distill => numlang::mir::supercompiler::SupercompileMode::Distill,
            SupercompileCliMode::Mrsc => numlang::mir::supercompiler::SupercompileMode::Mrsc,
            SupercompileCliMode::MrscExhaustive => {
                numlang::mir::supercompiler::SupercompileMode::MrscExhaustive
            }
        }
    }
}

fn resolve_supercompile_mode(
    cli_mode: SupercompileCliMode,
    exhaustive: bool,
) -> numlang::mir::supercompiler::SupercompileMode {
    if exhaustive {
        numlang::mir::supercompiler::SupercompileMode::MrscExhaustive
    } else {
        cli_mode.into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Default)]
pub enum Backend {
    #[default]
    Cranelift,
    Llvm,
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Backend::Cranelift => write!(f, "cranelift"),
            Backend::Llvm => write!(f, "llvm"),
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Explain a compiler error code (e.g. E001)
    Explain {
        #[arg(help = "Error code to explain (e.g. E001)")]
        code: String,
    },
    /// Compile and run a numlang program directly
    Run {
        #[arg(help = "Path to source file (.nl)")]
        file: PathBuf,

        #[arg(
            long = "bench",
            help = "Run with high-resolution in-process benchmarking entry"
        )]
        bench: bool,

        #[arg(
            long = "supercompile",
            help = "Supercompile MIR and compile via selected backend (Cranelift or LLVM)"
        )]
        supercompile: bool,

        #[arg(
            long = "parallel-residualize",
            help = "Emit parallel Fork/Join regions for data-independent loop residuals"
        )]
        parallel_residualize: bool,

        #[arg(
            long = "cache-dir",
            help = "Path to specialization cache directory (default: .numlang_cache/)",
            default_value = ".numlang_cache"
        )]
        cache_dir: PathBuf,

        #[arg(
            long = "no-cache",
            help = "Disable the specialization cache for this invocation"
        )]
        no_cache: bool,

        #[arg(
            long = "incremental",
            help = "Enable incremental modular supercompilation with fine-grained cache invalidation"
        )]
        incremental: bool,

        #[arg(
            long = "mode",
            value_enum,
            default_value_t = SupercompileCliMode::Classic,
            help = "Supercompiler mode (classic | distill | mrsc)"
        )]
        mode: SupercompileCliMode,

        #[arg(
            long = "mrsc-objective",
            default_value = "size",
            help = "MRSC optimization objective (size | branch | pareto | speed | balanced)"
        )]
        mrsc_objective: String,

        #[arg(
            long = "mrsc-exhaustive",
            help = "Enable exhaustive MRSC supercompilation with IDDFS and Pareto cost optimization"
        )]
        mrsc_exhaustive: bool,

        #[arg(
            long = "ho-distill",
            help = "Enable pre-defunctionalization higher-order AST distillation (Hamilton fold/unfold)"
        )]
        ho_distill: bool,

        #[arg(
            long = "use-mir",
            help = "Compile MIR directly via Cranelift without AST codegen"
        )]
        use_mir: bool,

        #[arg(
            long = "backend",
            value_enum,
            default_value_t = Backend::Cranelift,
            help = "Codegen backend to use (cranelift | llvm)"
        )]
        backend: Backend,

        #[arg(
            long = "opt-level",
            default_value = "2",
            help = "Optimization level (0, 1, 2, 3)"
        )]
        opt_level: String,
    },
    /// Compile and link a numlang program into a native executable
    Build {
        #[arg(help = "Path to source file (.nl)")]
        file: PathBuf,

        #[arg(
            short = 'o',
            long = "output",
            help = "Output path for the compiled executable (defaults to <name>.exe)"
        )]
        output: Option<PathBuf>,

        #[arg(
            long = "emit-obj",
            help = "Emit compiled native COFF object file (.obj)"
        )]
        emit_obj: Option<PathBuf>,

        #[arg(
            long = "bench",
            help = "Build with high-resolution in-process benchmarking entry"
        )]
        bench: bool,

        #[arg(
            long = "supercompile",
            help = "Supercompile MIR and compile via selected backend (Cranelift or LLVM)"
        )]
        supercompile: bool,

        #[arg(
            long = "parallel-residualize",
            help = "Emit parallel Fork/Join regions for data-independent loop residuals"
        )]
        parallel_residualize: bool,

        #[arg(
            long = "cache-dir",
            help = "Path to specialization cache directory (default: .numlang_cache/)",
            default_value = ".numlang_cache"
        )]
        cache_dir: PathBuf,

        #[arg(
            long = "no-cache",
            help = "Disable the specialization cache for this invocation"
        )]
        no_cache: bool,

        #[arg(
            long = "incremental",
            help = "Enable incremental modular supercompilation with fine-grained cache invalidation"
        )]
        incremental: bool,

        #[arg(
            long = "mode",
            value_enum,
            default_value_t = SupercompileCliMode::Classic,
            help = "Supercompiler mode (classic | distill | mrsc)"
        )]
        mode: SupercompileCliMode,

        #[arg(
            long = "mrsc-objective",
            default_value = "size",
            help = "MRSC optimization objective (size | branch | pareto | speed | balanced)"
        )]
        mrsc_objective: String,

        #[arg(
            long = "mrsc-exhaustive",
            help = "Enable exhaustive MRSC supercompilation with IDDFS and Pareto cost optimization"
        )]
        mrsc_exhaustive: bool,

        #[arg(
            long = "ho-distill",
            help = "Enable pre-defunctionalization higher-order AST distillation (Hamilton fold/unfold)"
        )]
        ho_distill: bool,

        #[arg(
            long = "use-mir",
            help = "Compile MIR directly via Cranelift without AST codegen"
        )]
        use_mir: bool,

        #[arg(
            long = "backend",
            value_enum,
            default_value_t = Backend::Cranelift,
            help = "Codegen backend to use (cranelift | llvm)"
        )]
        backend: Backend,

        #[arg(
            long = "opt-level",
            default_value = "2",
            help = "Optimization level (0, 1, 2, 3)"
        )]
        opt_level: String,
    },
    /// Check a numlang program for syntax and type errors
    Check {
        #[arg(help = "Path to source file (.nl)")]
        file: PathBuf,
    },
    /// Format a numlang source file
    Fmt {
        #[arg(help = "Path to source file (.nl)")]
        file: PathBuf,

        #[arg(long = "check", help = "Exit 1 if file is not already formatted")]
        check: bool,

        #[arg(long = "stdout", help = "Print formatted output to stdout")]
        stdout: bool,
    },
    /// Generate Markdown documentation from doc-comments
    Doc {
        #[arg(help = "Path to source file (.nl)")]
        file: PathBuf,

        #[arg(short = 'o', long = "output", help = "Output markdown file path")]
        output: Option<PathBuf>,
    },
}

fn compile_source_to_typed(file_path: &Path) -> Result<(String, TypedProgram)> {
    let filename = file_path.to_string_lossy().to_string();
    let source = fs::read_to_string(file_path)
        .into_diagnostic()
        .map_err(|e| miette::miette!("Failed to read file '{}': {}", filename, e))?;

    // Lexing
    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(err) => {
            let diag = CompilerDiagnostic::from_lex_error(err, &filename, &source);
            return Err(diag.into());
        }
    };

    // Parsing
    let program = match parse(&tokens) {
        Ok(p) => p,
        Err(err) => {
            let diag = CompilerDiagnostic::from_parse_error(err, &filename, &source);
            return Err(diag.into());
        }
    };

    // Semantic Analysis & Type Checking
    let typed_program = match typecheck(&program) {
        Ok(tp) => tp,
        Err(err) => {
            let diag = CompilerDiagnostic::from_type_error(err, &filename, &source);
            return Err(diag.into());
        }
    };

    Ok((source, typed_program))
}

#[allow(clippy::too_many_arguments)]
fn compile_program_to_obj_bytes(
    typed_program: &TypedProgram,
    supercompile: bool,
    use_mir: bool,
    backend: Backend,
    opt_level: &str,
    mode: numlang::mir::supercompiler::SupercompileMode,
    mrsc_objective: &str,
    parallel_residualize: bool,
    cache_dir: &Path,
    no_cache: bool,
) -> Result<Vec<u8>> {
    let opt: numlang::codegen::OptLevel = opt_level
        .parse()
        .map_err(|e: String| miette::miette!("{}", e))?;

    let opt_cache = if (supercompile
        || mode != numlang::mir::supercompiler::SupercompileMode::Classic)
        && !no_cache
    {
        Some(numlang::mir::supercompiler::SpecializationCache::open(
            cache_dir,
        ))
    } else {
        None
    };

    match backend {
        Backend::Cranelift => {
            if supercompile || mode != numlang::mir::supercompiler::SupercompileMode::Classic {
                numlang::codegen::compile_supercompiled_to_obj_with_cache(
                    typed_program,
                    mode,
                    mrsc_objective,
                    parallel_residualize,
                    opt_cache.as_ref(),
                )
                .map_err(|e| miette::miette!("Codegen error: {}", e))
            } else if use_mir {
                let mir = numlang::mir::lower::lower_program(typed_program);
                numlang::codegen::compile_mir_to_obj(&mir)
                    .map_err(|e| miette::miette!("Codegen error: {}", e))
            } else {
                numlang::codegen::compile_to_obj_with_opt(typed_program, false)
                    .map_err(|e| miette::miette!("Codegen error: {}", e))
            }
        }
        Backend::Llvm => {
            let mut mir = numlang::mir::lower::lower_program(typed_program);
            if supercompile || mode != numlang::mir::supercompiler::SupercompileMode::Classic {
                numlang::mir::supercompiler::supercompile_mir_program_with_cache(
                    &mut mir,
                    mode,
                    mrsc_objective,
                    parallel_residualize,
                    opt_cache.as_ref(),
                );
            }
            let mut compiler = numlang::codegen::LlvmCompiler::new(opt);
            compiler
                .compile_mir_to_obj_bytes(&mir)
                .map_err(|e| miette::miette!("LLVM codegen error: {}", e))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_executable(
    typed_program: &TypedProgram,
    out_exe: &Path,
    supercompile: bool,
    use_mir: bool,
    backend: Backend,
    opt_level: &str,
    mode: numlang::mir::supercompiler::SupercompileMode,
    mrsc_objective: &str,
    parallel_residualize: bool,
    cache_dir: &Path,
    no_cache: bool,
) -> Result<()> {
    let obj_bytes = compile_program_to_obj_bytes(
        typed_program,
        supercompile,
        use_mir,
        backend,
        opt_level,
        mode,
        mrsc_objective,
        parallel_residualize,
        cache_dir,
        no_cache,
    )?;

    let temp_dir = std::env::temp_dir();
    let obj_file = temp_dir.join(format!(
        "numlang_build_{}_{}.obj",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    fs::write(&obj_file, obj_bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("Failed to write temporary object file: {}", e))?;

    let link_res = numlang::codegen::link_executable(&obj_file, out_exe);
    let _ = fs::remove_file(&obj_file);

    link_res.map_err(|e| miette::miette!("Linker error: {}", e))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_run(
    file: &Path,
    supercompile: bool,
    ho_distill: bool,
    use_mir: bool,
    backend: Backend,
    opt_level: &str,
    mode: numlang::mir::supercompiler::SupercompileMode,
    mrsc_objective: &str,
    parallel_residualize: bool,
    cache_dir: &Path,
    no_cache: bool,
) -> Result<()> {
    let (_, mut typed_program) = compile_source_to_typed(file)?;
    numlang::compiler::distill_and_optimize(
        &mut typed_program,
        &numlang::compiler::CompilerConfig {
            ho_distill,
            supercompile,
            ..Default::default()
        },
    );

    let temp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp_exe = temp_dir.join(format!(
        "numlang_run_{}_{}.exe",
        std::process::id(),
        timestamp
    ));

    build_executable(
        &typed_program,
        &temp_exe,
        supercompile,
        use_mir,
        backend,
        opt_level,
        mode,
        mrsc_objective,
        parallel_residualize,
        cache_dir,
        no_cache,
    )?;

    let status = Command::new(&temp_exe)
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("Failed to execute '{}': {}", temp_exe.display(), e))?;

    let _ = fs::remove_file(&temp_exe);

    if !status.success() {
        if let Some(code) = status.code() {
            std::process::exit(code);
        } else {
            std::process::exit(1);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_build(
    file: &Path,
    output: Option<PathBuf>,
    emit_obj: Option<PathBuf>,
    supercompile: bool,
    ho_distill: bool,
    use_mir: bool,
    backend: Backend,
    opt_level: &str,
    mode: numlang::mir::supercompiler::SupercompileMode,
    mrsc_objective: &str,
    parallel_residualize: bool,
    cache_dir: &Path,
    no_cache: bool,
) -> Result<()> {
    let (_, mut typed_program) = compile_source_to_typed(file)?;
    numlang::compiler::distill_and_optimize(
        &mut typed_program,
        &numlang::compiler::CompilerConfig {
            ho_distill,
            supercompile,
            ..Default::default()
        },
    );

    let obj_bytes = compile_program_to_obj_bytes(
        &typed_program,
        supercompile,
        use_mir,
        backend,
        opt_level,
        mode,
        mrsc_objective,
        parallel_residualize,
        cache_dir,
        no_cache,
    )?;

    if let Some(ref obj_path) = emit_obj {
        fs::write(obj_path, &obj_bytes)
            .into_diagnostic()
            .map_err(|e| miette::miette!("Failed to write object file: {}", e))?;
        println!("numlang: emitted object file: {}", obj_path.display());
    }

    if output.is_some() || emit_obj.is_none() {
        let out_exe = output.unwrap_or_else(|| file.with_extension("exe"));
        let temp_dir = std::env::temp_dir();
        let obj_file = if let Some(ref obj_path) = emit_obj {
            obj_path.clone()
        } else {
            temp_dir.join(format!(
                "numlang_build_{}_{}.obj",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ))
        };

        if emit_obj.is_none() {
            fs::write(&obj_file, &obj_bytes)
                .into_diagnostic()
                .map_err(|e| miette::miette!("Failed to write temporary object file: {}", e))?;
        }

        let link_res = numlang::codegen::link_executable(&obj_file, &out_exe);
        if emit_obj.is_none() {
            let _ = fs::remove_file(&obj_file);
        }

        link_res.map_err(|e| miette::miette!("Linker error: {}", e))?;
        println!("numlang: generated executable: {}", out_exe.display());
    }

    Ok(())
}

fn handle_check(file: &Path) -> Result<()> {
    let (_, typed_program) = compile_source_to_typed(file)?;
    println!(
        "numlang: type check passed. Verified {} function(s).",
        typed_program.functions.len()
    );
    Ok(())
}

fn handle_fmt(file: &Path, check: bool, stdout: bool) -> Result<()> {
    if file.extension().and_then(|e| e.to_str()) == Some("rs") {
        let mut cmd = Command::new("rustfmt");
        if check {
            cmd.arg("--check");
        } else if stdout {
            cmd.arg("--emit").arg("stdout");
        }
        cmd.arg(file);
        let status = cmd
            .status()
            .map_err(|e| miette::miette!("Failed to run rustfmt: {}", e))?;
        if check && !status.success() {
            std::process::exit(1);
        }
        return Ok(());
    }

    let filename = file.to_string_lossy().to_string();
    let source = fs::read_to_string(file)
        .into_diagnostic()
        .map_err(|e| miette::miette!("Failed to read file '{}': {}", filename, e))?;

    let formatted = numlang::fmt::format_source(&source)
        .map_err(|e| miette::miette!("Formatting error in '{}': {}", filename, e))?;

    if check {
        if source.replace("\r\n", "\n") != formatted.replace("\r\n", "\n") {
            std::process::exit(1);
        }
        return Ok(());
    }

    if stdout {
        print!("{}", formatted);
        return Ok(());
    }

    if source.replace("\r\n", "\n") != formatted.replace("\r\n", "\n") {
        fs::write(file, &formatted)
            .into_diagnostic()
            .map_err(|e| miette::miette!("Failed to write file '{}': {}", filename, e))?;
    }

    Ok(())
}

fn handle_doc(file: &Path, output: Option<PathBuf>) -> Result<()> {
    let filename = file.to_string_lossy().to_string();
    let source = fs::read_to_string(file)
        .into_diagnostic()
        .map_err(|e| miette::miette!("Failed to read file '{}': {}", filename, e))?;

    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(err) => {
            let diag = CompilerDiagnostic::from_lex_error(err, &filename, &source);
            return Err(diag.into());
        }
    };

    let program = match parse(&tokens) {
        Ok(p) => p,
        Err(err) => {
            let diag = CompilerDiagnostic::from_parse_error(err, &filename, &source);
            return Err(diag.into());
        }
    };

    let markdown = numlang::doc::generate_doc(&program);
    if let Some(out_path) = output {
        fs::write(&out_path, &markdown)
            .into_diagnostic()
            .map_err(|e| miette::miette!("Failed to write doc file: {}", e))?;
    } else {
        print!("{}", markdown);
    }
    Ok(())
}

fn main() -> Result<()> {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(real_main)
        .map_err(|e| miette::miette!("Failed to spawn thread: {}", e))?
        .join()
        .map_err(|_| miette::miette!("Main thread panicked"))?
}

fn real_main() -> Result<()> {
    let cli = Cli::parse();

    if cli.bench {
        std::env::set_var("NUMLANG_BENCH", "1");
    }

    if let Some(code) = &cli.explain {
        if numlang::explain::print_explanation(code) {
            return Ok(());
        } else {
            std::process::exit(1);
        }
    }

    // Handle subcommands if provided
    if let Some(command) = cli.command {
        match command {
            Commands::Explain { code } => {
                if numlang::explain::print_explanation(&code) {
                    return Ok(());
                } else {
                    std::process::exit(1);
                }
            }
            Commands::Run {
                file,
                bench,
                supercompile,
                parallel_residualize,
                cache_dir,
                no_cache,
                incremental,
                mode,
                mrsc_objective,
                mrsc_exhaustive,
                ho_distill,
                use_mir,
                backend,
                opt_level,
            } => {
                if bench {
                    std::env::set_var("NUMLANG_BENCH", "1");
                }
                return handle_run(
                    &file,
                    supercompile || incremental,
                    ho_distill,
                    use_mir,
                    backend,
                    &opt_level,
                    resolve_supercompile_mode(mode, mrsc_exhaustive),
                    &mrsc_objective,
                    parallel_residualize,
                    &cache_dir,
                    no_cache,
                );
            }
            Commands::Build {
                file,
                output,
                emit_obj,
                bench,
                supercompile,
                parallel_residualize,
                cache_dir,
                no_cache,
                incremental,
                mode,
                mrsc_objective,
                mrsc_exhaustive,
                ho_distill,
                use_mir,
                backend,
                opt_level,
            } => {
                if bench {
                    std::env::set_var("NUMLANG_BENCH", "1");
                }
                return handle_build(
                    &file,
                    output,
                    emit_obj,
                    supercompile || incremental,
                    ho_distill,
                    use_mir,
                    backend,
                    &opt_level,
                    resolve_supercompile_mode(mode, mrsc_exhaustive),
                    &mrsc_objective,
                    parallel_residualize,
                    &cache_dir,
                    no_cache,
                );
            }
            Commands::Check { file } => return handle_check(&file),
            Commands::Fmt {
                file,
                check,
                stdout,
            } => return handle_fmt(&file, check, stdout),
            Commands::Doc { file, output } => return handle_doc(&file, output),
        }
    }

    if cli.futamura2 {
        let out_target = cli.futamura2_out.unwrap_or_else(|| {
            PathBuf::from("target/release").join(if cfg!(windows) {
                "minspec_cogen.exe"
            } else {
                "minspec_cogen"
            })
        });
        match numlang::mir::supercompiler::futamura2::build_minspec_cogen_binary(&out_target) {
            Ok(path) => {
                println!(
                    "numlang: 2nd Futamura projection completed successfully. Generated cogen binary: {}",
                    path.display()
                );
                return Ok(());
            }
            Err(e) => {
                eprintln!("numlang: 2nd Futamura cogen generation failed: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Top-level flag / legacy positional mode
    let file_path = match cli.file {
        Some(path) => path,
        None => {
            eprintln!("numlang: error: no input file provided. Run with --help for usage.");
            std::process::exit(1);
        }
    };

    let filename = file_path.to_string_lossy().to_string();
    let source = fs::read_to_string(&file_path)
        .into_diagnostic()
        .map_err(|e| miette::miette!("Failed to read file '{}': {}", filename, e))?;

    // Lexing
    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(err) => {
            let diag = CompilerDiagnostic::from_lex_error(err, &filename, &source);
            return Err(diag.into());
        }
    };

    if cli.emit_tokens {
        print!("{}", format_tokens(&tokens));
        return Ok(());
    }

    // Parsing
    let program = match parse(&tokens) {
        Ok(p) => p,
        Err(err) => {
            let diag = CompilerDiagnostic::from_parse_error(err, &filename, &source);
            return Err(diag.into());
        }
    };

    if cli.emit_ast {
        println!("{}", format_ast(&program));
        return Ok(());
    }

    // Semantic Analysis & Type Checking
    let typed_program = match typecheck(&program) {
        Ok(tp) => tp,
        Err(err) => {
            let diag = CompilerDiagnostic::from_type_error(err, &filename, &source);
            return Err(diag.into());
        }
    };

    if cli.emit_typed_ast {
        println!("{}", format_typed_ast(&typed_program));
        return Ok(());
    }

    if cli.check {
        println!(
            "numlang: type check passed. Verified {} function(s).",
            typed_program.functions.len()
        );
        return Ok(());
    }

    // Optimization pass (run once)
    let mut typed_program = typed_program;
    numlang::compiler::distill_and_optimize(
        &mut typed_program,
        &numlang::compiler::CompilerConfig {
            ho_distill: cli.ho_distill,
            supercompile: cli.supercompile,
            ..Default::default()
        },
    );

    if cli.emit_mir {
        let mir_program = numlang::mir::lower::lower_program(&typed_program);
        println!("{:#?}", mir_program);
        return Ok(());
    }

    if cli.emit_memory_ssa {
        let mir_program = numlang::mir::lower::lower_program(&typed_program);
        for func in &mir_program.functions {
            let mssa = numlang::mir::memory_ssa::MemorySSA::build(func);
            println!("{}", mssa.display(func));
        }
        return Ok(());
    }

    if cli.emit_supercompiled_mir {
        let mut mir_program = numlang::mir::lower::lower_program(&typed_program);
        let opt_cache = if !cli.no_cache {
            Some(numlang::mir::supercompiler::SpecializationCache::open(
                &cli.cache_dir,
            ))
        } else {
            None
        };
        numlang::mir::supercompiler::supercompile_mir_program_with_cache(
            &mut mir_program,
            cli.mode.into(),
            &cli.mrsc_objective,
            cli.parallel_residualize,
            opt_cache.as_ref(),
        );
        println!("{:#?}", mir_program);
        return Ok(());
    }

    if cli.emit_process_tree {
        let mir_program = numlang::mir::lower::lower_program(&typed_program);
        for func in &mir_program.functions {
            let tree = numlang::mir::supercompiler::build_process_tree(func);
            println!("Function `{}`:\n{}", func.name, tree.display());
        }
        return Ok(());
    }

    if cli.emit_termination_proof {
        let mir_program = numlang::mir::lower::lower_program(&typed_program);
        for func in &mir_program.functions {
            let driver = numlang::mir::supercompiler::SupercompilerDriver::new(func)
                .with_program_functions(&mir_program.functions);
            let tree = driver.run();
            let firings = &tree.witness.firings;
            if firings.is_empty() {
                println!(
                    r#"{{"function_name": "{}", "total_firings": 0, "note": "no whistle fired — function terminates trivially"}}"#,
                    func.name
                );
            } else {
                let mut firings_json = String::new();
                firings_json.push('[');
                for (i, f) in firings.iter().enumerate() {
                    if i > 0 {
                        firings_json.push_str(", ");
                    }
                    let kind_str = match f.kind {
                        numlang::mir::supercompiler::WhistleKind::HomeomorphicEmbedding => {
                            "HomeomorphicEmbedding"
                        }
                        numlang::mir::supercompiler::WhistleKind::HeaderVisitCutoff => {
                            "HeaderVisitCutoff"
                        }
                    };
                    firings_json.push_str(&format!(
                        r#"{{"from_id": {}, "ancestor_id": {}, "kind": "{}"}}"#,
                        f.from_id.0, f.ancestor_id.0, kind_str
                    ));
                }
                firings_json.push(']');
                println!(
                    r#"{{"function_name": "{}", "total_firings": {}, "firings": {}}}"#,
                    func.name,
                    firings.len(),
                    firings_json
                );
            }
        }
        return Ok(());
    }

    let effective_mode = resolve_supercompile_mode(cli.mode, cli.mrsc_exhaustive);

    if cli.supercompile_stats {
        let mut mir_program = numlang::mir::lower::lower_program(&typed_program);
        let opt_cache = if !cli.no_cache {
            Some(numlang::mir::supercompiler::SpecializationCache::open(
                &cli.cache_dir,
            ))
        } else {
            None
        };
        let stats = numlang::mir::supercompiler::supercompile_mir_program_with_cache(
            &mut mir_program,
            effective_mode,
            &cli.mrsc_objective,
            cli.parallel_residualize,
            opt_cache.as_ref(),
        );
        println!("{}", stats);
        println!("  residual blocks:   {}", stats.residual_block_count);
        println!("  residual stmts:    {}", stats.residual_stmt_count);
        return Ok(());
    }

    if cli.verify_equivalence {
        let orig_mir = numlang::mir::lower::lower_program(&typed_program);
        let mut sc_mir = orig_mir.clone();
        numlang::mir::supercompiler::supercompile_mir_program_with_mode(
            &mut sc_mir,
            effective_mode,
            &cli.mrsc_objective,
        );
        match numlang::mir::supercompiler::verify_program_equivalence(&orig_mir, &sc_mir) {
            Ok(certs) => {
                println!(
                    "numlang: translation validation certified {} function(s) successfully.",
                    certs.len()
                );
                for cert in certs {
                    println!(
                        "  ✓ `{}`: verified {} block(s) across {} path(s)",
                        cert.function_name, cert.residual_blocks, cert.paths_verified
                    );
                }
            }
            Err(e) => {
                eprintln!("numlang: translation validation error: {}", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    if cli.emit_ir {
        let ir_program = numlang::ir::lower::lower_to_ir(&typed_program);
        print!("{}", numlang::ir::format_ir(&ir_program));
        return Ok(());
    }

    let should_emit_obj = cli.emit_obj.is_some();
    let should_link_exe = cli.output.is_some();

    if should_emit_obj || should_link_exe {
        let obj_bytes = compile_program_to_obj_bytes(
            &typed_program,
            cli.supercompile || cli.incremental,
            cli.use_mir,
            cli.backend,
            &cli.opt_level,
            effective_mode,
            &cli.mrsc_objective,
            cli.parallel_residualize,
            &cli.cache_dir,
            cli.no_cache,
        )?;

        if let Some(ref obj_path) = cli.emit_obj {
            fs::write(obj_path, &obj_bytes)
                .into_diagnostic()
                .map_err(|e| miette::miette!("Failed to write object file: {}", e))?;
            println!("numlang: emitted object file: {}", obj_path.display());
        }

        if let Some(ref out_exe) = cli.output {
            let temp_dir = std::env::temp_dir();
            let obj_file = if let Some(ref obj_path) = cli.emit_obj {
                obj_path.clone()
            } else {
                temp_dir.join(format!(
                    "numlang_build_{}_{}.obj",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0)
                ))
            };

            if cli.emit_obj.is_none() {
                fs::write(&obj_file, &obj_bytes)
                    .into_diagnostic()
                    .map_err(|e| miette::miette!("Failed to write temporary object file: {}", e))?;
            }

            let link_res = numlang::codegen::link_executable(&obj_file, out_exe);
            if cli.emit_obj.is_none() {
                let _ = fs::remove_file(&obj_file);
            }

            link_res.map_err(|e| miette::miette!("Linker error: {}", e))?;
            println!("numlang: generated executable: {}", out_exe.display());
        }

        return Ok(());
    }

    println!(
        "numlang: compiled and verified {} function(s) successfully.",
        typed_program.functions.len()
    );
    Ok(())
}
