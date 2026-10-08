use std::fs;
use std::process::Command;

use numlang::codegen::compile_supercompiled_to_obj_with_mode;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::polyhedral::{
    apply_polyhedral_tiling, BareissSimplex, ConstraintOp, PlutoScheduler, SimplexResult,
    TilingConfig,
};
use numlang::mir::supercompiler::SupercompileMode;
use numlang::mir::Terminator;
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

    let test_dir = std::env::temp_dir().join(format!("numlang_poly_ilp_{}", test_name));
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
fn test_bareiss_simplex_fraction_free_pivoting() {
    // Solve linear program via Bareiss integer pivoting:
    // Minimize: x_0 + 2 * x_1
    // Subject to:
    //   x_0 + x_1 >= 3
    //   x_0 <= 5
    //   x_1 <= 5
    //   x_0, x_1 >= 0
    let mut simplex = BareissSimplex::new(2);
    simplex.add_constraint(&[1, 1], ConstraintOp::Ge, 3);
    simplex.add_constraint(&[1, 0], ConstraintOp::Le, 5);
    simplex.add_constraint(&[0, 1], ConstraintOp::Le, 5);
    simplex.set_objective(&[1, 2], true);

    let res = simplex.solve();
    match res {
        SimplexResult::Optimal {
            objective,
            solution,
            pivots,
        } => {
            // Optimal solution: x_0 = 3, x_1 = 0, obj = 3
            assert_eq!(objective, 3);
            assert_eq!(solution[0], 3);
            assert_eq!(solution[1], 0);
            assert!(pivots > 0, "At least one pivot must be performed");
            assert!(pivots <= 20, "Pivots must terminate in small steps");
        }
        other => panic!("Expected optimal solution, got {:?}", other),
    }
}

#[test]
fn test_simplex_pivot_bound_for_8_dimensions() {
    // Verify requirement ILP-05:
    // Simplex solver terminates in <= 1,000 pivot steps for loops with <= 8 dimensions.
    let dims = 8;
    let mut simplex = BareissSimplex::new(dims);

    // Non-triviality constraint: sum(x_k) >= 1
    let ones = vec![1i64; dims];
    simplex.add_constraint(&ones, ConstraintOp::Ge, 1);

    // Coordinate bounds: 0 <= x_k <= 4
    for k in 0..dims {
        let mut row = vec![0i64; dims];
        row[k] = 1;
        simplex.add_constraint(&row, ConstraintOp::Le, 4);
        simplex.add_constraint(&row, ConstraintOp::Ge, 0);
    }

    // 8 dependence distance vectors (unit vectors along each dimension)
    for k in 0..dims {
        let mut dep = vec![0i64; dims];
        dep[k] = 1;
        simplex.add_constraint(&dep, ConstraintOp::Ge, 0);
    }

    // Additional cross-dimensional diagonal constraints
    for k in 0..(dims - 1) {
        let mut diag = vec![0i64; dims];
        diag[k] = 1;
        diag[k + 1] = 1;
        simplex.add_constraint(&diag, ConstraintOp::Ge, 0);
    }

    let obj = vec![1i64; dims];
    simplex.set_objective(&obj, true);

    let res = simplex.solve();
    match res {
        SimplexResult::Optimal { pivots, .. } => {
            assert!(
                pivots <= 1000,
                "Simplex solver must terminate in <= 1,000 pivot steps for <= 8 dimensions, took: {}",
                pivots
            );
        }
        other => panic!("Expected optimal schedule, got {:?}", other),
    }
}

#[test]
fn test_pluto_permutability_matrix_multiply_3d() {
    // 3D Matrix Multiplication C[i][j] += A[i][k] * B[k][j]:
    // Dependence vectors:
    // d_C = (0, 0, 1) - accumulation along k
    // d_A = (0, 1, 0) - reuse of A along j
    // d_B = (1, 0, 0) - reuse of B along i
    let mut scheduler = PlutoScheduler::new(3);
    scheduler.add_dependence(&[0, 0, 1]);
    scheduler.add_dependence(&[0, 1, 0]);
    scheduler.add_dependence(&[1, 0, 0]);

    let schedule = scheduler.compute_schedule();
    assert_eq!(schedule.dimensions.len(), 3);
    assert!(
        schedule.is_permutable,
        "3D matrix multiplication must be recognized as fully permutable"
    );
    assert!(
        schedule.is_tiled,
        "Permutable multi-dimensional loop must enable rectangular tiling"
    );

    // Verify legality for each schedule vector theta . d >= 0
    for dim in &schedule.dimensions {
        assert!(
            scheduler.is_legal_vector(&dim.coeffs),
            "Schedule dimension {:?} must satisfy all dependence constraints",
            dim.coeffs
        );
    }
}

#[test]
fn test_pluto_skewing_for_wavefront_dependence() {
    // Loop with reverse dependence d = (1, -1) (wavefront pattern)
    // theta . (1, -1) = theta_0 - theta_1 >= 0 => theta_0 >= theta_1
    let mut scheduler = PlutoScheduler::new(2);
    scheduler.add_dependence(&[1, -1]);

    // Unskewed (0, 1) is illegal: 0 - 1 = -1 < 0
    assert!(
        !scheduler.is_legal_vector(&[0, 1]),
        "Unskewed inner loop (0, 1) violates dependence (1, -1)"
    );

    // Skewed schedule (1, 1) is legal: 1 - 1 = 0 >= 0
    assert!(
        scheduler.is_legal_vector(&[1, 1]),
        "Skewed schedule (1, 1) must be legal under dependence (1, -1)"
    );

    let schedule = scheduler.compute_schedule();
    assert!(schedule.is_permutable);
}

#[test]
fn test_polyhedral_loop_tiling_3d_matmul_and_buffer_contraction() {
    let code = r#"
    fn matmul_2x2(a: [i64; 4], b: [i64; 4]) -> i64 {
        let mut c: [i64; 4] = [0, 0, 0, 0];
        let mut temp_buf: [i64; 4] = [0, 0, 0, 0];
        for i in 0..2 {
            for j in 0..2 {
                for k in 0..2 {
                    temp_buf[k] = a[i * 2 + k] * b[k * 2 + j];
                    c[i * 2 + j] = c[i * 2 + j] + temp_buf[k];
                }
            }
        }
        return c[0] + c[1] + c[2] + c[3];
    }

    fn main() -> i64 {
        // A = [[1, 2], [3, 4]]
        // B = [[5, 6], [7, 8]]
        // C = A * B:
        // C[0][0] = 1*5 + 2*7 = 5 + 14 = 19
        // C[0][1] = 1*6 + 2*8 = 6 + 16 = 22
        // C[1][0] = 3*5 + 4*7 = 15 + 28 = 43
        // C[1][1] = 3*6 + 4*8 = 18 + 32 = 50
        // Total sum = 19 + 22 + 43 + 50 = 134
        let a: [i64; 4] = [1, 2, 3, 4];
        let b: [i64; 4] = [5, 6, 7, 8];
        let res: i64 = matmul_2x2(a, b);
        if res == 134 {
            return 0;
        }
        return 1;
    }
    "#;

    // 1. Direct native execution and numerical verification
    let exit_code = compile_and_run(code, "poly_ilp_matmul_2x2");
    assert_eq!(
        exit_code, 0,
        "Native matmul execution must produce 134 and return 0"
    );

    // 2. MIR analysis: verify loop tiling and buffer contraction
    let mut mir = get_mir(code);
    let func = mir
        .functions
        .iter_mut()
        .find(|f| f.name == "matmul_2x2")
        .unwrap();

    let tiling_cfg = TilingConfig {
        tile_size: 32,
        min_depth: 2,
        vectorize_innermost: true,
    };
    let tiled_count = apply_polyhedral_tiling(func, &tiling_cfg);
    assert!(
        tiled_count > 0,
        "Polyhedral loop tiling must succeed on nested loop"
    );

    // Verify intermediate buffer `temp_buf` was contracted to scalar
    let mut has_temp_alloc = false;
    for b in &func.blocks {
        for stmt in &b.statements {
            let Statement::Assign(dest, rval) = stmt;
            if dest.local == "temp_buf" && matches!(rval, Rvalue::Array(_) | Rvalue::Alloc(_)) {
                has_temp_alloc = true;
            }
        }
    }
    assert!(
        !has_temp_alloc,
        "Intermediate array `temp_buf` allocation must be contracted away"
    );
}

#[test]
fn test_tiled_loop_vectorization_fork_emission() {
    let code = r#"
    fn grid_blur_2d(img: [i64; 4]) -> i64 {
        let mut out: [i64; 4] = [0, 0, 0, 0];
        for i in 0..2 {
            for j in 0..2 {
                out[i * 2 + j] = img[i * 2 + j] * 3;
            }
        }
        return out[0] + out[1] + out[2] + out[3];
    }
    "#;

    let mut mir = get_mir(code);
    let func = mir
        .functions
        .iter_mut()
        .find(|f| f.name == "grid_blur_2d")
        .unwrap();

    let tiling_cfg = TilingConfig {
        tile_size: 32,
        min_depth: 2,
        vectorize_innermost: true,
    };
    let count = apply_polyhedral_tiling(func, &tiling_cfg);
    assert!(count > 0, "Tiling must succeed");

    // Verify that Terminator::Fork was emitted for innermost tile dimension
    let has_fork = func
        .blocks
        .iter()
        .any(|b| matches!(b.terminator, Terminator::Fork { .. }));
    assert!(
        has_fork,
        "Innermost tile dimension must emit Terminator::Fork for parallel vectorization"
    );
}
