use clap::Parser;
use miette::{IntoDiagnostic, Result};
use numlang::diagnostic::{format_ast, format_tokens, format_typed_ast, CompilerDiagnostic};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "numlang",
    version = "0.1.0",
    about = "numlang mathematical systems programming language compiler"
)]
pub struct Cli {
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

    #[arg(help = "Path to source file (.nl)")]
    pub file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

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
        let obj_bytes = numlang::codegen::compile_to_obj(&typed_program)
            .map_err(|e| miette::miette!("Codegen error: {}", e))?;
        let temp_dir = std::env::temp_dir();
        let obj_file = temp_dir.join(format!("numlang_{}.obj", std::process::id()));
        fs::write(&obj_file, obj_bytes)
            .into_diagnostic()
            .map_err(|e| miette::miette!("Failed to write temporary object file: {}", e))?;

        numlang::codegen::link_executable(&obj_file, out_exe)
            .map_err(|e| miette::miette!("Linker error: {}", e))?;

        let _ = fs::remove_file(&obj_file);
        println!("numlang: generated executable: {}", out_exe.display());
        return Ok(());
    }

    println!(
        "numlang: compiled and verified {} function(s) successfully.",
        typed_program.functions.len()
    );
    Ok(())
}
