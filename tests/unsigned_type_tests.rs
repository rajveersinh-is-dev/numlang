use std::fs;
use std::process::Command;

fn run_numlang_code(code: &str) -> (Option<i32>, String, String) {
    let id = format!(
        "{}_{:?}_{}",
        std::process::id(),
        std::thread::current().id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
    .replace(['(', ')', ' '], "_");
    let test_dir = std::env::temp_dir().join(format!("numlang_unsigned_{}", id));
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg(&src_file)
        .output()
        .expect("Failed to run numlang program");

    let _ = fs::remove_dir_all(&test_dir);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.code(), stdout, stderr)
}

#[test]
fn test_unsigned_types_and_suffixed_literals() {
    let code = r#"
fn compute_u64(x: u64, y: u64) -> u64 {
    let a: u64 = 100u64;
    let b: u64 = 25u64;
    return x + y + a + b;
}
fn main() -> i64 {
    let res: u64 = compute_u64(10u64, 5u64);
    let u_idx: usize = 3usize;
    let arr: [i64; 5] = [10, 20, 30, 40, 50];
    let val: i64 = arr[u_idx];
    if res == 140 {
        return val;
    }
    return 0;
}
"#;
    let (code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(code, Some(40));
}

#[test]
fn test_unsigned_logical_shift() {
    let code = r#"
fn main() -> i64 {
    let a: u64 = 9223372036854775808u64;
    let shifted: u64 = a >> 63;
    if shifted == 1 {
        return 77;
    }
    return 0;
}
"#;
    let (code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(code, Some(77));
}

#[test]
fn test_unsigned_division_and_modulo() {
    let code = r#"
fn main() -> i64 {
    let x: u64 = 1000u64;
    let y: u64 = 30u64;
    let q: u64 = x / y;
    let r: u64 = x % y;
    if q == 33 {
        if r == 10 {
            return 43;
        }
    }
    return 0;
}
"#;
    let (code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(code, Some(43));
}

#[test]
fn test_structured_panic_bounds_check() {
    let code = r#"
fn main() -> i64 {
    let a: [i64; 3] = [10, 20, 30];
    let bad_idx: i64 = 99;
    return a[bad_idx];
}
"#;
    let (exit_code, _stdout, stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(101), "Bounds check must exit with code 101");
    assert!(
        stderr.contains("panic: array index out of bounds"),
        "Bounds check must print diagnostic message to stderr, got: '{}'",
        stderr
    );
}

#[test]
fn test_signed_i8_and_i16_types() {
    let code = r#"
fn main() -> i64 {
    let x: i8 = -128i8;
    let y: i8 = 127i8;
    let z: i16 = 32767i16;
    let w: i16 = -32768i16;
    if x == -128i8 {
        if y == 127i8 {
            if z == 32767i16 {
                if w == -32768i16 {
                    return 55;
                }
            }
        }
    }
    return 0;
}
"#;
    let (code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(code, Some(55));
}

#[test]
fn test_signed_i8_i16_overflow_wrapping() {
    let code = r#"
fn main() -> i64 {
    let max_i8: i8 = 127i8;
    let wrapped_i8: i8 = max_i8 + 1i8;
    let max_i16: i16 = 32767i16;
    let wrapped_i16: i16 = max_i16 + 1i16;
    if wrapped_i8 == -128i8 {
        if wrapped_i16 == -32768i16 {
            return 88;
        }
    }
    return 0;
}
"#;
    let (code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(code, Some(88));
}

#[test]
fn test_i8_i16_arithmetic_and_print() {
    let code = r#"
fn main() -> i64 {
    let a: i8 = 10i8;
    let b: i8 = -20i8;
    let c: i8 = a + b;
    let d: i16 = 1000i16;
    let e: i16 = -500i16;
    let f: i16 = d / e;
    println(c);
    println(f);
    return 0;
}
"#;
    let (code, stdout, _stderr) = run_numlang_code(code);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("-10"));
    assert!(stdout.contains("-2"));
}

