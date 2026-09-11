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

    #[error("Type error: {message}")]
    #[diagnostic(
        code(numlang::typecheck::type_error),
        help("{help}")
    )]
    TypeError {
        message: String,
        help: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("{label}")]
        span: SourceSpan,
        label: String,
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

    pub fn from_type_error(err: crate::typecheck::TypeError, filename: &str, source: &str) -> Self {
        let span = err.span();
        let (message, label, help) = match &err {
            crate::typecheck::TypeError::TypeMismatch { expected, found, .. } => (
                format!("Type mismatch: expected `{}`, found `{}`", expected, found),
                format!("expected `{}`, found `{}`", expected, found),
                "numlang requires exact type matching without implicit conversions".to_string(),
            ),
            crate::typecheck::TypeError::UndeclaredVariable { name, .. } => (
                format!("Undeclared variable `{}`", name),
                "not found in this scope".to_string(),
                "Ensure variable is declared with `let` before use".to_string(),
            ),
            crate::typecheck::TypeError::UndeclaredFunction { name, .. } => (
                format!("Undeclared function `{}`", name),
                "function not declared".to_string(),
                "Define the function with `fn` before calling it".to_string(),
            ),
            crate::typecheck::TypeError::CannotMutateImmutable { name, .. } => (
                format!("Cannot mutate immutable variable `{}`", name),
                "cannot assign twice to immutable variable".to_string(),
                "Consider declaring the variable as mutable: `let mut`".to_string(),
            ),
            crate::typecheck::TypeError::DuplicateDeclaration { name, .. } => (
                format!("Identifier `{}` is already declared in this scope", name),
                "duplicate definition".to_string(),
                "Use a different name or remove redundant declaration".to_string(),
            ),
            crate::typecheck::TypeError::InvalidConditionType { found, .. } => (
                format!("Condition must evaluate to `bool`, found `{}`", found),
                format!("expected `bool`, found `{}`", found),
                "Use a comparison or boolean expression in if/while conditions".to_string(),
            ),
            crate::typecheck::TypeError::InvalidBinaryOperands { op, left, right, .. } => (
                format!("Cannot apply operator `{:?}` to `{}` and `{}`", op, left, right),
                "mismatched operand types".to_string(),
                "Binary operators require matching numeric or boolean operands".to_string(),
            ),
            crate::typecheck::TypeError::InvalidUnaryOperand { op, found, .. } => (
                format!("Cannot apply unary operator `{:?}` to `{}`", op, found),
                "invalid operand type".to_string(),
                "Ensure operand type matches operator expectation".to_string(),
            ),
            crate::typecheck::TypeError::ArityMismatch { name, expected, found, .. } => (
                format!("Function `{}` expected {} arguments, received {}", name, expected, found),
                format!("expected {} arguments", expected),
                "Ensure call site passes the correct number of arguments".to_string(),
            ),
            crate::typecheck::TypeError::UnknownType { name, .. } => (
                format!("Unknown type `{}`", name),
                "unrecognized type name".to_string(),
                "Supported primitive types are: i32, i64, f32, f64, bool, void".to_string(),
            ),
            crate::typecheck::TypeError::InvalidReturn { expected, found, .. } => (
                format!("Function return type mismatch: expected `{}`, found `{}`", expected, found),
                format!("expected `{}`, returned `{}`", expected, found),
                "Ensure returned value matches the function's declared return type".to_string(),
            ),
            crate::typecheck::TypeError::CannotIndexNonArray { found, .. } => (
                format!("Cannot index non-array type `{}`", found),
                "indexing requires array type `[T; N]`".to_string(),
                "Only array types can be indexed with `[i]`".to_string(),
            ),
            crate::typecheck::TypeError::InvalidIndexType { found, .. } => (
                format!("Array index must be an integer, found `{}`", found),
                "invalid index type".to_string(),
                "Array indices must be integer types: i64 or i32".to_string(),
            ),
            crate::typecheck::TypeError::EmptyArrayLiteral { .. } => (
                "Array literal cannot be empty".to_string(),
                "empty array literal".to_string(),
                "Provide at least one element in the array literal".to_string(),
            ),
            crate::typecheck::TypeError::IndexOutOfBounds { index, len, .. } => (
                format!("Array index {} out of bounds for array of length {}", index, len),
                format!("index {} >= length {}", index, len),
                "Ensure index is within 0 <= index < length".to_string(),
            ),
            crate::typecheck::TypeError::ArrayElementMismatch { expected, found, .. } => (
                format!("Array element type mismatch: expected `{}`, found `{}`", expected, found),
                format!("expected `{}`, found `{}`", expected, found),
                "All elements in an array literal must share the exact same type".to_string(),
            ),
            crate::typecheck::TypeError::BreakOutsideLoop { .. } => (
                "`break` used outside a loop".to_string(),
                "no enclosing loop".to_string(),
                "Use `break;` only inside a `while` loop".to_string(),
            ),
        };

        CompilerDiagnostic::TypeError {
            message,
            help,
            src: NamedSource::new(filename, source.to_string()),
            span: span.into(),
            label,
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

pub fn format_typed_ast(program: &crate::typecheck::TypedProgram) -> String {
    format!("{:#?}", program)
}
