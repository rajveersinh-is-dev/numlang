use std::fs;
use std::process::Command;

fn run_numlang_code(src: &str, test_name: &str) -> i32 {
    let test_dir = std::env::temp_dir().join("numlang_dynamic_sroa_tests");
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
fn test_dynamic_indexed_array_mutation_and_read() {
    let src = r#"
        fn main() -> i64 {
            let mut arr: [i64; 10] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            let mut i: i64 = 0;
            while i < 10 {
                arr[i] = (i + 1) * 3;
                i = i + 1;
            }
            let mut sum: i64 = 0;
            let mut j: i64 = 0;
            while j < 10 {
                sum = sum + arr[j];
                j = j + 1;
            }
            return sum;
        }
    "#;
    let code = run_numlang_code(src, "dynamic_sroa_mut_read");
    // sum = 3 + 6 + 9 + 12 + 15 + 18 + 21 + 24 + 27 + 30 = 165
    assert_eq!(code, 165);
}

#[test]
fn test_mixed_static_and_dynamic_arrays() {
    let src = r#"
        fn main() -> i64 {
            // arr_static should be promoted to registers (SROA)
            let mut arr_static: [i64; 3] = [100, 200, 300];
            arr_static[0] = arr_static[0] + 5;
            arr_static[2] = arr_static[2] + 10;

            // arr_dynamic should stay on the stack slot (contiguous memory)
            let mut arr_dynamic: [i64; 4] = [1, 2, 3, 4];
            let mut idx: i64 = 2;
            arr_dynamic[idx] = 99;

            return (arr_static[0] + arr_static[2]) - arr_dynamic[idx];
        }
    "#;
    let code = run_numlang_code(src, "mixed_static_dynamic");
    // arr_static[0] = 105, arr_static[2] = 310 => 415
    // arr_dynamic[2] = 99 => 415 - 99 = 316
    assert_eq!(code, 316);
}

#[test]
fn test_nqueens_diagonal_conflict_kernel() {
    let src = r#"
        fn main() -> i64 {
            let mut queens: [i64; 8] = [0, 4, 7, 5, 2, 6, 1, 3];
            // Queen placement [0, 4, 7, 5, 2, 6, 1, 3] is a known valid 8-queens solution!
            let mut r: i64 = 1;
            let mut all_valid: i64 = 1;
            while r < 8 {
                let col: i64 = queens[r];
                let mut i: i64 = 0;
                while i < r {
                    let q_col: i64 = queens[i];
                    if q_col == col {
                        all_valid = 0;
                    }
                    let diff_col: i64 = col - q_col;
                    let diff_row: i64 = r - i;
                    let mut abs_diff: i64 = diff_col;
                    if abs_diff < 0 {
                        abs_diff = -abs_diff;
                    }
                    if abs_diff == diff_row {
                        all_valid = 0;
                    }
                    i = i + 1;
                }
                r = r + 1;
            }
            return all_valid;
        }
    "#;
    let code = run_numlang_code(src, "nqueens_diagonal_kernel");
    assert_eq!(code, 1);
}
