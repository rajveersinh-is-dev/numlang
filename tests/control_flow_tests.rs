use std::fs;
use std::process::Command;

fn run_numlang_src(test_name: &str, src: &str) -> i32 {
    let test_dir = std::env::temp_dir().join(format!("numlang_test_{}", test_name));
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join(format!("{}.nl", test_name));
    let exe_file = test_dir.join(format!("{}.exe", test_name));

    fs::write(&src_file, src).unwrap();

    let compile_output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("-o")
        .arg(&exe_file)
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang compiler");

    assert!(
        compile_output.status.success(),
        "Compilation failed for {}:\nSTDOUT: {}\nSTDERR: {}",
        test_name,
        String::from_utf8_lossy(&compile_output.stdout),
        String::from_utf8_lossy(&compile_output.stderr)
    );
    assert!(exe_file.exists());

    let run_output = Command::new(&exe_file)
        .output()
        .expect("Failed to execute compiled numlang binary");

    run_output.status.code().unwrap_or(-1)
}

#[test]
fn test_for_loop_exclusive_sum() {
    let src = r#"
        fn main() -> i64 {
            let mut sum: i64 = 0;
            for i in 0..10 {
                sum = sum + i;
            }
            return sum;
        }
    "#;
    assert_eq!(run_numlang_src("for_exclusive", src), 45);
}

#[test]
fn test_for_loop_inclusive_sum() {
    let src = r#"
        fn main() -> i64 {
            let mut sum: i64 = 0;
            for i in 1..=10 {
                sum = sum + i;
            }
            return sum;
        }
    "#;
    assert_eq!(run_numlang_src("for_inclusive", src), 55);
}

#[test]
fn test_nested_for_loops() {
    let src = r#"
        fn main() -> i64 {
            let mut total: i64 = 0;
            for r in 0..4 {
                for c in 0..4 {
                    total = total + 1;
                }
            }
            return total;
        }
    "#;
    assert_eq!(run_numlang_src("nested_for", src), 16);
}

#[test]
fn test_continue_skipping_evens() {
    let src = r#"
        fn main() -> i64 {
            let mut sum: i64 = 0;
            for i in 0..10 {
                if i % 2 == 0 {
                    continue;
                }
                sum = sum + i;
            }
            return sum;
        }
    "#;
    assert_eq!(run_numlang_src("continue_odds", src), 25);
}

#[test]
fn test_loop_infinite_break() {
    let src = r#"
        fn main() -> i64 {
            let mut count: i64 = 0;
            loop {
                count = count + 1;
                if count == 7 {
                    break;
                }
            }
            return count;
        }
    "#;
    assert_eq!(run_numlang_src("loop_break", src), 7);
}

#[test]
fn test_continue_outside_loop_rejected() {
    let test_dir = std::env::temp_dir().join("numlang_test_continue_outside");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("continue_outside.nl");
    let exe_file = test_dir.join("continue_outside.exe");

    let src = r#"
        fn main() -> i64 {
            continue;
            return 0;
        }
    "#;
    fs::write(&src_file, src).unwrap();

    let compile_output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("-o")
        .arg(&exe_file)
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang compiler");

    assert!(
        !compile_output.status.success(),
        "Compilation should have failed for continue outside loop"
    );
}
