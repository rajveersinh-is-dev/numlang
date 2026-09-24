use std::fs;
use std::process::Command;

use numlang::codegen::linker::link_executable;
use numlang::codegen::{compile_mir_to_obj, compile_supercompiled_to_obj};
use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn compile_and_run_mir_code(src: &str, test_name: &str) -> i32 {
    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();
    let mir = lower_program(&typed);

    let obj_bytes = compile_mir_to_obj(&mir).expect("MIR codegen failed");

    let test_dir = std::env::temp_dir().join("numlang_mir_codegen_tests");
    fs::create_dir_all(&test_dir).unwrap();
    let obj_path = test_dir.join(format!("{}.obj", test_name));
    let exe_path = test_dir.join(format!("{}.exe", test_name));
    fs::write(&obj_path, obj_bytes).unwrap();

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to run binary");
    output.status.code().unwrap_or(-1)
}

fn compile_and_run_supercompiled_code(src: &str, test_name: &str) -> i32 {
    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();

    let obj_bytes = compile_supercompiled_to_obj(&typed).expect("Supercompiled codegen failed");

    let test_dir = std::env::temp_dir().join("numlang_mir_codegen_tests");
    fs::create_dir_all(&test_dir).unwrap();
    let obj_path = test_dir.join(format!("{}.obj", test_name));
    let exe_path = test_dir.join(format!("{}.exe", test_name));
    fs::write(&obj_path, obj_bytes).unwrap();

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to run binary");
    output.status.code().unwrap_or(-1)
}

#[test]
fn test_mir_codegen_simple_arithmetic() {
    let src = r#"
        fn main() -> i64 {
            let a: i64 = 40;
            let b: i64 = 2;
            return a + b;
        }
    "#;
    let code = compile_and_run_mir_code(src, "mir_arith");
    assert_eq!(code, 42);
}

#[test]
fn test_mir_codegen_conditional_branch() {
    let src = r#"
        fn select(x: i64) -> i64 {
            if x > 10 {
                return 100;
            } else {
                return 200;
            }
        }

        fn main() -> i64 {
            return select(15) - select(5);
        }
    "#;
    let code = compile_and_run_mir_code(src, "mir_branch");
    // select(15) is 100, select(5) is 200 => 100 - 200 = -100 (% 256 = 156 on unsigned exit code or -100)
    assert_eq!(code as i8, -100);
}

#[test]
fn test_mir_codegen_loop_accumulation() {
    let src = r#"
        fn main() -> i64 {
            let mut s: i64 = 0;
            let mut i: i64 = 1;
            while i <= 10 {
                s = s + i;
                i = i + 1;
            }
            return s;
        }
    "#;
    // sum 1..10 = 55
    let code = compile_and_run_mir_code(src, "mir_loop");
    assert_eq!(code, 55);
}

#[test]
fn test_mir_supercompiled_direct_execution() {
    let src = r#"
        fn main() -> i64 {
            let mut s: i64 = 0;
            let mut i: i64 = 0;
            while i < 10 {
                s = s + i;
                i = i + 1;
            }
            return s;
        }
    "#;
    // sum 0..9 = 45
    let code = compile_and_run_supercompiled_code(src, "supercompiled_direct");
    assert_eq!(code, 45);
}

#[test]
fn test_mir_supercompiled_triangular_closed_form() {
    let src = r#"
        fn compute_triangular(n: i64) -> i64 {
            let mut s: i64 = 0;
            let mut i: i64 = 0;
            while i < n {
                s = s + i;
                i = i + 1;
            }
            return s;
        }

        fn main() -> i64 {
            let r: i64 = compute_triangular(10);
            return r;
        }
    "#;
    let code = compile_and_run_supercompiled_code(src, "supercompiled_triangular");
    assert_eq!(code, 45);
}

#[test]
fn test_mir_supercompiler_explicit_pipeline() {
    let src = r#"
        fn main() -> i64 {
            let mut c: i64 = 0;
            let mut j: i64 = 0;
            while j < 5 {
                c = c + 3;
                j = j + 1;
            }
            return c;
        }
    "#;
    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();
    let mut mir = lower_program(&typed);
    supercompile_mir_program(&mut mir);

    let obj_bytes = compile_mir_to_obj(&mir).expect("Explicit supercompiled MIR codegen failed");
    let test_dir = std::env::temp_dir().join("numlang_mir_codegen_tests");
    fs::create_dir_all(&test_dir).unwrap();
    let obj_path = test_dir.join("test_explicit_pipeline.obj");
    let exe_path = test_dir.join("test_explicit_pipeline.exe");
    fs::write(&obj_path, obj_bytes).unwrap();

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to run binary");
    assert_eq!(output.status.code().unwrap_or(-1), 15);
}
