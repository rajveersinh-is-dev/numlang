use numlang::diagnostic::CompilerDiagnostic;
use numlang::explain::{get_explanation, print_explanation, EXPLANATIONS};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use std::process::Command;

#[test]
fn test_all_error_codes_e001_to_e020_have_explanations() {
    assert_eq!(EXPLANATIONS.len(), 20);
    for i in 1..=20 {
        let code = format!("E{:03}", i);
        let exp = get_explanation(&code);
        assert!(exp.is_some(), "Error code {} should have an explanation", code);
        let exp = exp.unwrap();
        assert!(!exp.title.is_empty());
        assert!(!exp.description.is_empty());
        assert!(!exp.bad_example.is_empty());
        assert!(!exp.good_example.is_empty());
    }
}

#[test]
fn test_explain_printing_and_lookup() {
    assert!(print_explanation("E001"));
    assert!(print_explanation("e001"));
    assert!(print_explanation("1"));
    assert!(print_explanation("E020"));
    assert!(!print_explanation("E999"));
    assert!(!print_explanation("XYZ"));
}

#[test]
fn test_cli_explain_flag_and_subcommand() {
    let target_dir = std::env::var("CARGO_BIN_EXE_numlang")
        .unwrap_or_else(|_| "target/debug/numlang.exe".to_string());

    // Test --explain E001
    let out = Command::new(&target_dir)
        .arg("--explain")
        .arg("E001")
        .output()
        .expect("Failed to run numlang --explain E001");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("NumLang Error Code: E001 (Type Mismatch)"));
    assert!(stdout.contains("Erroneous Example:"));
    assert!(stdout.contains("Corrected Example:"));

    // Test explain subcommand
    let out = Command::new(&target_dir)
        .arg("explain")
        .arg("E010")
        .output()
        .expect("Failed to run numlang explain E010");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("NumLang Error Code: E010 (Unknown Type Name)"));

    // Test unknown code exits 1
    let out = Command::new(&target_dir)
        .arg("--explain")
        .arg("E999")
        .output()
        .expect("Failed to run numlang --explain E999");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown error code 'E999'"));
}

fn render_diagnostic(diag: &CompilerDiagnostic) -> String {
    format!("{:?}", miette::Report::new(diag.clone()))
}

#[test]
fn test_parse_error_let_hint() {
    let filename = "test_let.nl";
    let source = "fn main() -> i64 {\n    let = 42;\n    return 0;\n}";
    let tokens = tokenize(source).unwrap();
    let err = parse(&tokens).unwrap_err();
    let diag = CompilerDiagnostic::from_parse_error(err, filename, source);
    let rendered = render_diagnostic(&diag);

    assert!(rendered.contains("test_let.nl"));
    assert!(rendered.contains("2") && (rendered.contains("│") || rendered.contains("|")));
    assert!(rendered.contains("Did you mean: let <name>: <type> = <value>?"));
}

#[test]
fn test_parse_error_missing_arrow_hint() {
    let filename = "test_fn.nl";
    let source = "fn calculate(x: i64) i64 {\n    return x * 2;\n}";
    let tokens = tokenize(source).unwrap();
    let err = parse(&tokens).unwrap_err();
    let diag = CompilerDiagnostic::from_parse_error(err, filename, source);
    let rendered = render_diagnostic(&diag);

    assert!(rendered.contains("test_fn.nl"));
    assert!(rendered.contains("1") && (rendered.contains("│") || rendered.contains("|")));
    assert!(rendered.contains("Did you forget the return type arrow ->?"));
}

#[test]
fn test_parser_recovery_multiple_statements() {
    let source = "fn main() -> i64 {\n    let = 10;\n    let y: i64 = 20;\n    let = 30;\n    return y;\n}";
    let tokens = tokenize(source).unwrap();
    let mut parser = numlang::parser::Parser::new(&tokens);
    let _ = parser.parse_program();

    // Recovered and collected multiple errors
    assert!(parser.errors.len() >= 2);
}

#[test]
fn test_type_error_type_mismatch_with_secondary_label() {
    let filename = "mismatch.nl";
    let source = "fn main() -> i64 {\n    let x: i64 = 3.14;\n    return 0;\n}";
    let tokens = tokenize(source).unwrap();
    let program = parse(&tokens).unwrap();
    let err = typecheck(&program).unwrap_err();

    assert_eq!(err.error_code(), "E001");
    let diag = CompilerDiagnostic::from_type_error(err, filename, source);
    let rendered = render_diagnostic(&diag);

    assert!(rendered.contains("mismatch.nl"));
    assert!(rendered.contains("[E001]"));
    assert!(rendered.contains("--explain") && rendered.contains("E001"));
    assert!(rendered.contains("declared type `i64` specified here"));
}

#[test]
fn test_type_error_arity_mismatch_with_secondary_label() {
    let filename = "arity.nl";
    let source = "fn add(a: i64, b: i64) -> i64 {\n    return a + b;\n}\n\nfn main() -> i64 {\n    return add(10);\n}";
    let tokens = tokenize(source).unwrap();
    let program = parse(&tokens).unwrap();
    let err = typecheck(&program).unwrap_err();

    assert_eq!(err.error_code(), "E009");
    let diag = CompilerDiagnostic::from_type_error(err, filename, source);
    let rendered = render_diagnostic(&diag);

    assert!(rendered.contains("arity.nl"));
    assert!(rendered.contains("[E009]"));
    assert!(rendered.contains("--explain") && rendered.contains("E009"));
    assert!(rendered.contains("function `add` defined here"));
}

#[test]
fn test_all_type_error_variants_diagnostics_coverage() {
    let cases = vec![
        // E001: TypeMismatch
        ("fn main() -> i64 {\n    let x: i64 = 3.14;\n    return 0;\n}", "E001"),
        // E002: UndeclaredVariable
        ("fn main() -> i64 {\n    return foo + 1;\n}", "E002"),
        // E003: UndeclaredFunction
        ("fn main() -> i64 {\n    return unknown_func(1);\n}", "E003"),
        // E004: CannotMutateImmutable
        ("fn main() -> i64 {\n    let x: i64 = 1;\n    x = 2;\n    return x;\n}", "E004"),
        // E005: DuplicateDeclaration
        ("fn main() -> i64 {\n    let x: i64 = 1;\n    let x: i64 = 2;\n    return x;\n}", "E005"),
        // E006: InvalidConditionType
        ("fn main() -> i64 {\n    if 5 {\n        return 1;\n    }\n    return 0;\n}", "E006"),
        // E007: InvalidBinaryOperands
        ("fn main() -> i64 {\n    let x: i64 = 1 + 2.0;\n    return x;\n}", "E007"),
        // E008: InvalidUnaryOperand
        ("fn main() -> i64 {\n    let x: bool = -true;\n    return 0;\n}", "E008"),
        // E009: ArityMismatch
        ("fn f(a: i64) -> i64 {\n    return a;\n}\n\nfn main() -> i64 {\n    return f(1, 2);\n}", "E009"),
        // E010: UnknownType
        ("fn main() -> i64 {\n    let x: invalid_type = 0;\n    return 0;\n}", "E010"),
        // E011: InvalidReturn
        ("fn main() -> i64 {\n    return 3.14;\n}", "E011"),
        // E012: CannotIndexNonArray
        ("fn main() -> i64 {\n    let x: i64 = 1;\n    return x[0];\n}", "E012"),
        // E013: InvalidIndexType
        ("fn main() -> i64 {\n    let arr: [i64; 2] = [1, 2];\n    return arr[3.14];\n}", "E013"),
        // E014: EmptyArrayLiteral
        ("fn main() -> i64 {\n    let arr = [];\n    return 0;\n}", "E014"),
        // E015: IndexOutOfBounds
        ("fn main() -> i64 {\n    let arr: [i64; 2] = [1, 2];\n    return arr[5];\n}", "E015"),
        // E016: ArrayElementMismatch
        ("fn main() -> i64 {\n    let arr = [1, 2.5];\n    return 0;\n}", "E016"),
        // E017: BreakOutsideLoop
        ("fn main() -> i64 {\n    break;\n    return 0;\n}", "E017"),
        // E018: ContinueOutsideLoop
        ("fn main() -> i64 {\n    continue;\n    return 0;\n}", "E018"),
        // E019: Struct error
        ("struct Point { x: i64 }\n\nfn main() -> i64 {\n    let p = Point { x: 1 };\n    return p.y;\n}", "E019"),
        // E020: Match error
        ("fn main() -> i64 {\n    let x: i64 = 1;\n    return match x {\n        0 => 10,\n    };\n}", "E020"),
    ];

    let filename = "coverage.nl";
    for (src, expected_code) in cases {
        let tokens = tokenize(src).expect("tokenization should succeed");
        let program = parse(&tokens).expect("parsing should succeed");
        let err = typecheck(&program).expect_err(&format!("typecheck should fail for code {}", expected_code));

        assert_eq!(err.error_code(), expected_code);
        assert!(!err.span().is_empty(), "Span must be non-zero for code {}", expected_code);

        let diag = CompilerDiagnostic::from_type_error(err, filename, src);
        match &diag {
            CompilerDiagnostic::TypeError { help, .. } => {
                assert!(help.contains(&format!("see 'numlang --explain {}'", expected_code)));
            }
            _ => panic!("Expected TypeError"),
        }

        let rendered = render_diagnostic(&diag);
        assert!(!rendered.is_empty());
        assert!(rendered.contains(filename), "Diagnostic output must contain filename");
        assert!(rendered.contains("│") || rendered.contains("|"), "Diagnostic output must contain source snippet");
        assert!(rendered.contains(&format!("[{}]", expected_code)), "Diagnostic must contain error code");
        assert!(rendered.contains("--explain") && rendered.contains(expected_code), "Diagnostic must contain explain reference");
    }
}
