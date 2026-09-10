use std::fs;
use std::process::Command;

fn run_numlang_code(src: &str, test_name: &str) -> i32 {
    let test_dir = std::env::temp_dir().join("numlang_sroa_tests");
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
fn test_sroa_promoted_array_indexing_and_mutation() {
    let src = r#"
        fn main() -> i64 {
            let mut arr: [i64; 4] = [10, 20, 30, 40];
            arr[0] = 55;
            arr[3] = 99;
            return arr[0] + arr[1] + arr[2] + arr[3];
        }
    "#;
    let code = run_numlang_code(src, "sroa_indexing");
    // 55 + 20 + 30 + 99 = 204
    assert_eq!(code, 204);
}

#[test]
fn test_sroa_dynamic_indexing() {
    let src = r#"
        fn main() -> i64 {
            let mut arr: [i64; 5] = [1, 2, 3, 4, 5];
            let mut i: i64 = 0;
            let mut total: i64 = 0;
            while i < 5 {
                total = total + arr[i];
                i = i + 1;
            }
            return total;
        }
    "#;
    let code = run_numlang_code(src, "sroa_dynamic_idx");
    // 1 + 2 + 3 + 4 + 5 = 15
    assert_eq!(code, 15);
}

#[test]
fn test_sroa_vector_dot_and_sum() {
    let src = r#"
        fn main() -> i64 {
            let a: [i64; 4] = [1, 2, 3, 4];
            let b: [i64; 4] = [5, 6, 7, 8];
            let d: i64 = dot(a, b);
            let s: i64 = sum(a);
            return (d + s) % 256;
        }
    "#;
    let code = run_numlang_code(src, "sroa_dot_sum");
    // dot: 1*5 + 2*6 + 3*7 + 4*8 = 5 + 12 + 21 + 32 = 70
    // sum: 1 + 2 + 3 + 4 = 10
    // d + s = 80
    assert_eq!(code, 80);
}

#[test]
fn test_sroa_vector_add_promoted() {
    let src = r#"
        fn main() -> i64 {
            let a: [i64; 4] = [1, 2, 3, 4];
            let b: [i64; 4] = [10, 20, 30, 40];
            let c: [i64; 4] = vec_add(a, b);
            return c[0] + c[1] + c[2] + c[3];
        }
    "#;
    let code = run_numlang_code(src, "sroa_vec_add");
    // c = [11, 22, 33, 44] -> sum = 110
    assert_eq!(code, 110);
}
