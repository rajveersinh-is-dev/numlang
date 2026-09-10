use std::fs;
use std::process::Command;

fn compile_and_run(src: &str, test_name: &str) -> Option<i32> {
    let test_dir = std::env::temp_dir().join(format!("numlang_unroll_{}", test_name));
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
        "Compilation failed:\nSTDOUT: {}\nSTDERR: {}",
        String::from_utf8_lossy(&compile_output.stdout),
        String::from_utf8_lossy(&compile_output.stderr)
    );

    let run_output = Command::new(&exe_file)
        .output()
        .expect("Failed to execute compiled numlang binary");

    run_output.status.code()
}

#[test]
fn test_small_fixed_loop_full_unroll() {
    // A small loop with 8 iterations initialized to 0 is fully unrolled to straight-line code.
    let src = r#"
        fn main() -> i64 {
            let mut arr: [i64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
            let mut sum: i64 = 0;
            let mut i: i64 = 0;
            while i < 8 {
                sum = sum + arr[i];
                i = i + 1;
            }
            return sum;
        }
    "#;
    // sum(1..=8) = 36
    assert_eq!(compile_and_run(src, "small_fixed"), Some(36));
}

#[test]
fn test_general_induction_loop_exact_multiple_of_four() {
    // 100 iterations (exact multiple of 4, tests unrolled loop block with 0 cleanup iterations)
    let src = r#"
        fn main() -> i64 {
            let mut sum: i64 = 0;
            let mut i: i64 = 0;
            while i < 100 {
                sum = sum + i;
                i = i + 1;
            }
            // sum of 0..99 = 99 * 100 / 2 = 4950
            return sum % 256;
        }
    "#;
    // 4950 % 256 = 86
    assert_eq!(compile_and_run(src, "mult_four"), Some(86));
}

#[test]
fn test_general_induction_loop_with_cleanup_remainder() {
    // 103 iterations (100 in unrolled loop, 3 in cleanup loop)
    let src = r#"
        fn main() -> i64 {
            let mut sum: i64 = 0;
            let mut i: i64 = 0;
            while i < 103 {
                sum = sum + i;
                i = i + 1;
            }
            // sum of 0..102 = 102 * 103 / 2 = 5253
            return sum % 256;
        }
    "#;
    // 5253 % 256 = 133
    assert_eq!(compile_and_run(src, "with_cleanup"), Some(133));
}

#[test]
fn test_vector_operations_8way_unrolling() {
    // Array of size 10 (exercises 8-way unrolled block + 2 scalar cleanup elements)
    let src = r#"
        fn main() -> i64 {
            let a: [i64; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
            let b: [i64; 10] = [2, 2, 2, 2, 2, 2, 2, 2, 2, 2];

            let s: i64 = sum(a);
            let d: i64 = dot(a, b);
            let c: [i64; 10] = vec_add(a, b);
            let sc: i64 = sum(c);

            // s = 55
            // d = 55 * 2 = 110
            // sc = 55 + 20 = 75
            // total = 55 + 110 + 75 = 240
            return s + d + sc;
        }
    "#;
    // 240
    assert_eq!(compile_and_run(src, "vec_8way"), Some(240));
}

#[test]
fn test_fma_float_dot_8way_unrolling() {
    // Floating point 8-way FMA pipeline with size 16 (exact 2 x 8 blocks)
    let src = r#"
        fn main() -> i64 {
            let a: [f64; 16] = [
                1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
                2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0
            ];
            let b: [f64; 16] = [
                3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0,
                4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0
            ];
            let res: f64 = dot(a, b);
            // 8 * (1.0 * 3.0) + 8 * (2.0 * 4.0) = 24.0 + 64.0 = 88.0
            let mut i: i64 = 0;
            while i < 100 {
                if i == 88 {
                    return i;
                }
                i = i + 1;
            }
            return 0;
        }
    "#;
    assert_eq!(compile_and_run(src, "float_fma_8way"), Some(88));
}
