use std::collections::HashMap;
use std::fs;
use std::process::Command;

use numlang::codegen::compile_supercompiled_to_obj_with_mode;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::polyhedral::{
    fuse_polyhedral_stencils, AffineExpr, DependenceDistance, Inequality, IterationDomain,
};
use numlang::mir::supercompiler::SupercompileMode;
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

    let obj_bytes = compile_supercompiled_to_obj_with_mode(
        &typed,
        SupercompileMode::Classic,
        "size",
    )
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
    assert!(dom1.is_compatible(&dom2), "Domains with identical bounds must be compatible");

    let dom_diff = IterationDomain::new_1d(
        "k",
        0,
        50,
        1,
        BasicBlockId(7),
        BasicBlockId(8),
        BasicBlockId(9),
    );
    assert!(!dom1.is_compatible(&dom_diff), "Domains with different bounds must not be compatible");
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
    assert!(!dep_anti.is_legal, "Negative dependence distance must be rejected as anti-causal");
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
    let func = mir.functions.iter_mut().find(|f| f.name == "pipeline_1d").unwrap();
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
    assert!(!has_temp_alloc, "Intermediate array `temp` allocation must be completely eliminated");
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
    let func = mir.functions.iter_mut().find(|f| f.name == "multi_pass_blur").unwrap();
    let fusions = fuse_polyhedral_stencils(func);
    assert!(fusions >= 2, "Both h_blur and v_blur pipelines must be fused (fusions >= 2)");

    let mut has_intermediate_alloc = false;
    for b in &func.blocks {
        for stmt in &b.statements {
            let Statement::Assign(dest, rval) = stmt;
            if (dest.local == "h_blur" || dest.local == "v_blur") && matches!(rval, Rvalue::Array(_)) {
                has_intermediate_alloc = true;
            }
        }
    }
    assert!(!has_intermediate_alloc, "All intermediate stencil buffers must be eliminated from MIR");
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
    assert_eq!(res, 360, "Three stage pipeline execution must produce 360");

    // 2. Fixed-point deforestation: p1 and p2 eliminated
    let mut mir = get_mir(code);
    let func = mir.functions.iter_mut().find(|f| f.name == "three_stage_pipeline").unwrap();
    let fusions = fuse_polyhedral_stencils(func);
    assert!(fusions >= 2, "Fixed-point fusion must fuse at least 2 stages");

    let mut has_p1_or_p2 = false;
    for b in &func.blocks {
        for stmt in &b.statements {
            let Statement::Assign(dest, rval) = stmt;
            if (dest.local == "p1" || dest.local == "p2") && matches!(rval, Rvalue::Array(_)) {
                has_p1_or_p2 = true;
            }
        }
    }
    assert!(!has_p1_or_p2, "Neither p1 nor p2 should allocate array memory in MIR");
}
