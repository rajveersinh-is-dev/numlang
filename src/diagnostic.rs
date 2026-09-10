use crate::ast::Program;
use crate::parser::ParseError;
use crate::token::{LexError, SpannedToken};
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum CompilerDiagnostic {
    #[error("Lexer error: unrecognized character or token")]
    #[diagnostic(
        code(numlang::lexer::invalid_token),
        help("Ensure numeric literals and characters follow numlang syntax rules")
    )]
    LexError {
        #[source_code]
        src: NamedSource<String>,
        #[label("unrecognized token")]
        span: SourceSpan,
    },

    #[error("Syntax error: {message}")]
    #[diagnostic(code(numlang::parser::syntax_error))]
    SyntaxError {
        message: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("syntax error occurred here")]
        span: SourceSpan,
    },
}

impl CompilerDiagnostic {
    pub fn from_lex_error(err: LexError, filename: &str, source: &str) -> Self {
        match err {
            LexError::InvalidToken(span) => CompilerDiagnostic::LexError {
                src: NamedSource::new(filename, source.to_string()),
                span: span.into(),
            },
        }
    }

    pub fn from_parse_error(err: ParseError, filename: &str, source: &str) -> Self {
        match err {
            ParseError::UnexpectedEof { expected, span } => CompilerDiagnostic::SyntaxError {
                message: format!("Unexpected end of input, expected {}", expected),
                src: NamedSource::new(filename, source.to_string()),
                span: span.into(),
            },
            ParseError::UnexpectedToken {
                found,
                expected,
                span,
            } => CompilerDiagnostic::SyntaxError {
                message: format!("Unexpected token '{:?}', expected {}", found, expected),
                src: NamedSource::new(filename, source.to_string()),
                span: span.into(),
            },
            ParseError::InvalidPrefix { found, span } => CompilerDiagnostic::SyntaxError {
                message: format!("Invalid prefix operator '{:?}'", found),
                src: NamedSource::new(filename, source.to_string()),
                span: span.into(),
            },
        }
    }
}

pub fn format_tokens(tokens: &[SpannedToken]) -> String {
    let mut out = String::new();
    for (i, tok) in tokens.iter().enumerate() {
        out.push_str(&format!(
            "[{:03}] {:<24} (span: {}..{})\n",
            i,
            format!("{:?}", tok.token),
            tok.span.start,
            tok.span.end
        ));
    }
    out
}

pub fn format_ast(program: &Program) -> String {
    format!("{:#?}", program)
}
