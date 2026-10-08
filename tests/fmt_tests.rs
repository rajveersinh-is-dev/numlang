use numlang::fmt::format_source;
use std::fs;
use std::process::Command;

#[test]
fn test_fmt_idempotent() {
    let unformatted = r#"
fn   foo ( a : i64,b:f64 )->i64{
let mut x=10;
let y=a+x*2;
if y>0{
return y;
}else{
return 0;
}
}
"#;

    let s1 = format_source(unformatted).expect("Formatting pass 1 failed");
    let s2 = format_source(&s1).expect("Formatting pass 2 failed");
    assert_eq!(s1, s2, "Formatting must be idempotent");
}

#[test]
fn test_fmt_already_formatted_unchanged() {
    let formatted = "fn foo(a: i64, b: f64) -> i64 {\n    let mut x: i64 = 10;\n    let y: i64 = a + x * 2;\n    if y > 0 {\n        return y;\n    } else {\n        return 0;\n    }\n}\n";
    let res = format_source(formatted).expect("Formatting failed");
    assert_eq!(
        res, formatted,
        "Already formatted code must remain unchanged"
    );
}

#[test]
fn test_fmt_type_inference_for_let() {
    let code = "fn main() -> i64 {\nlet x = 42;\nlet f = 3.14;\nreturn x;\n}\n";
    let formatted = format_source(code).expect("Formatting failed");
    assert!(
        formatted.contains("let x: i64 = 42;"),
        "Expected inferred i64 type: {}",
        formatted
    );
    assert!(
        formatted.contains("let f: f64 = 3.14;"),
        "Expected inferred f64 type: {}",
        formatted
    );
}

#[test]
fn test_fmt_struct_and_functions() {
    let code = r#"
struct Point{x:f64,y:f64}
fn origin()->Point{
return Point{x:0.0,y:0.0};
}
"#;
    let formatted = format_source(code).expect("Formatting failed");
    let expected = "struct Point {\n    x: f64,\n    y: f64,\n}\n\nfn origin() -> Point {\n    return Point { x: 0.0, y: 0.0 };\n}\n";
    assert_eq!(formatted, expected);
}

#[test]
fn test_fmt_match_expression() {
    let code =
        "fn eval(x: i64) -> i64 {\nreturn match x {\n0 => 10,\n1 | 2 => 20,\n_ => 0,\n};\n}\n";
    let formatted = format_source(code).expect("Formatting failed");
    let expected = "fn eval(x: i64) -> i64 {\n    return match x {\n        0 => 10,\n        1 | 2 => 20,\n        _ => 0,\n    };\n}\n";
    assert_eq!(formatted, expected);
}

#[test]
fn test_fmt_cli_check_and_stdout() {
    let id = format!(
        "{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let test_dir = std::env::temp_dir().join(format!("numlang_test_fmt_{}", id));
    fs::create_dir_all(&test_dir).unwrap();

    let unformatted_file = test_dir.join("unformatted.nl");
    let unformatted_content = "fn main()->i64{let x=1;return x;}";
    fs::write(&unformatted_file, unformatted_content).unwrap();

    // --check on unformatted file should exit 1
    let check_status = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("fmt")
        .arg("--check")
        .arg(&unformatted_file)
        .status()
        .expect("Failed to run fmt --check");
    assert_eq!(check_status.code(), Some(1));

    // --stdout on unformatted file should print formatted output and exit 0
    let stdout_out = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("fmt")
        .arg("--stdout")
        .arg(&unformatted_file)
        .output()
        .expect("Failed to run fmt --stdout");
    assert!(stdout_out.status.success());
    let stdout_str = String::from_utf8_lossy(&stdout_out.stdout);
    assert_eq!(
        stdout_str,
        "fn main() -> i64 {\n    let x: i64 = 1;\n    return x;\n}\n"
    );

    // In-place format
    let inplace_status = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("fmt")
        .arg(&unformatted_file)
        .status()
        .expect("Failed to run fmt in-place");
    assert!(inplace_status.success());

    // --check on formatted file should now exit 0
    let check2_status = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("fmt")
        .arg("--check")
        .arg(&unformatted_file)
        .status()
        .expect("Failed to run fmt --check after format");
    assert_eq!(check2_status.code(), Some(0));

    let _ = fs::remove_dir_all(&test_dir);
}
