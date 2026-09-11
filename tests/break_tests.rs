use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::{typecheck, TypeError};
use std::fs;
use std::process::Command;

#[test]
fn rejects_break_outside_loop() {
    let source = "fn main() { break; }";
    let program = parse(&tokenize(source).unwrap()).unwrap();
    assert!(matches!(
        typecheck(&program),
        Err(TypeError::BreakOutsideLoop { .. })
    ));
}

#[test]
fn exits_only_the_innermost_loop() {
    let source = r#"
        fn main() -> i64 {
            let mut outer: i64 = 0;
            let mut total: i64 = 0;
            while outer < 4 {
                let mut inner: i64 = 0;
                while inner < 10 {
                    total = total + 1;
                    break;
                }
                outer = outer + 1;
            }
            return total;
        }
    "#;
    let test_dir = std::env::temp_dir().join("numlang_break_tests");
    fs::create_dir_all(&test_dir).unwrap();
    let source_path = test_dir.join("nested_break.nl");
    let exe_path = test_dir.join("nested_break.exe");
    fs::write(&source_path, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            exe_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(Command::new(exe_path).status().unwrap().code(), Some(4));
}
