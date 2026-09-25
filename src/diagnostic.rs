use crate::ast::Program;
use crate::parser::ParseError;
use crate::token::{LexError, SpannedToken, Token};
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug, Clone)]
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
    #[diagnostic(
        code(numlang::parser::syntax_error),
        help("{help}")
    )]
    SyntaxError {
        message: String,
        help: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("syntax error occurred here")]
        span: SourceSpan,
    },

    #[error("Type error [{code}]: {message}")]
    #[diagnostic(
        code(numlang::typecheck::type_error),
        help("{help}")
    )]
    TypeError {
        code: &'static str,
        message: String,
        help: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("{label}")]
        span: SourceSpan,
        label: String,
        #[label("{secondary_label}")]
        secondary_span: Option<SourceSpan>,
        secondary_label: String,
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
            ParseError::UnexpectedEof { expected, span } => {
                let help = if expected.contains("variable name") {
                    "Did you mean: let <name>: <type> = <value>?".to_string()
                } else if expected.contains("return type") || expected.contains("->") {
                    "Did you forget the return type arrow ->?".to_string()
                } else {
                    format!("Expected {}", expected)
                };
                CompilerDiagnostic::SyntaxError {
                    message: format!("Unexpected end of input, expected {}", expected),
                    help,
                    src: NamedSource::new(filename, source.to_string()),
                    span: span.into(),
                }
            }
            ParseError::UnexpectedToken {
                found,
                expected,
                span,
            } => {
                let span_start = span.start;
                let prefix = if span_start <= source.len() {
                    &source[..span_start]
                } else {
                    ""
                };
                let trimmed_prefix = prefix.trim_end();

                let help = if expected.contains("variable name")
                    || trimmed_prefix.ends_with("let")
                    || trimmed_prefix.ends_with("let mut")
                {
                    "Did you mean: let <name>: <type> = <value>?".to_string()
                } else if expected.contains("return type")
                    || expected.contains("->")
                    || (found == Token::LBrace && (trimmed_prefix.ends_with(')') || expected.contains("type")))
                {
                    "Did you forget the return type arrow ->?".to_string()
                } else {
                    format!("Expected {}", expected)
                };

                CompilerDiagnostic::SyntaxError {
                    message: format!("Unexpected token '{:?}', expected {}", found, expected),
                    help,
                    src: NamedSource::new(filename, source.to_string()),
                    span: span.into(),
                }
            }
            ParseError::InvalidPrefix { found, span } => CompilerDiagnostic::SyntaxError {
                message: format!("Invalid prefix operator '{:?}'", found),
                help: "Ensure operator is valid in prefix position (e.g. '-' or '!')".to_string(),
                src: NamedSource::new(filename, source.to_string()),
                span: span.into(),
            },
        }
    }

    pub fn from_type_error(err: crate::typecheck::TypeError, filename: &str, source: &str) -> Self {
        let span = err.span();
        let code = err.error_code();
        let mut secondary_span: Option<SourceSpan> = None;
        let mut secondary_label = String::new();

        let (message, label, base_help) = match &err {
            crate::typecheck::TypeError::TypeMismatch { expected, found, .. } => {
                let err_start = span.start;
                if err_start <= source.len() {
                    let prefix = &source[..err_start];
                    if let Some(colon_pos) = prefix.rfind(':') {
                        let between = &source[colon_pos..err_start];
                        if !between.contains(';') && between.contains('=') {
                            if let Some(eq_pos) = between.find('=') {
                                let ty_str = source[colon_pos + 1..colon_pos + eq_pos].trim();
                                if !ty_str.is_empty() {
                                    if let Some(offset) = source[colon_pos + 1..colon_pos + eq_pos].find(ty_str) {
                                        let s = colon_pos + 1 + offset;
                                        let l = ty_str.len();
                                        secondary_span = Some((s, l).into());
                                        secondary_label = format!("declared type `{}` specified here", ty_str);
                                    }
                                }
                            }
                        }
                    }
                }
                (
                    format!("Type mismatch: expected `{}`, found `{}`", expected, found),
                    format!("expected `{}`, found `{}`", expected, found),
                    "numlang requires exact type matching without implicit conversions".to_string(),
                )
            }
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
            crate::typecheck::TypeError::ArityMismatch { name, expected, found, .. } => {
                let search = format!("fn {}", name);
                if let Some(pos) = source.find(&search) {
                    secondary_span = Some((pos, search.len()).into());
                    secondary_label = format!("function `{}` defined here", name);
                }
                (
                    format!("Function `{}` expected {} arguments, received {}", name, expected, found),
                    format!("expected {} arguments", expected),
                    "Ensure call site passes the correct number of arguments".to_string(),
                )
            }
            crate::typecheck::TypeError::UnknownType { name, .. } => (
                format!("Unknown type `{}`", name),
                "unrecognized type name".to_string(),
                "Supported primitive types are: i8, i16, i32, i64, u8, u16, u32, u64, usize, f32, f64, bool, void".to_string(),
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
                "Use `break;` only inside a loop".to_string(),
            ),
            crate::typecheck::TypeError::ContinueOutsideLoop { .. } => (
                "`continue` used outside a loop".to_string(),
                "no enclosing loop".to_string(),
                "Use `continue;` only inside a loop".to_string(),
            ),
            crate::typecheck::TypeError::NoSuchField { name, field, .. } => (
                format!("Struct `{}` has no field `{}`", name, field),
                format!("field `{}` not found on `{}`", field, name),
                "Check struct definition for available fields".to_string(),
            ),
            crate::typecheck::TypeError::MissingField { name, field, .. } => (
                format!("Missing field `{}` in struct `{}` initialization", field, name),
                format!("missing field `{}`", field),
                "Provide all declared fields in the struct literal".to_string(),
            ),
            crate::typecheck::TypeError::CannotAccessFieldNonStruct { found, .. } => (
                format!("Cannot access field on non-struct type `{}`", found),
                "not a struct".to_string(),
                "Field access `.` requires a struct type".to_string(),
            ),
            crate::typecheck::TypeError::NonExhaustiveMatch { .. } => (
                "Non-exhaustive pattern match".to_string(),
                "missing wildcard pattern `_` or uncovered cases".to_string(),
                "Add a wildcard arm `_ => ...` to cover all remaining cases".to_string(),
            ),
            crate::typecheck::TypeError::EmptyMatch { .. } => (
                "Match expression cannot be empty".to_string(),
                "empty match block".to_string(),
                "Provide at least one match arm in the match expression".to_string(),
            ),
            crate::typecheck::TypeError::NoSuchVariant { enum_name, variant_name, .. } => (
                format!("Enum `{}` has no variant `{}`", enum_name, variant_name),
                format!("variant `{}` not found in enum `{}`", variant_name, enum_name),
                "Check the enum definition for available variants".to_string(),
            ),
            crate::typecheck::TypeError::CannotMatchNonEnum { found, .. } => (
                format!("Cannot match variant pattern on non-enum type `{}`", found),
                "not an enum type".to_string(),
                "Variant patterns can only match enum types".to_string(),
            ),
            crate::typecheck::TypeError::PayloadArityMismatch { enum_name, variant_name, expected, found, .. } => (
                format!("Variant `{}::{}` expected {} payload arguments, found {}", enum_name, variant_name, expected, found),
                format!("expected {} arguments, found {}", expected, found),
                "Provide the correct number of payload arguments for this variant".to_string(),
            ),
            crate::typecheck::TypeError::NotCallable { ty, .. } => (
                format!("Type `{}` is not callable", ty),
                "not callable".to_string(),
                "Only functions and closures can be called".to_string(),
            ),
        };

        let help = format!("{} (see 'numlang --explain {}')", base_help, code);

        CompilerDiagnostic::TypeError {
            code,
            message,
            help,
            src: NamedSource::new(filename, source.to_string()),
            span: span.into(),
            label,
            secondary_span,
            secondary_label,
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
