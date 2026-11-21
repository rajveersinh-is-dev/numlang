use std::collections::HashMap;
use std::fs;
use std::process::Command;

use numlang::codegen::compile_supercompiled_to_obj_with_mode;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::polyhedral::{
    fuse_polyhedral_stencils, AffineExpr, DependenceDistance, Inequality, IterationDomain,
};
use numlang::mir::supercompiler::{supercompile_mir_program_with_mode, SupercompileMode};
use numlang::mir::BasicBlockId;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn get_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).expect("Tokenize failed");
    let program = parse(&tokens).expect("Parse failed");
    let mut typed = typecheck(&program).expect("Typecheck failed");
    typed.desugar_for_loops();
    lower_program(&typed)
}

fn compile_and_run(src: &str, test_name: &str) -> i32 {
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");

    let obj_bytes =
        compile_supercompiled_to_obj_with_mode(&typed, SupercompileMode::Classic, "size")
            .expect("Codegen failed");

    let test_dir = std::env::temp_dir().join(format!("numlang_poly_phase23_{}", test_name));
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
fn test_polyhedral_affine_algebra_and_inequalities() {
    // 1. Test AffineExpr arithmetic
    let i = AffineExpr::variable("i");
    let j = AffineExpr::variable("j");
    let c5 = AffineExpr::constant(5);

    // 2*i + 3
    let expr1 = i.mul_const(2).add(&AffineExpr::constant(3));
    // 4*j - 1
    let expr2 = j.mul_const(4).sub(&AffineExpr::constant(1));

    // (2*i + 3) + (4*j - 1) = 2*i + 4*j + 2
    let sum_expr = expr1.add(&expr2);
    assert_eq!(sum_expr.constant, 2);
    assert_eq!(sum_expr.coefficients.get("i").copied(), Some(2));
    assert_eq!(sum_expr.coefficients.get("j").copied(), Some(4));

    // Test with c5 and Inequality
    let ineq = Inequality::ge_zero(i.add(&c5));
    assert_eq!(ineq.expr.constant, 5);
    assert_eq!(ineq.expr.coefficients.get("i").copied(), Some(1));

    // Evaluation
    let mut env = HashMap::new();
    env.insert("i".to_string(), 10);
    env.insert("j".to_string(), 3);
    assert_eq!(sum_expr.eval(&env), 2 * 10 + 4 * 3 + 2); // 20 + 12 + 2 = 34

    // Offset calculation
    let i_plus_3 = i.add(&AffineExpr::constant(3));
    let i_plus_1 = i.add(&AffineExpr::constant(1));
    assert_eq!(i_plus_3.offset_from(&i_plus_1), Some(2));
    assert_eq!(i_plus_1.offset_from(&i_plus_3), Some(-2));

    // 2. Test IterationDomain and polyhedral inequalities
    let dom1 = IterationDomain::new_1d(
        "i",
        0,
        100,
        1,
        BasicBlockId(1),
        BasicBlockId(2),
        BasicBlockId(3),
    );
    assert_eq!(dom1.trip_count(), 100);
    assert_eq!(dom1.inequalities.len(), 2);

    let dom2 = IterationDomain::new_1d(
        "j",
        0,
        100,
        1,
        BasicBlockId(4),
        BasicBlockId(5),
        BasicBlockId(6),
    );
    assert!(
        dom1.is_compatible(&dom2),
        "Domains with identical bounds must be compatible"
    );

    let dom_diff = IterationDomain::new_1d(
        "k",
        0,
        50,
        1,
        BasicBlockId(7),
        BasicBlockId(8),
        BasicBlockId(9),
    );
    assert!(
        !dom1.is_compatible(&dom_diff),
        "Domains with different bounds must not be compatible"
    );
}

#[test]
fn test_polyhedral_dependence_distance_vectors() {
    let dep_zero = DependenceDistance::new(0);
    assert!(dep_zero.is_elementwise);
    assert!(dep_zero.is_stencil);
    assert!(dep_zero.is_legal);

    let dep_forward = DependenceDistance::new(2);
    assert!(!dep_forward.is_elementwise);
    assert!(dep_forward.is_stencil);
    assert!(dep_forward.is_legal);

    let dep_anti = DependenceDistance::new(-1);
    assert!(
        !dep_anti.is_legal,
        "Negative dependence distance must be rejected as anti-causal"
    );
}

#[test]
fn test_polyhedral_single_pass_elementwise_fusion_and_buffer_elimination() {
    let code = r#"
    fn pipeline_1d(xs: [i64; 4]) -> i64 {
        let mut temp: [i64; 4] = [0, 0, 0, 0];
        for i in 0..4 {
            temp[i] = xs[i] * 3 + 7;
        }
        let mut out: i64 = 0;
        for j in 0..4 {
            out = out + temp[j] * 2;
        }
        return out;
    }

    fn main() -> i64 {
        let xs: [i64; 4] = [1, 2, 3, 4];
        return pipeline_1d(xs);
    }
    "#;

    // 1. Direct native execution verification
    let res = compile_and_run(code, "test_polyhedral_single_pass");
    // (1*3+7)*2 + (2*3+7)*2 + (3*3+7)*2 + (4*3+7)*2
    // = 10*2 + 13*2 + 16*2 + 19*2 = 20 + 26 + 32 + 38 = 116
    assert_eq!(res, 116, "Execution result must match 116");

    // 2. MIR analysis: verify intermediate buffer `temp` is eliminated
    let mut mir = get_mir(code);
    let func = mir
        .functions
        .iter_mut()
        .find(|f| f.name == "pipeline_1d")
        .unwrap();
    let fusions = fuse_polyhedral_stencils(func);
    assert!(fusions > 0, "Polyhedral fusion must succeed");

    let mut has_temp_alloc = false;
    for b in &func.blocks {
        for stmt in &b.statements {
            let Statement::Assign(dest, rval) = stmt;
            if dest.local == "temp" && matches!(rval, Rvalue::Array(_)) {
                has_temp_alloc = true;
            }
        }
    }
    assert!(
        !has_temp_alloc,
        "Intermediate array `temp` allocation must be completely eliminated"
    );
}

#[test]
fn test_polyhedral_horizontal_vertical_2d_stencil_deforestation() {
    let code = r#"
    fn multi_pass_blur(input: [i64; 4]) -> i64 {
        let mut h_blur: [i64; 4] = [0, 0, 0, 0];
        for i in 0..4 {
            h_blur[i] = input[i] + 10;
        }
        let mut v_blur: [i64; 4] = [0, 0, 0, 0];
        for j in 0..4 {
            v_blur[j] = h_blur[j] * 2;
        }
        let mut total: i64 = 0;
        for k in 0..4 {
            total = total + v_blur[k];
        }
        return total;
    }

    fn main() -> i64 {
        let input: [i64; 4] = [5, 15, 25, 35];
        return multi_pass_blur(input);
    }
    "#;

    // 1. Direct native execution verification
    let res = compile_and_run(code, "test_polyhedral_2d_stencil");
    // (5+10)*2 + (15+10)*2 + (25+10)*2 + (35+10)*2
    // = 30 + 50 + 70 + 90 = 240
    assert_eq!(res, 240, "2D multi-pass stencil execution must produce 240");

    // 2. MIR analysis: verify intermediate buffers `h_blur` and `v_blur` are deforested
    let mut mir = get_mir(code);
    let func = mir
        .functions
        .iter_mut()
        .find(|f| f.name == "multi_pass_blur")
        .unwrap();
    let fusions = fuse_polyhedral_stencils(func);
    assert!(
        fusions >= 2,
        "Both h_blur and v_blur pipelines must be fused (fusions >= 2)"
    );

    let mut has_intermediate_alloc = false;
    for b in &func.blocks {
        for stmt in &b.statements {
            let Statement::Assign(dest, rval) = stmt;
            if (dest.local == "h_blur" || dest.local == "v_blur")
                && matches!(rval, Rvalue::Array(_))
            {
                has_intermediate_alloc = true;
            }
        }
    }
    assert!(
        !has_intermediate_alloc,
        "All intermediate stencil buffers must be eliminated from MIR"
    );
}

#[test]
fn test_polyhedral_multi_stage_3pass_deforestation() {
    let code = r#"
    fn three_stage_pipeline(xs: [i64; 4]) -> i64 {
        let mut p1: [i64; 4] = [0, 0, 0, 0];
        for i in 0..4 {
            p1[i] = xs[i] + 5;
        }
        let mut p2: [i64; 4] = [0, 0, 0, 0];
        for j in 0..4 {
            p2[j] = p1[j] * 3;
        }
        let mut sum: i64 = 0;
        for k in 0..4 {
            sum = sum + p2[k];
        }
        return sum;
    }

    fn main() -> i64 {
        let xs: [i64; 4] = [10, 20, 30, 40];
        return three_stage_pipeline(xs);
    }
    "#;

    // 1. Direct native execution
    let res = compile_and_run(code, "test_polyhedral_3stage");
    // (10+5)*3 + (20+5)*3 + (30+5)*3 + (40+5)*3
    // = 45 + 75 + 105 + 135 = 360
    assert_eq!(
        res,
        expected_exit(360),
        "Three stage pipeline execution must produce 360"
    );

    // 2. Fixed-point deforestation: p1 and p2 eliminated
    let mut mir = get_mir(code);
    let func = mir
        .functions
        .iter_mut()
        .find(|f| f.name == "three_stage_pipeline")
        .unwrap();
    let fusions = fuse_polyhedral_stencils(func);
    assert!(
        fusions >= 2,
        "Fixed-point fusion must fuse at least 2 stages"
    );

    let mut has_p1_or_p2 = false;
    for b in &func.blocks {
        for stmt in &b.statements {
            let Statement::Assign(dest, rval) = stmt;
            if (dest.local == "p1" || dest.local == "p2") && matches!(rval, Rvalue::Array(_)) {
                has_p1_or_p2 = true;
            }
        }
    }
    assert!(
        !has_p1_or_p2,
        "Neither p1 nor p2 should allocate array memory in MIR"
    );
}

#[test]
fn test_polyhedral_nested_loops_execution() {
    let code = r#"
    fn nested_sum(n: i64) -> i64 {
        let mut total: i64 = 0;
        for i in 0..n {
            for j in 0..n {
                total = total + i + j;
            }
        }
        return total;
    }

    fn main() -> i64 {
        return nested_sum(4);
    }
    "#;

    let res = compile_and_run(code, "test_polyhedral_nested_loops");
    // sum_{i=0}^{3} sum_{j=0}^{3} (i+j) = 4*sum_i i + 4*sum_j j = 4*6 + 4*6 = 48
    assert_eq!(res, expected_exit(48), "Nested loop sum must be 48");
}

#[test]
fn test_polyhedral_2d_stencil_execution() {
    let code = r#"
    fn stencil_2d() -> i64 {
        let xs: [i64; 9] = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut out: i64 = 0;
        for i in 1..2 {
            for j in 1..2 {
                let idx: i64 = i * 3 + j;
                out = out + xs[idx - 3] + xs[idx] + xs[idx + 3];
            }
        }
        return out;
    }

    fn main() -> i64 {
        return stencil_2d();
    }
    "#;

    let res = compile_and_run(code, "test_polyhedral_2d_stencil_exec");
    // i=1,j=1: idx=4, xs[1]+xs[4]+xs[7] = 2+5+8 = 15
    assert_eq!(res, expected_exit(15), "2D stencil execution must be 15");
}

#[test]
fn test_constant_array_pipeline_folds_to_constant() {
    // Generic constant-index array pipeline: all indices/values are compile-time
    // constants. Const propagation + DCE should collapse it to near-constant MIR.
    let code = r#"
    struct Item { a: i64, b: i64, c: i64, d: i64 }
    fn compute() -> i64 {
        let items: [Item; 4] = [
            Item { a: 0, b: 10, c: 2, d: 1 },
            Item { a: 1, b: 20, c: 3, d: 2 },
            Item { a: 2, b: 30, c: 1, d: 0 },
            Item { a: 3, b: 40, c: 4, d: 3 },
        ];
        let mut xs: [i64; 4] = [0, 0, 0, 0];
        let mut i: i64 = 0;
        while i < 4 {
            let it: Item = items[i];
            xs[i] = (it.b % 7) * it.c;
            i = i + 1;
        }
        let mut ys: [i64; 2] = [0, 0];
        i = 0;
        while i < 4 {
            let it: Item = items[i];
            let slot: i64 = it.a % 2;
            ys[slot] = ys[slot] + xs[i] + it.d;
            i = i + 1;
        }
        let mut total: i64 = 0;
        let mut k: i64 = 0;
        while k < 2 {
            let mut w: i64 = k * 3;
            if k == 0 { w = 5; }
            total = total + ys[k] * w;
            k = k + 1;
        }
        return total;
    }
    fn main() -> i64 { return compute() % 256; }
    "#;

    let mut mir = get_mir(code);
    supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    let main_func = mir.functions.iter().find(|f| f.name == "main").unwrap();
    let main_stmts: usize = main_func.blocks.iter().map(|b| b.statements.len()).sum();
    assert!(
        main_stmts <= 20,
        "constant array pipeline should fold main to small MIR, got {} stmts in {} blocks",
        main_stmts,
        main_func.blocks.len()
    );

    let res = compile_and_run(code, "test_const_array_pipeline_fold");
    // compute: xs = [(10%7)*2=6, (20%7)*3=18, (30%7)*1=2, (40%7)*4=20]
    // ys[0]+=xs[0]+1=7, ys[1]+=xs[1]+2=20, ys[0]+=xs[2]+0=9, ys[1]+=xs[3]+3=43
    // total = ys[0]*5 + ys[1]*3 = 45 + 129 = 174
    assert_eq!(res, expected_exit(174), "constant array pipeline result mismatch");
}
