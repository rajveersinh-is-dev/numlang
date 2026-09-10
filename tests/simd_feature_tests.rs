use numlang::codegen::compile_to_obj;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use std::fs;
use std::process::Command;

#[test]
fn test_host_cpu_feature_compilation() {
    let src = r#"
        fn compute(a: f64, b: f64) -> f64 {
            let mut x: f64 = a * b + 1.0;
            return x;
        }

        fn main() -> i64 {
            let res: f64 = compute(2.5, 4.0);
            if res > 10.0 {
                return 42;
            } else {
                return 0;
            }
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();
    let obj_bytes = compile_to_obj(&typed).unwrap();

    assert!(!obj_bytes.is_empty());
    // Verify Windows COFF header
    assert_eq!(obj_bytes[0], 0x64);
    assert_eq!(obj_bytes[1], 0x86);
}

#[test]
fn test_fma_vector_dot_product_execution() {
    let test_dir = std::env::temp_dir().join("numlang_test_simd_fma_dot");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("fma_dot.nl");
    let exe_file = test_dir.join("fma_dot.exe");

    let src = r#"
        fn main() -> i64 {
            let a: [f64; 8] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
            let b: [f64; 8] = [0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5];
            let d: f64 = dot(a, b);
            // 1*0.5 + 2*1.5 + 3*2.5 + 4*3.5 + 5*4.5 + 6*5.5 + 7*6.5 + 8*7.5
            // = 0.5 + 3.0 + 7.5 + 14.0 + 22.5 + 33.0 + 45.5 + 60.0 = 186.0
            if d == 186.0 {
                return 42;
            } else {
                return 0;
            }
        }
    "#;
    fs::write(&src_file, src).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("build")
        .arg(&src_file)
        .arg("-o")
        .arg(&exe_file)
        .output()
        .expect("Failed to build numlang FMA dot binary");

    assert!(output.status.success());
    assert!(exe_file.exists());

    let run_output = Command::new(&exe_file).output().expect("Failed to run binary");
    assert_eq!(run_output.status.code(), Some(42));
}

#[test]
fn test_unrolled_vector_sum_and_add() {
    let test_dir = std::env::temp_dir().join("numlang_test_simd_unroll");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("unroll.nl");
    let exe_file = test_dir.join("unroll.exe");

    let src = r#"
        fn main() -> i64 {
            let a: [i64; 8] = [10, 20, 30, 40, 50, 60, 70, 80];
            let b: [i64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
            let c: [i64; 8] = vec_add(a, b);
            let s: i64 = sum(c);
            // sum([11, 22, 33, 44, 55, 66, 77, 88]) = 396
            return s % 256; // 396 % 256 = 140
        }
    "#;
    fs::write(&src_file, src).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("build")
        .arg(&src_file)
        .arg("-o")
        .arg(&exe_file)
        .output()
        .expect("Failed to build unrolled vector binary");

    assert!(output.status.success());
    assert!(exe_file.exists());

    let run_output = Command::new(&exe_file).output().expect("Failed to run binary");
    assert_eq!(run_output.status.code(), Some(140));
}
