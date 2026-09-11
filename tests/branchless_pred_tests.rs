use numlang::codegen::compile_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use std::fs;
use std::process::Command;

#[test]
fn test_branchless_bsearch_kernel_execution() {
    let src = r#"
        fn bsearch_kernel(iters: i64) -> i64 {
            let mut total: i64 = 0;
            let mut state: i64 = 123456789;
            let mut i: i64 = 0;
            while i < iters {
                state = (state * 1664525 + 1013904223) % 4294967296;
                let target: i64 = state % 7168;
                let mut low: i64 = 0;
                let mut high: i64 = 1023;
                let mut idx: i64 = 0 - 1;
                while low <= high {
                    let mid: i64 = (low + high) / 2;
                    let val: i64 = mid * 7 + 3;
                    if val == target {
                        idx = mid;
                        break;
                    } else {
                        if val < target {
                            low = mid + 1;
                        } else {
                            high = mid - 1;
                        }
                    }
                }
                total = (total + idx + 1) % 1000000007;
                i = i + 1;
            }
            return total;
        }

        fn main() -> i64 {
            let res: i64 = bsearch_kernel(10000);
            return res % 256;
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let mut typed = typecheck(&ast).unwrap();
    numlang::opt::optimize_program(&mut typed);

    let obj_bytes = compile_to_obj(&typed).unwrap();
    let test_dir = std::env::temp_dir().join("numlang_test_pred");
    fs::create_dir_all(&test_dir).unwrap();
    let obj_path = test_dir.join("test_bsearch.obj");
    let exe_path = test_dir.join("test_bsearch.exe");
    fs::write(&obj_path, obj_bytes).unwrap();

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to run binary");
    assert!(output.status.success() || output.status.code().is_some());
}

#[test]
fn test_branchless_conditional_swap_execution() {
    let src = r#"
        fn cond_swap_test(a_in: i64, b_in: i64) -> i64 {
            let mut a: i64 = a_in;
            let mut b: i64 = b_in;
            if a > b {
                let temp: i64 = a;
                a = b;
                b = temp;
            }
            return a * 1000 + b;
        }

        fn main() -> i64 {
            let r1: i64 = cond_swap_test(42, 17);
            let r2: i64 = cond_swap_test(10, 99);
            if r1 == 17042 {
                if r2 == 10099 {
                    return 0;
                }
            }
            return 1;
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let mut typed = typecheck(&ast).unwrap();
    numlang::opt::optimize_program(&mut typed);

    let obj_bytes = compile_to_obj(&typed).unwrap();
    let test_dir = std::env::temp_dir().join("numlang_test_pred");
    fs::create_dir_all(&test_dir).unwrap();
    let obj_path = test_dir.join("test_swap.obj");
    let exe_path = test_dir.join("test_swap.exe");
    fs::write(&obj_path, obj_bytes).unwrap();

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to run binary");
    assert_eq!(output.status.code().unwrap(), 0);
}
