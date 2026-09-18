use std::fs;
use std::process::Command;

fn run_numlang_io(test_name: &str, src: &str) -> (i32, String) {
    let test_dir = std::env::temp_dir().join(format!("numlang_test_io_{}", test_name));
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

    let stdout = String::from_utf8_lossy(&run_output.stdout).replace("\r\n", "\n");
    let code = run_output.status.code().unwrap_or(-1);
    (code, stdout)
}

#[test]
fn test_print_verification_example() {
    let src = r#"
        fn main() -> i64 {
            println("NumLang v1.0");
            print(42);
            print(" ");
            println(1337);
            return 0;
        }
    "#;
    let (code, stdout) = run_numlang_io("verification_example", src);
    assert_eq!(code, 0);
    assert_eq!(stdout, "NumLang v1.0\n42 1337\n");
}

#[test]
fn test_println_string_literal() {
    let src = r#"
        fn main() -> i64 {
            println("hello");
            return 0;
        }
    "#;
    let (code, stdout) = run_numlang_io("println_string", src);
    assert_eq!(code, 0);
    assert_eq!(stdout, "hello\n");
}

#[test]
fn test_print_i64_and_negatives() {
    let src = r#"
        fn main() -> i64 {
            print(0);
            print(" ");
            print(42);
            print(" ");
            print(-17);
            print(" ");
            println(-9223372036854775807);
            return 0;
        }
    "#;
    let (code, stdout) = run_numlang_io("print_i64_neg", src);
    assert_eq!(code, 0);
    assert_eq!(stdout, "0 42 -17 -9223372036854775807\n");
}

#[test]
fn test_print_floats() {
    let src = r#"
        fn main() -> i64 {
            print(3.14);
            print(" ");
            print(-0.5);
            print(" ");
            println(42.0);
            return 0;
        }
    "#;
    let (code, stdout) = run_numlang_io("print_floats", src);
    assert_eq!(code, 0);
    assert_eq!(stdout, "3.14 -0.5 42.0\n");
}

#[test]
fn test_print_booleans() {
    let src = r#"
        fn main() -> i64 {
            println(true);
            println(false);
            return 0;
        }
    "#;
    let (code, stdout) = run_numlang_io("print_booleans", src);
    assert_eq!(code, 0);
    assert_eq!(stdout, "true\nfalse\n");
}

#[test]
fn test_print_loop_concatenation() {
    let src = r#"
        fn main() -> i64 {
            for i in 0..5 {
                print(i);
                if i < 4 {
                    print(",");
                }
            }
            println();
            return 0;
        }
    "#;
    let (code, stdout) = run_numlang_io("print_loop", src);
    assert_eq!(code, 0);
    assert_eq!(stdout, "0,1,2,3,4\n");
}
