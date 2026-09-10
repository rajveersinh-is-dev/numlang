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

    #[arg(help = "Path to source file (.nl)")]
    pub file: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Compile and run a numlang program directly
    Run {
        #[arg(help = "Path to source file (.nl)")]
        file: PathBuf,

        #[arg(
            long = "bench",
            help = "Run with high-resolution in-process benchmarking entry"
        )]
        bench: bool,
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
    },
    /// Check a numlang program for syntax and type errors
    Check {
        #[arg(help = "Path to source file (.nl)")]
        file: PathBuf,
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

fn build_executable(typed_program: &TypedProgram, out_exe: &Path) -> Result<()> {
    let obj_bytes = numlang::codegen::compile_to_obj(typed_program)
        .map_err(|e| miette::miette!("Codegen error: {}", e))?;

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

fn handle_run(file: &Path) -> Result<()> {
    let (_, typed_program) = compile_source_to_typed(file)?;

    let temp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp_exe = temp_dir.join(format!("numlang_run_{}_{}.exe", std::process::id(), timestamp));

    build_executable(&typed_program, &temp_exe)?;

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

fn handle_build(file: &Path, output: Option<PathBuf>, emit_obj: Option<PathBuf>) -> Result<()> {
    let (_, typed_program) = compile_source_to_typed(file)?;

    if let Some(obj_path) = emit_obj {
        let obj_bytes = numlang::codegen::compile_to_obj(&typed_program)
            .map_err(|e| miette::miette!("Codegen error: {}", e))?;
        fs::write(&obj_path, obj_bytes)
            .into_diagnostic()
            .map_err(|e| miette::miette!("Failed to write object file: {}", e))?;
        println!("numlang: emitted object file: {}", obj_path.display());
        return Ok(());
    }

    let out_exe = output.unwrap_or_else(|| file.with_extension("exe"));
    build_executable(&typed_program, &out_exe)?;
    println!("numlang: generated executable: {}", out_exe.display());
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.bench {
        std::env::set_var("NUMLANG_BENCH", "1");
    }

    // Handle subcommands if provided
    if let Some(command) = cli.command {
        match command {
            Commands::Run { file, bench } => {
                if bench {
                    std::env::set_var("NUMLANG_BENCH", "1");
                }
                return handle_run(&file);
            }
            Commands::Build {
                file,
                output,
                emit_obj,
                bench,
            } => {
                if bench {
                    std::env::set_var("NUMLANG_BENCH", "1");
                }
                return handle_build(&file, output, emit_obj);
            }
            Commands::Check { file } => return handle_check(&file),
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

    // Intermediate Representation (IR) Lowering
    let ir_program = numlang::ir::lower::lower_to_ir(&typed_program);

    if cli.emit_ir {
        print!("{}", numlang::ir::format_ir(&ir_program));
        return Ok(());
    }

    if let Some(obj_path) = &cli.emit_obj {
        let obj_bytes = numlang::codegen::compile_to_obj(&typed_program)
            .map_err(|e| miette::miette!("Codegen error: {}", e))?;
        fs::write(obj_path, obj_bytes)
            .into_diagnostic()
            .map_err(|e| miette::miette!("Failed to write object file: {}", e))?;
        println!("numlang: emitted object file: {}", obj_path.display());
        return Ok(());
    }

    if let Some(out_exe) = &cli.output {
        build_executable(&typed_program, out_exe)?;
        println!("numlang: generated executable: {}", out_exe.display());
        return Ok(());
    }

    println!(
        "numlang: compiled and verified {} function(s) successfully.",
        typed_program.functions.len()
    );
    Ok(())
}
