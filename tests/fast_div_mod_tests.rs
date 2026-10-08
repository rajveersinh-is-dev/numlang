use std::fs;
use std::process::Command;

fn run_numlang_code(src: &str, test_name: &str) -> i32 {
    let test_dir = std::env::temp_dir().join("numlang_div_mod_tests");
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
fn test_nonneg_power_of_two_mod_and_div() {
    let src = r#"
        fn main() -> i64 {
            let mut state: i64 = 123456789;
            let mut i: i64 = 0;
            let mut sum: i64 = 0;
            while i < 1000 {
                state = (state * 1664525 + 1013904223) % 4294967296;
                let half: i64 = state / 2;
                sum = (sum + (half % 256)) % 256;
                i = i + 1;
            }
            return sum;
        }
    "#;
    let code = run_numlang_code(src, "nonneg_pow2");

    // Compute expected result directly in Rust
    let mut state = 123456789i64;
    let mut sum = 0i64;
    for _ in 0..1000 {
        state = (state * 1664525 + 1013904223) % 4294967296;
        let half = state / 2;
        sum = (sum + (half % 256)) % 256;
    }
    assert_eq!(code, sum as i32);
}

#[test]
fn test_nonneg_constant_modulo_prime() {
    let src = r#"
        fn main() -> i64 {
            let mut acc: i64 = 0;
            let mut i: i64 = 0;
            while i < 500 {
                let term: i64 = i * 17 + 13;
                acc = (acc * 3 + term) % 1000000007;
                i = i + 1;
            }
            return acc % 256;
        }
    "#;
    let code = run_numlang_code(src, "nonneg_const_prime_mod");

    let mut acc = 0i64;
    for i in 0..500i64 {
        let term = i * 17 + 13;
        acc = (acc * 3 + term) % 1000000007;
    }
    assert_eq!(code, (acc % 256) as i32);
}

#[test]
fn test_signed_negative_modulo_and_div() {
    let src = r#"
        fn main() -> i64 {
            let neg_val: i64 = -12345;
            let rem_pow2: i64 = neg_val % 16;     // -12345 % 16 = -9
            let div_pow2: i64 = neg_val / 16;     // -12345 / 16 = -771
            let rem_const: i64 = neg_val % 7;     // -12345 % 7 = -4
            let div_const: i64 = neg_val / 7;     // -12345 / 7 = -1763

            if rem_pow2 != -9 { return 1; }
            if div_pow2 != -771 { return 2; }
            if rem_const != -4 { return 3; }
            if div_const != -1763 { return 4; }

            return 42;
        }
    "#;
    let code = run_numlang_code(src, "signed_neg_div_mod");
    assert_eq!(code, 42);
}

#[test]
fn test_horner_constant_mod_7() {
    let src = r#"
        fn main() -> i64 {
            let mut i: i64 = 0;
            let mut total: i64 = 0;
            while i < 100 {
                let rem: i64 = i % 7;
                total = total + rem;
                i = i + 1;
            }
            return total % 256;
        }
    "#;
    let code = run_numlang_code(src, "horner_mod_7");
    let mut total = 0i64;
    for i in 0..100i64 {
        total += i % 7;
    }
    assert_eq!(code, (total % 256) as i32);
}
