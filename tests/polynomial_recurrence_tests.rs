use std::fs;
use std::process::Command;

use numlang::ast::BinaryOp;
use numlang::codegen::compile_supercompiled_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, MirProgram};
use numlang::mir::supercompiler::generalize::solve_recurrence;
use numlang::mir::supercompiler::recurrence::{
    detect_nonlinear_recurrence, solve_nonlinear_recurrence, NonlinearRecurrence,
};
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::mir::supercompiler::term::{SymTerm, TermInterner};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::types::Type;

fn compile_and_run_supercompiled(src: &str, test_name: &str) -> i32 {
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");

    let obj_bytes = compile_supercompiled_to_obj(&typed).expect("Supercompiled codegen failed");

    let test_dir = std::env::temp_dir().join("numlang_phase60_tests");
    fs::create_dir_all(&test_dir).expect("Failed to create test directory");
    let obj_path = test_dir.join(format!("{}.obj", test_name));
    let exe_path = test_dir.join(format!("{}.exe", test_name));
    fs::write(&obj_path, obj_bytes).expect("Failed to write obj file");

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to run binary");
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

fn get_mir(src: &str) -> MirProgram {
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");
    lower_program(&typed)
}

#[test]
fn test_detect_square_pyramid_sum_closed_form() {
    let mut interner = TermInterner::new();
    // Sum of squares: S_n = sum_{k=1}^n k^2 = n(n+1)(2n+1)/6
    // S_0 = 0, S_1 = 1, S_2 = 5, S_3 = 14, S_4 = 30, S_5 = 55, S_6 = 91
    let samples = vec![0, 1, 5, 14, 30, 55, 91];

    let detected = detect_nonlinear_recurrence(&samples, &mut interner)
        .expect("Failed to detect nonlinear recurrence for square pyramid");
    match detected {
        NonlinearRecurrence::PolynomialSum { .. } => {}
        _ => panic!("Expected PolynomialSum, got: {:?}", detected),
    }

    let n10 = interner.intern_int(10);
    let sol = solve_nonlinear_recurrence(&detected, n10, &mut interner);
    if let SymTerm::ConstInt(val, _) = interner.get(sol) {
        assert_eq!(*val, 385, "10th square pyramid sum must be 385");
    } else {
        panic!("Expected ConstInt result for n=10");
    }
}

#[test]
fn test_detect_cubic_sum_closed_form() {
    let mut interner = TermInterner::new();
    // Sum of cubes: S_n = sum_{k=1}^n k^3 = n^2(n+1)^2 / 4
    // S_0 = 0, S_1 = 1, S_2 = 9, S_3 = 36, S_4 = 100, S_5 = 225, S_6 = 441
    let samples = vec![0, 1, 9, 36, 100, 225, 441];

    let detected = detect_nonlinear_recurrence(&samples, &mut interner)
        .expect("Failed to detect nonlinear recurrence for cubic sum");
    match detected {
        NonlinearRecurrence::CubicSum { .. } => {}
        _ => panic!("Expected CubicSum, got: {:?}", detected),
    }

    let n10 = interner.intern_int(10);
    let sol = solve_nonlinear_recurrence(&detected, n10, &mut interner);
    if let SymTerm::ConstInt(val, _) = interner.get(sol) {
        assert_eq!(*val, 3025, "10th cubic sum must be 3025");
    } else {
        panic!("Expected ConstInt result for n=10");
    }
}

#[test]
fn test_detect_geometric_series_sum() {
    let mut interner = TermInterner::new();
    // S_n = sum_{k=0}^{n-1} 2^k = 2^n - 1
    // S_0 = 0, S_1 = 1, S_2 = 3, S_3 = 7, S_4 = 15, S_5 = 31, S_6 = 63
    let samples = vec![0, 1, 3, 7, 15, 31, 63];

    let detected = detect_nonlinear_recurrence(&samples, &mut interner)
        .expect("Failed to detect geometric series recurrence");
    match detected {
        NonlinearRecurrence::GeometricSeries { .. } => {}
        _ => panic!("Expected GeometricSeries, got: {:?}", detected),
    }

    let n10 = interner.intern_int(10);
    let sol = solve_nonlinear_recurrence(&detected, n10, &mut interner);
    if let SymTerm::ConstInt(val, _) = interner.get(sol) {
        assert_eq!(*val, 1023, "2^10 - 1 must be 1023");
    } else {
        panic!("Expected ConstInt result for n=10");
    }
}

#[test]
fn test_detect_exponential_power_loop() {
    let mut interner = TermInterner::new();
    // P_n = 3 * 2^n
    // P_0 = 3, P_1 = 6, P_2 = 12, P_3 = 24, P_4 = 48, P_5 = 96
    let samples = vec![3, 6, 12, 24, 48, 96];

    let detected = detect_nonlinear_recurrence(&samples, &mut interner)
        .expect("Failed to detect exponential power");
    match detected {
        NonlinearRecurrence::ExponentialPower { .. } => {}
        _ => panic!("Expected ExponentialPower, got: {:?}", detected),
    }

    let n10 = interner.intern_int(10);
    let sol = solve_nonlinear_recurrence(&detected, n10, &mut interner);
    if let SymTerm::ConstInt(val, _) = interner.get(sol) {
        assert_eq!(*val, 3072, "3 * 2^10 must be 3072");
    } else {
        panic!("Expected ConstInt result for n=10");
    }
}

#[test]
fn test_detect_power_tower_recurrence() {
    let mut interner = TermInterner::new();
    // T_n = 2^(2^n)
    // T_0 = 2, T_1 = 4, T_2 = 16, T_3 = 256, T_4 = 65536
    let samples = vec![2, 4, 16, 256, 65536];

    let detected = detect_nonlinear_recurrence(&samples, &mut interner)
        .expect("Failed to detect power tower recurrence");
    match detected {
        NonlinearRecurrence::PowerTower { .. } => {}
        _ => panic!("Expected PowerTower, got: {:?}", detected),
    }

    let n3 = interner.intern_int(3);
    let sol3 = solve_nonlinear_recurrence(&detected, n3, &mut interner);
    if let SymTerm::ConstInt(val, _) = interner.get(sol3) {
        assert_eq!(*val, 256, "T_3 must be 256");
    } else {
        panic!("Expected ConstInt for T_3");
    }

    let n4 = interner.intern_int(4);
    let sol4 = solve_nonlinear_recurrence(&detected, n4, &mut interner);
    if let SymTerm::ConstInt(val, _) = interner.get(sol4) {
        assert_eq!(*val, 65536, "T_4 must be 65536");
    } else {
        panic!("Expected ConstInt for T_4");
    }
}

#[test]
fn test_symbolic_generalization_preserves_structure() {
    let mut interner = TermInterner::new();
    let samples = vec![0, 1, 5, 14, 30, 55, 91];

    let n_sym = interner.intern_var(
        numlang::mir::Place {
            local: "n".to_string(),
            projections: Vec::new(),
        },
        Type::I64,
    );
    let sol = solve_recurrence(&samples, n_sym, &mut interner)
        .expect("Failed to solve recurrence symbolically");

    // Must be a composite expression containing Binary additions and multiplications
    match interner.get(sol) {
        SymTerm::Binary(BinaryOp::Add, _, _, _) => {}
        other => panic!(
            "Expected symbolic polynomial sum expression, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_supercompiled_square_pyramid_execution() {
    let src = r#"
        fn pyramid(n: i64) -> i64 {
            let mut sum: i64 = 0;
            let mut i: i64 = 1;
            while i <= n {
                sum = sum + i * i;
                i = i + 1;
            }
            return sum;
        }

        fn main() -> i64 {
            return pyramid(10);
        }
    "#;

    let mut mir = get_mir(src);
    let stats = supercompile_mir_program(&mut mir);
    assert!(
        stats.loops_collapsed >= 1,
        "Pyramid loop should be collapsed to closed form"
    );

    let exit_code = compile_and_run_supercompiled(src, "test_square_pyramid_run");
    assert_eq!(
        exit_code,
        expected_exit(385),
        "Pyramid sum for n=10 must be 385"
    );
}

#[test]
fn test_supercompiled_cubic_sum_execution() {
    let src = r#"
        fn cubic(n: i64) -> i64 {
            let mut sum: i64 = 0;
            let mut i: i64 = 1;
            while i <= n {
                sum = sum + i * i * i;
                i = i + 1;
            }
            return sum;
        }

        fn main() -> i64 {
            return cubic(10);
        }
    "#;

    let mut mir = get_mir(src);
    let stats = supercompile_mir_program(&mut mir);
    assert!(
        stats.loops_collapsed >= 1,
        "Cubic sum loop should be collapsed to closed form"
    );

    let exit_code = compile_and_run_supercompiled(src, "test_cubic_sum_run");
    assert_eq!(
        exit_code,
        expected_exit(3025),
        "Cubic sum for n=10 must be 3025"
    );
}

#[test]
fn test_supercompiled_geometric_series_execution() {
    let src = r#"
        fn geom(n: i64) -> i64 {
            let mut sum: i64 = 0;
            let mut p: i64 = 1;
            let mut i: i64 = 0;
            while i < n {
                sum = sum + p;
                p = p * 2;
                i = i + 1;
            }
            return sum;
        }

        fn main() -> i64 {
            return geom(10);
        }
    "#;

    let mut mir = get_mir(src);
    let stats = supercompile_mir_program(&mut mir);
    assert!(
        stats.loops_collapsed >= 1,
        "Coupled geometric series loop should be collapsed"
    );

    let exit_code = compile_and_run_supercompiled(src, "test_geom_series_run");
    assert_eq!(exit_code, expected_exit(1023), "2^10 - 1 must be 1023");
}

#[test]
fn test_supercompiled_exponential_power_execution() {
    let src = r#"
        fn exp_pow(n: i64) -> i64 {
            let mut p: i64 = 3;
            let mut i: i64 = 0;
            while i < n {
                p = p * 2;
                i = i + 1;
            }
            return p;
        }

        fn main() -> i64 {
            return exp_pow(10);
        }
    "#;

    let mut mir = get_mir(src);
    let stats = supercompile_mir_program(&mut mir);
    assert!(
        stats.loops_collapsed >= 1,
        "Exponential power loop should be collapsed"
    );

    let exit_code = compile_and_run_supercompiled(src, "test_exp_power_run");
    assert_eq!(exit_code, expected_exit(3072), "3 * 2^10 must be 3072");
}

#[test]
fn test_recurrence_large_k_overflow_soundness() {
    let mut interner = TermInterner::new();
    let samples = vec![0, 1, 3, 6, 10, 15]; // triangular sequence s_k = k*(k+1)/2
    let k_val = 4_000_000_000i64;
    let k_term = interner.intern_int(k_val);
    let sol = solve_recurrence(&samples, k_term, &mut interner).expect("solve recurrence");
    let result = interner.get(sol).to_literal().expect("constant literal");
    let result_val = match result {
        numlang::typecheck::typed_ast::TypedLiteral::Int(v, _) => v,
        _ => panic!("expected int"),
    };
    let expected = (k_val / 2) * (k_val + 1); // 8_000_000_002_000_000_000
    assert_eq!(
        result_val, expected,
        "Recurrence must not overflow intermediate product before division!"
    );
}
