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

    println!(
        "numlang: compiled and verified {} function(s) successfully.",
        typed_program.functions.len()
    );
    Ok(())
}
