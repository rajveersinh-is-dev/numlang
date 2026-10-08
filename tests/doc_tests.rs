use numlang::doc::generate_doc;
use numlang::parser::parse;
use numlang::token::tokenize;
use std::fs;
use std::process::Command;

#[test]
fn test_doc_comments_on_functions_and_structs() {
    let source = r#"
/// Computes the nth triangular number T(n) = n*(n+1)/2.
/// 
/// # Arguments
/// * `n` - must be non-negative
fn triangular(n: i64) -> i64 {
    return n * (n + 1) / 2;
}

fn undocumented(x: f64) -> f64 {
    return x * 2.0;
}

/// Represents a 2D Cartesian point.
struct Point {
    x: f64,
    y: f64,
}
"#;

    let tokens = tokenize(source).expect("Tokenization failed");
    let program = parse(&tokens).expect("Parsing failed");
    let doc = generate_doc(&program);

    // 1. Doc comment on function appears
    assert!(
        doc.contains("triangular"),
        "Missing function triangular in doc"
    );
    assert!(
        doc.contains("Computes the nth triangular number"),
        "Missing doc comment text in doc"
    );
    assert!(
        doc.contains("# Arguments"),
        "Missing doc comment sub-header in doc"
    );

    // 2. Function without doc comment still appears
    assert!(
        doc.contains("undocumented"),
        "Missing undocumented function in doc"
    );
    assert!(
        doc.contains("_No description provided._"),
        "Missing placeholder for undocumented item"
    );

    // 3. Struct and fields table appear
    assert!(doc.contains("struct Point"), "Missing struct Point in doc");
    assert!(
        doc.contains("Represents a 2D Cartesian point."),
        "Missing struct doc comment"
    );
    assert!(
        doc.contains("| Field | Type |"),
        "Missing fields table header"
    );
    assert!(doc.contains("| `x` | `f64` |"), "Missing field x row");
    assert!(doc.contains("| `y` | `f64` |"), "Missing field y row");
}

#[test]
fn test_doc_cli_stdout_and_file_output() {
    let id = format!(
        "{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let test_dir = std::env::temp_dir().join(format!("numlang_test_doc_{}", id));
    fs::create_dir_all(&test_dir).unwrap();

    let src_file = test_dir.join("math.nl");
    let code = r#"
/// Adds two numbers together.
fn add(a: i64, b: i64) -> i64 {
    return a + b;
}
"#;
    fs::write(&src_file, code).unwrap();

    // Test stdout
    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("doc")
        .arg(&src_file)
        .output()
        .expect("Failed to run numlang doc");
    assert!(output.status.success());
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    assert!(stdout_str.contains("# API Reference"));
    assert!(stdout_str.contains("Adds two numbers together."));

    // Test --output file
    let doc_file = test_dir.join("DOCS.md");
    let output2 = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("doc")
        .arg(&src_file)
        .arg("--output")
        .arg(&doc_file)
        .output()
        .expect("Failed to run numlang doc --output");
    assert!(output2.status.success());
    assert!(doc_file.exists());
    let file_content = fs::read_to_string(&doc_file).unwrap();
    assert_eq!(file_content, stdout_str);

    let _ = fs::remove_dir_all(&test_dir);
}
