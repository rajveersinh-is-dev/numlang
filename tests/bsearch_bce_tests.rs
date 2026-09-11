use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typed_ast::{TypedExpr, TypedStmt};
use numlang::typecheck::typecheck;
use std::fs;
use std::process::Command;

fn run_numlang_code(src: &str, test_name: &str) -> i32 {
    let test_dir = std::env::temp_dir().join("numlang_bsearch_tests");
    fs::create_dir_all(&test_dir).unwrap();

    let src_file = test_dir.join(format!("{}.nl", test_name));
    let exe_file = test_dir.join(format!("{}.exe", test_name));
    fs::write(&src_file, src).unwrap();

    let numlang_bin = env!("CARGO_BIN_EXE_numlang");
    let build_output = Command::new(numlang_bin)
        .arg("build")
        .arg(&src_file)
        .arg("-o")
        .arg(&exe_file)
        .output()
        .expect("Failed to build numlang binary");

    assert!(
        build_output.status.success(),
        "Build failed: {}\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let run_output = Command::new(&exe_file)
        .output()
        .expect("Failed to run binary");

    run_output.status.code().unwrap()
}

#[test]
fn test_binary_search_bce_midpoint_is_safe() {
    let src = r#"
        fn bsearch(target: i64) -> i64 {
            let mut arr: [i64; 16] = [2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32];
            let mut low: i64 = 0;
            let mut high: i64 = 15;
            let mut found: i64 = -1;
            while low <= high {
                let mid: i64 = (low + high) / 2;
                let val: i64 = arr[mid];
                if val == target {
                    found = mid;
                    break;
                } else {
                    if val < target {
                        low = mid + 1;
                    } else {
                        high = mid - 1;
                    }
                }
            }
            return found;
        }

        fn main() -> i64 {
            return bsearch(14);
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let mut typed = typecheck(&ast).unwrap();
    numlang::opt::optimize_program(&mut typed);

    // Verify that in `bsearch`, inside while loop, statement `let val: i64 = arr[mid]` has is_safe = true!
    let func = &typed.functions[0];
    let mut found_is_safe = false;
    for stmt in &func.body.stmts {
        if let TypedStmt::While { body, .. } = stmt {
            for s in &body.stmts {
                if let TypedStmt::Let { value: TypedExpr::Index { is_safe, .. }, .. } = s {
                    if *is_safe {
                        found_is_safe = true;
                    }
                }
            }
        }
    }
    assert!(found_is_safe, "Binary search midpoint index arr[mid] must have is_safe = true!");

    let code = run_numlang_code(src, "bsearch_bce");
    // Target 14 is at index 6 (arr[6] == 14)
    assert_eq!(code, 6);
}

#[test]
fn test_while_true_loop_execution() {
    let src = r#"
        fn main() -> i64 {
            let mut count: i64 = 0;
            while true {
                count = count + 1;
                if count >= 10 {
                    break;
                }
            }
            return count;
        }
    "#;
    let code = run_numlang_code(src, "while_true");
    assert_eq!(code, 10);
}
