use std::fs;
use std::process::Command;

use numlang::codegen::compile_supercompiled_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn get_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    lower_program(&typed)
}

fn compile_and_run_supercompiled(src: &str, test_name: &str) -> i32 {
    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();

    let obj_bytes = compile_supercompiled_to_obj(&typed).expect("Supercompiled codegen failed");

    let test_dir = std::env::temp_dir().join("numlang_phase6_tests");
    let _ = fs::create_dir_all(&test_dir);
    let obj_path = test_dir.join(format!("{}.obj", test_name));
    let exe_path = test_dir.join(format!("{}.exe", test_name));
    fs::write(&obj_path, obj_bytes).unwrap();

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to run binary");
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    output.status.code().unwrap_or(-1)
}

fn assert_buffer_eliminated(mir: &MirProgram, buf_name: &str) {
    for func in &mir.functions {
        for b in &func.blocks {
            for stmt in &b.statements {
                let Statement::Assign(dest, rval) = stmt;
                if dest.local == buf_name {
                    assert!(
                        !matches!(rval, Rvalue::Array(_)),
                        "Array allocation for `{}` must be eliminated",
                        buf_name
                    );
                }
            }
        }
    }
}

#[test]
fn test_deforestation_struct_construction_and_field_access() {
    let code = r#"
    struct Point {
        x: i64,
        y: i64,
    }

    fn distance_squared(a: i64, b: i64) -> i64 {
        let p: Point = Point { x: a, y: b };
        return p.x * p.x + p.y * p.y;
    }
    "#;

    let mut mir_program = get_mir(code);
    let stats = supercompile_mir_program(&mut mir_program);

    assert_eq!(mir_program.functions.len(), 1);
    let func = &mir_program.functions[0];
    assert_eq!(func.name, "distance_squared");
    // Verify that the function was explored and retained clean blocks
    assert!(!func.blocks.is_empty());
    assert!(stats.nodes_explored >= 1);
}

#[test]
fn test_deforestation_chained_function_passing() {
    let code = r#"
    struct Pair {
        first: i64,
        second: i64,
    }

    fn make_pair(x: i64, y: i64) -> Pair {
        return Pair { first: x + 1, second: y * 2 };
    }

    fn consume_pair(p: Pair) -> i64 {
        return p.first + p.second;
    }

    fn pipeline(x: i64, y: i64) -> i64 {
        let p: Pair = make_pair(x, y);
        return consume_pair(p);
    }
    "#;

    let mut mir_program = get_mir(code);
    let stats = supercompile_mir_program(&mut mir_program);

    assert_eq!(mir_program.functions.len(), 3);
    assert!(stats.nodes_explored >= 3);
}

#[test]
fn test_array_map_fusion() {
    let code = r#"
    fn main() -> i64 {
        let mut a: [i64; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        let mut i: i64 = 0;
        while i < 8 {
            a[i] = i * 2;
            i = i + 1;
        }
        let mut b: [i64; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        let mut k: i64 = 0;
        while k < 8 {
            b[k] = a[k] + 5;
            k = k + 1;
        }
        return b[3];
    }
    "#;

    let mut mir = get_mir(code);
    supercompile_mir_program(&mut mir);
    assert_buffer_eliminated(&mir, "a");

    let exit_code = compile_and_run_supercompiled(code, "test_array_map_fusion");
    assert_eq!(exit_code, 11);
}

#[test]
fn test_fold_over_map() {
    let code = r#"
    fn main() -> i64 {
        let mut a: [i64; 10] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut i: i64 = 0;
        while i < 10 {
            a[i] = i * i;
            i = i + 1;
        }
        let mut sum: i64 = 0;
        let mut k: i64 = 0;
        while k < 10 {
            sum = sum + a[k];
            k = k + 1;
        }
        return sum;
    }
    "#;

    let mut mir = get_mir(code);
    let stats = supercompile_mir_program(&mut mir);
    assert_buffer_eliminated(&mir, "a");
    assert!(
        stats.loops_collapsed >= 1,
        "Loop should be collapsed by recurrence solver"
    );

    let exit_code = compile_and_run_supercompiled(code, "test_fold_over_map");
    assert_eq!(exit_code, 285);
}

#[test]
fn test_filter_map_fusion() {
    let code = r#"
    fn main() -> i64 {
        let src: [i64; 6] = [1, 5, 2, 8, 3, 10];
        let mut tmp: [i64; 6] = [0, 0, 0, 0, 0, 0];
        let mut j: i64 = 0;
        let mut i: i64 = 0;
        while i < 6 {
            if src[i] > 4 {
                tmp[j] = src[i];
                j = j + 1;
            }
            i = i + 1;
        }

        let mut dst: [i64; 6] = [0, 0, 0, 0, 0, 0];
        let mut k: i64 = 0;
        while k < j {
            dst[k] = tmp[k] * 2;
            k = k + 1;
        }

        return dst[0] + dst[1] + dst[2];
    }
    "#;

    let mut mir = get_mir(code);
    supercompile_mir_program(&mut mir);
    assert_buffer_eliminated(&mir, "tmp");

    let exit_code = compile_and_run_supercompiled(code, "test_filter_map_fusion");
    assert_eq!(exit_code, 46);
}

#[test]
fn test_accumulator_fold_closed_form() {
    let code = r#"
    fn main() -> i64 {
        let mut acc: i64 = 0;
        let mut i: i64 = 0;
        while i < 100 {
            acc = acc + i * i;
            i = i + 1;
        }
        return acc;
    }
    "#;

    let mut mir = get_mir(code);
    let stats = supercompile_mir_program(&mut mir);
    assert!(
        stats.loops_collapsed >= 1,
        "Loop should be collapsed to closed form"
    );

    let exit_code = compile_and_run_supercompiled(code, "test_accumulator_fold_closed_form");
    assert_eq!(exit_code, 328350);
}

#[test]
fn test_accumulator_fold_cubic_closed_form() {
    let code = r#"
    fn main() -> i64 {
        let mut acc: i64 = 0;
        let mut i: i64 = 0;
        while i < 10 {
            acc = acc + i * i * i;
            i = i + 1;
        }
        return acc;
    }
    "#;

    let mut mir = get_mir(code);
    let stats = supercompile_mir_program(&mut mir);
    assert!(stats.loops_collapsed >= 1, "Cubic loop should be collapsed");

    let exit_code = compile_and_run_supercompiled(code, "test_accumulator_fold_cubic");
    assert_eq!(exit_code, 2025);
}
