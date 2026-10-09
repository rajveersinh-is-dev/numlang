use std::fs;
use std::process::Command;

use numlang::codegen::compile_supercompiled_to_obj_with_mode;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::mrsc::{MinCodeSizeObjective, MultiResultEngine, ParetoObjective};
use numlang::mir::supercompiler::{supercompile_mir_program_with_mode, SupercompileMode};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn get_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).expect("Tokenize failed");
    let program = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&program).expect("Typecheck failed");
    lower_program(&typed)
}

fn compile_and_run_mode(
    src: &str,
    test_name: &str,
    mode: SupercompileMode,
    objective: &str,
) -> i32 {
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");

    let obj_bytes = compile_supercompiled_to_obj_with_mode(&typed, mode, objective)
        .expect("Codegen with mode failed");

    let test_dir = std::env::temp_dir().join(format!("numlang_phase18_{}", test_name));
    let _ = fs::create_dir_all(&test_dir);
    let obj_path = test_dir.join(format!("{}.obj", test_name));
    let exe_path = test_dir.join(format!("{}.exe", test_name));
    fs::write(&obj_path, obj_bytes).expect("Write obj failed");

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to run binary");
    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_dir(&test_dir);
    output.status.code().unwrap_or(-1)
}

fn expected_exit(code: i32) -> i32 {
    #[cfg(target_os = "windows")]
    {
        code
    }
    #[cfg(not(target_os = "windows"))]
    {
        code.rem_euclid(256)
    }
}

#[test]
fn test_distillation_nested_tree_inversion() {
    let code = r#"
    enum Tree {
        Leaf(i64),
        Node(Box<Tree>, Box<Tree>),
    }

    fn invert(t: Tree) -> Tree {
        return match t {
            Leaf(v) => Leaf(v),
            Node(l, r) => Node(box(invert(deref(r))), box(invert(deref(l)))),
        };
    }

    fn sum_tree(t: Tree) -> i64 {
        return match t {
            Leaf(v) => v,
            Node(l, r) => sum_tree(deref(l)) + sum_tree(deref(r)),
        };
    }

    fn main() -> i64 {
        let t: Tree = Node(box(Leaf(10)), box(Leaf(20)));
        let inv1: Tree = invert(t);
        let inv2: Tree = invert(inv1);
        return sum_tree(inv2);
    }
    "#;

    let res = compile_and_run_mode(
        code,
        "test_distillation_nested_tree_inversion",
        SupercompileMode::Distill,
        "size",
    );
    assert_eq!(res, 30, "Double invert of [10, 20] must produce sum 30");

    // Verify structural distillation folding on process tree
    let mut mir_program = get_mir(code);
    let stats =
        supercompile_mir_program_with_mode(&mut mir_program, SupercompileMode::Distill, "size");
    assert!(
        stats.nodes_explored > 0,
        "Distillation must explore process tree nodes"
    );
}

#[test]
fn test_distillation_double_zip() {
    // zipWith (+) (zipWith (*) xs ys) zs
    let code = r#"
    fn double_zip(xs: [i64; 4], ys: [i64; 4], zs: [i64; 4]) -> i64 {
        let mut temp: [i64; 4] = [0, 0, 0, 0];
        for i in 0..4 {
            temp[i] = xs[i] * ys[i];
        }
        let mut s: i64 = 0;
        for i in 0..4 {
            s = s + temp[i] + zs[i];
        }
        return s;
    }

    fn main() -> i64 {
        let xs: [i64; 4] = [1, 2, 3, 4];
        let ys: [i64; 4] = [10, 20, 30, 40];
        let zs: [i64; 4] = [100, 200, 300, 400];
        return double_zip(xs, ys, zs);
    }
    "#;

    let res = compile_and_run_mode(
        code,
        "test_distillation_double_zip",
        SupercompileMode::Distill,
        "size",
    );
    // (1*10+100) + (2*20+200) + (3*30+300) + (4*40+400) = 110 + 240 + 390 + 560 = 1300
    assert_eq!(
        res,
        expected_exit(1300),
        "double_zip exit code must match 1300"
    );

    // Verify intermediate buffer elimination and fusion in MIR
    let mut mir_program = get_mir(code);
    supercompile_mir_program_with_mode(&mut mir_program, SupercompileMode::Distill, "size");

    let double_zip_func = mir_program
        .functions
        .iter()
        .find(|f| f.name == "double_zip")
        .expect("double_zip function must exist in MIR");

    let mut has_temp_array_alloc = false;
    for block in &double_zip_func.blocks {
        for stmt in &block.statements {
            if let Statement::Assign(dest, Rvalue::Array(_)) = stmt {
                if dest.local == "temp" {
                    has_temp_array_alloc = true;
                }
            }
        }
    }
    assert!(
        !has_temp_array_alloc,
        "Intermediate array `temp` must be eliminated by deforestation/distillation"
    );
}

#[test]
fn test_mrsc_optimal_code_size() {
    let builder = std::thread::Builder::new().stack_size(16 * 1024 * 1024);
    let handler = builder
        .spawn(|| {
            let code = r#"
            fn branch_tree(x: i64, depth: i64) -> i64 {
                if depth <= 0 {
                    return x;
                }
                if (x % 2) == 0 {
                    return branch_tree(x / 2, depth - 1);
                } else {
                    return branch_tree(x * 3 + 1, depth - 1);
                }
            }

            fn main() -> i64 {
                return branch_tree(7, 4);
            }
            "#;

            let mir = get_mir(code);
            let func = mir
                .functions
                .iter()
                .find(|f| f.name == "branch_tree")
                .unwrap();

            let mrsc = MultiResultEngine::new(func, &mir.functions);
            let (best_res, _best_tree, score) = mrsc.explore_and_select(&MinCodeSizeObjective);

            let mut actual_count = 0;
            for b in &best_res.blocks {
                actual_count += 1 + b.statements.len();
            }
            assert_eq!(
                score, actual_count as f64,
                "MRSC score must equal residual block/stmt count"
            );

            // Also test Pareto objective
            let (pareto_res, _pareto_tree, pareto_score) =
                mrsc.explore_and_select(&ParetoObjective);
            assert!(
                pareto_score >= score,
                "Pareto score should incorporate branch penalties"
            );
            assert!(
                !pareto_res.blocks.is_empty(),
                "Pareto residual must have blocks"
            );

            // Test execution of program compiled with MRSC
            let res = compile_and_run_mode(
                code,
                "test_mrsc_optimal_code_size",
                SupercompileMode::Mrsc,
                "size",
            );
            assert!(res >= 0, "Execution under MRSC must succeed");
        })
        .unwrap();
    handler.join().unwrap();
}

#[test]
fn test_cli_mode_flags() {
    let code = r#"
    fn main() -> i64 {
        let mut sum: i64 = 0;
        for i in 1..=5 {
            sum = sum + i;
        }
        return sum;
    }
    "#;

    let test_dir = std::env::temp_dir().join("numlang_test_cli_mode");
    let _ = fs::create_dir_all(&test_dir);
    let src_file = test_dir.join("cli_test.nl");
    fs::write(&src_file, code).unwrap();

    // 1. Test --mode classic
    let out_classic = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .args(["run", "--mode", "classic", src_file.to_str().unwrap()])
        .output()
        .expect("Run classic failed");
    assert_eq!(out_classic.status.code(), Some(15));

    // 2. Test --mode distill
    let out_distill = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .args(["run", "--mode", "distill", src_file.to_str().unwrap()])
        .output()
        .expect("Run distill failed");
    assert_eq!(out_distill.status.code(), Some(15));

    // 3. Test --mode mrsc --mrsc-objective size
    let out_mrsc = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .args([
            "run",
            "--mode",
            "mrsc",
            "--mrsc-objective",
            "size",
            src_file.to_str().unwrap(),
        ])
        .output()
        .expect("Run mrsc failed");
    assert_eq!(out_mrsc.status.code(), Some(15));

    let _ = fs::remove_dir_all(&test_dir);
}
