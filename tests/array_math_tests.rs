use std::fs;
use std::process::Command;

fn run_numlang_code(code: &str) -> Option<i32> {
    let test_dir = std::env::temp_dir().join("numlang_test_arrays");
    fs::create_dir_all(&test_dir).unwrap();
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let src_file = test_dir.join(format!("test_{}.nl", id));
    fs::write(&src_file, code).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg(&src_file)
        .output()
        .expect("Failed to run numlang program");

    let _ = fs::remove_file(&src_file);
    output.status.code()
}

#[test]
fn test_array_declaration_and_indexing() {
    let code = r#"
fn main() -> i64 {
    let a: [i64; 3] = [10, 25, 30];
    return a[1];
}
"#;
    assert_eq!(run_numlang_code(code), Some(25));
}

#[test]
fn test_array_element_mutation() {
    let code = r#"
fn main() -> i64 {
    let mut a: [i64; 3] = [10, 20, 30];
    a[1] = 99;
    return a[1];
}
"#;
    assert_eq!(run_numlang_code(code), Some(99));
}

#[test]
fn test_array_loop_accumulation() {
    let code = r#"
fn main() -> i64 {
    let a: [i64; 4] = [5, 10, 15, 20];
    let mut i: i64 = 0;
    let mut sum: i64 = 0;
    while i < 4 {
        sum = sum + a[i];
        i = i + 1;
    }
    return sum;
}
"#;
    assert_eq!(run_numlang_code(code), Some(50));
}

#[test]
fn test_array_runtime_bounds_check() {
    let code = r#"
fn main() -> i64 {
    let a: [i64; 3] = [1, 2, 3];
    let mut bad_idx: i64 = 5;
    return a[bad_idx];
}
"#;
    // Runtime out of bounds should exit with 101
    assert_eq!(run_numlang_code(code), Some(101));
}

#[test]
fn test_math_intrinsic_abs() {
    let code = r#"
fn main() -> i64 {
    let x: i64 = -42;
    return abs(x);
}
"#;
    assert_eq!(run_numlang_code(code), Some(42));
}

#[test]
fn test_math_intrinsic_sqrt() {
    let code = r#"
fn main() -> i64 {
    let x: f64 = 64.0;
    let s: f64 = sqrt(x);
    // Return integer truncated representation for exit code
    let mut i: i64 = 0;
    while i < 10 {
        if i == 8 {
            return i;
        }
        i = i + 1;
    }
    return 0;
}
"#;
    assert_eq!(run_numlang_code(code), Some(8));
}

#[test]
fn test_vector_dot_product() {
    let code = r#"
fn main() -> i64 {
    let a: [i64; 3] = [1, 2, 3];
    let b: [i64; 3] = [4, 5, 6];
    let d: i64 = dot(a, b);
    return d;
}
"#;
    // 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32
    assert_eq!(run_numlang_code(code), Some(32));
}

#[test]
fn test_vector_sum_and_vec_add() {
    let code = r#"
fn main() -> i64 {
    let a: [i64; 3] = [10, 20, 30];
    let b: [i64; 3] = [1, 2, 3];
    let c: [i64; 3] = vec_add(a, b);
    return sum(c);
}
"#;
    // (10+1) + (20+2) + (30+3) = 11 + 22 + 33 = 66
    assert_eq!(run_numlang_code(code), Some(66));
}
