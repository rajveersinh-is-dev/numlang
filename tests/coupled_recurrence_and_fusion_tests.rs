use std::fs;
use std::process::Command;

use numlang::ast::BinaryOp;
use numlang::codegen::compile_supercompiled_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, Rvalue, Statement};
use numlang::mir::supercompiler::generalize::{
    solve_coupled_2var_recurrence, solve_order2_recurrence, solve_order3_recurrence,
};
use numlang::mir::supercompiler::{supercompile_mir_program, supercompile_mir_program_with_mode, SupercompileMode};
use numlang::mir::supercompiler::term::{SymTerm, TermInterner};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::types::Type;

fn compile_and_run_supercompiled(src: &str, test_name: &str) -> i32 {
    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();

    let obj_bytes = compile_supercompiled_to_obj(&typed).expect("Supercompiled codegen failed");

    let test_dir = std::env::temp_dir().join("numlang_phase4_tests");
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
fn test_order2_recurrence_fibonacci_direct() {
    let mut interner = TermInterner::new();
    // F(0)=0, F(1)=1, F(2)=1, F(3)=2, F(4)=3, F(5)=5, F(6)=8, F(7)=13
    let samples = vec![0, 1, 1, 2, 3, 5, 8, 13, 21];

    let n10 = interner.intern_int(10);
    let sol10 =
        solve_order2_recurrence(&samples, n10, &mut interner).expect("Failed to solve F(10)");
    if let SymTerm::ConstInt(val, _) = interner.get(sol10) {
        assert_eq!(*val, 55, "F(10) must be 55");
    } else {
        panic!("Expected ConstInt for F(10)");
    }

    let n20 = interner.intern_int(20);
    let sol20 =
        solve_order2_recurrence(&samples, n20, &mut interner).expect("Failed to solve F(20)");
    if let SymTerm::ConstInt(val, _) = interner.get(sol20) {
        assert_eq!(*val, 6765, "F(20) must be 6765");
    } else {
        panic!("Expected ConstInt for F(20)");
    }
}

#[test]
fn test_order2_recurrence_integer_roots_symbolic() {
    let mut interner = TermInterner::new();
    // s_k = 3*s_{k-1} - 2*s_{k-2}, roots are 1 and 2
    // s_k = 2^(k+1) - 1 => s0=1, s1=3, s2=7, s3=15, s4=31, s5=63
    let samples = vec![1, 3, 7, 15, 31, 63];

    let place = numlang::mir::Place {
        local: "n".to_string(),
        projections: vec![],
    };
    let n_sym = interner.intern_var(place, Type::I64);

    let sol = solve_order2_recurrence(&samples, n_sym, &mut interner)
        .expect("Failed to solve order 2 integer roots");

    // Must produce a symbolic sum of powers
    match interner.get(sol) {
        SymTerm::Binary(BinaryOp::Add, _, _, _) => {}
        other => panic!(
            "Expected Binary Add term for closed form, found: {:?}",
            other
        ),
    }

    // Now test with concrete n=5: 2^6 - 1 = 63
    let n5 = interner.intern_int(5);
    let sol5 =
        solve_order2_recurrence(&samples, n5, &mut interner).expect("Failed to solve for n=5");
    if let SymTerm::ConstInt(val, _) = interner.get(sol5) {
        assert_eq!(*val, 63, "s_5 must be 63");
    } else {
        panic!("Expected ConstInt 63");
    }
}

#[test]
fn test_order3_recurrence_tribonacci() {
    let mut interner = TermInterner::new();
    // Tribonacci: T(0)=0, T(1)=0, T(2)=1, T(3)=1, T(4)=2, T(5)=4, T(6)=7, T(7)=13, T(8)=24
    let samples = vec![0, 0, 1, 1, 2, 4, 7, 13, 24, 44];

    let n7 = interner.intern_int(7);
    let sol7 = solve_order3_recurrence(&samples, n7, &mut interner)
        .expect("Failed to solve Tribonacci T(7)");
    if let SymTerm::ConstInt(val, _) = interner.get(sol7) {
        assert_eq!(*val, 13, "T(7) must be 13");
    } else {
        panic!("Expected ConstInt 13");
    }

    let n9 = interner.intern_int(9);
    let sol9 = solve_order3_recurrence(&samples, n9, &mut interner)
        .expect("Failed to solve Tribonacci T(9)");
    if let SymTerm::ConstInt(val, _) = interner.get(sol9) {
        assert_eq!(*val, 44, "T(9) must be 44");
    } else {
        panic!("Expected ConstInt 44");
    }
}

#[test]
fn test_interprocedural_inlining_and_folding() {
    let src = r#"
        fn square(x: i64) -> i64 {
            return x * x;
        }

        fn main() -> i64 {
            let a: i64 = 9;
            return square(a);
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();
    let mut mir = lower_program(&typed);

    let stats = supercompile_mir_program(&mut mir);
    assert!(stats.nodes_explored > 0);

    let main_func = mir.functions.iter().find(|f| f.name == "main").unwrap();
    // Verify that main's body has folded the call to square into a constant or direct computation
    let has_call = main_func.blocks.iter().any(|b| {
        b.statements
            .iter()
            .any(|s| matches!(s, Statement::Assign(_, Rvalue::Call(..))))
    });
    assert!(
        !has_call,
        "Call to square should be completely eliminated via interprocedural inlining"
    );

    let code = compile_and_run_supercompiled(src, "interproc_square");
    assert_eq!(code, 81);
}

#[test]
fn test_interprocedural_triangular_loop_fusion() {
    let src = r#"
        fn tri_sum(n: i64) -> i64 {
            let mut acc: i64 = 0;
            let mut i: i64 = 1;
            while i <= n {
                acc = acc + i;
                i = i + 1;
            }
            return acc;
        }

        fn main() -> i64 {
            return tri_sum(100) % 256;
        }
    "#;

    let code = compile_and_run_supercompiled(src, "interproc_tri");
    assert_eq!(code, 186);
}

#[test]
fn test_coupled_fibonacci_native_execution() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            let mut a: i64 = 0;
            let mut b: i64 = 1;
            let mut i: i64 = 0;
            while i < n {
                let t: i64 = a + b;
                a = b;
                b = t;
                i = i + 1;
            }
            return a;
        }

        fn main() -> i64 {
            return fib(10);
        }
    "#;

    // F(10) = 55
    let code = compile_and_run_supercompiled(src, "coupled_fib_10");
    assert_eq!(code, 55);
}

#[test]
fn test_coupled_fibonacci_symbolic_recurrence() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            let mut a: i64 = 0;
            let mut b: i64 = 1;
            let mut i: i64 = 0;
            while i < n {
                let t: i64 = a + b;
                a = b;
                b = t;
                i = i + 1;
            }
            return a;
        }

        fn run_fib(x: i64) -> i64 {
            return fib(x);
        }

        fn main() -> i64 {
            return run_fib(10);
        }
    "#;

    let code = compile_and_run_supercompiled(src, "coupled_fib_symbolic");
    assert_eq!(code, 55);
}

#[test]
fn test_coupled_2var_recurrence_direct() {
    let mut interner = TermInterner::new();
    // a_{k+1} = 2*a_k + b_k
    // b_{k+1} = a_k + 2*b_k
    // a0 = 1, b0 = 0
    // a: 1, 2, 5, 14, 41
    // b: 0, 1, 4, 13, 40
    let samples_a = vec![1, 2, 5, 14];
    let samples_b = vec![0, 1, 4, 13];

    let n4 = interner.intern_int(4);
    let (sol_a, sol_b) = solve_coupled_2var_recurrence(&samples_a, &samples_b, n4, &mut interner)
        .expect("Failed to solve coupled 2-variable recurrence for n=4");

    if let SymTerm::ConstInt(val_a, _) = interner.get(sol_a) {
        assert_eq!(*val_a, 41, "a_4 must be 41");
    } else {
        panic!("Expected ConstInt for a_4");
    }
    if let SymTerm::ConstInt(val_b, _) = interner.get(sol_b) {
        assert_eq!(*val_b, 40, "b_4 must be 40");
    } else {
        panic!("Expected ConstInt for b_4");
    }

    // Symbolic recurrence has no codegen handler; solver must not emit unhandleable intrinsics.
    let place = numlang::mir::Place {
        local: "n".to_string(),
        projections: vec![],
    };
    let n_sym = interner.intern_var(place, Type::I64);
    let sym = solve_coupled_2var_recurrence(&samples_a, &samples_b, n_sym, &mut interner);
    assert!(
        sym.is_none(),
        "Symbolic coupled recurrence should fall back (no codegen handler) rather than emit __coupled_* intrinsics"
    );
}

#[test]
fn test_coupled_2var_linear_recurrence() {
    let src = r#"
        fn coupled(n: i64) -> i64 {
            let mut a: i64 = 1;
            let mut b: i64 = 0;
            let mut i: i64 = 0;
            while i < n {
                let next_a: i64 = 2 * a + b;
                let next_b: i64 = a + 2 * b;
                a = next_a;
                b = next_b;
                i = i + 1;
            }
            return a + b;
        }

        fn main() -> i64 {
            return coupled(4);
        }
    "#;
    let code = compile_and_run_supercompiled(src, "coupled_2var");
    assert_eq!(code, 81);
}

#[test]
fn test_affine_coupled_recurrence_direct() {
    let mut interner = TermInterner::new();
    // a_{k+1} = 2*a_k + b_k + 1
    // b_{k+1} = a_k + 2*b_k - 1
    // a0 = 1, b0 = 0
    let mut a = 1i64;
    let mut b = 0i64;
    let mut sa = vec![a];
    let mut sb = vec![b];
    for _ in 0..6 {
        let na = 2 * a + b + 1;
        let nb = a + 2 * b - 1;
        a = na;
        b = nb;
        sa.push(a);
        sb.push(b);
    }

    let n4 = interner.intern_int(4);
    let (sol_a, sol_b) =
        solve_coupled_2var_recurrence(&sa, &sb, n4, &mut interner)
            .expect("Failed to solve affine coupled recurrence for n=4");
    if let SymTerm::ConstInt(val_a, _) = interner.get(sol_a) {
        assert_eq!(*val_a, sa[4], "a_4 must match iterative");
    } else {
        panic!("Expected ConstInt for a_4");
    }
    if let SymTerm::ConstInt(val_b, _) = interner.get(sol_b) {
        assert_eq!(*val_b, sb[4], "b_4 must match iterative");
    } else {
        panic!("Expected ConstInt for b_4");
    }

    // Symbolic recurrence has no codegen handler; solver must not emit unhandleable intrinsics.
    let place = numlang::mir::Place {
        local: "n".to_string(),
        projections: vec![],
    };
    let n_sym = interner.intern_var(place, Type::I64);
    let sym = solve_coupled_2var_recurrence(&sa, &sb, n_sym, &mut interner);
    assert!(
        sym.is_none(),
        "Symbolic coupled recurrence should fall back (no codegen handler) rather than emit __coupled_* intrinsics"
    );
}

#[test]
fn test_nway_affine_matrix_recurrence_large_n() {
    let mut interner = TermInterner::new();
    // X_{n+1} = M X_n + c, M=[[2,1],[1,3]], c=[5,-2], X0=[1,0]
    let mut x = vec![1i64, 0i64];
    let mut t0 = vec![x[0]];
    let mut t1 = vec![x[1]];
    for _ in 0..20 {
        let nx0 = 2 * x[0] + x[1] + 5;
        let nx1 = x[0] + 3 * x[1] - 2;
        x = vec![nx0, nx1];
        t0.push(x[0]);
        t1.push(x[1]);
    }
    let trajs = vec![t0.clone(), t1.clone()];
    let sys = numlang::mir::supercompiler::recurrence::detect_nway_linear_system(&trajs)
        .expect("detect nway");
    // Large N with modular wrap: n=1000
    let big_n = 1000i64;
    // iterative
    let mut xi = vec![1i64, 0i64];
    for _ in 0..big_n {
        let nx0 = 2i64.wrapping_mul(xi[0]).wrapping_add(xi[1]).wrapping_add(5);
        let nx1 = xi[0].wrapping_add(3i64.wrapping_mul(xi[1])).wrapping_sub(2);
        xi = vec![nx0, nx1];
    }
    let n_term = interner.intern_int(big_n);
    let res = numlang::mir::supercompiler::recurrence::solve_nway_recurrence(&sys, n_term, &mut interner);
    assert_eq!(res.len(), 2);
    if let SymTerm::ConstInt(v, _) = interner.get(res[0]) {
        assert_eq!(*v, xi[0], "nway large N component 0 must match iterative");
    } else {
        panic!("Expected ConstInt");
    }
    if let SymTerm::ConstInt(v, _) = interner.get(res[1]) {
        assert_eq!(*v, xi[1], "nway large N component 1 must match iterative");
    } else {
        panic!("Expected ConstInt");
    }
}

#[test]
fn test_fib_matrix_coupled_recurrence_collapses() {
    let src = r#"
        fn fib_coupled() -> i64 {
            let mut a: i64 = 0;
            let mut b: i64 = 1;
            let mut i: i64 = 0;
            while i < 40 {
                let next_a: i64 = b;
                let next_b: i64 = (a + b) % 1000000007;
                a = next_a;
                b = next_b;
                i = i + 1;
            }
            return a;
        }

        fn main() -> i64 {
            let mut sum: i64 = 0;
            for i in 0..1000 {
                sum = (sum + fib_coupled()) % 1000000007;
            }
            println(sum);
            return sum % 256;
        }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();
    let mut mir = lower_program(&typed);

    let stats = supercompile_mir_program(&mut mir);

    let fib = mir
        .functions
        .iter()
        .find(|f| f.name == "fib_coupled")
        .expect("fib_coupled should exist");
    let has_loop = fib.blocks.iter().any(|b| {
        matches!(
            b.terminator,
            numlang::mir::Terminator::BranchIf { .. } | numlang::mir::Terminator::Switch { .. }
        )
    });

    // The coupled Fibonacci matrix recurrence must collapse to a closed form.
    assert!(
        !has_loop,
        "fib_coupled loop should be collapsed by coupled recurrence solver"
    );
    assert!(
        stats.loops_collapsed >= 1,
        "expected at least one loop collapse, got {:?}",
        stats
    );

    let code = compile_and_run_supercompiled(src, "fib_matrix_collapse");
    assert_eq!(code, 46);
}

/// Regression: enabling `Mod` in the accumulator simulator made the dynamic-bound
/// matrix loop collapse to the unmodded value `1000 * fib(40) == 102334155000`
/// (exit 248) instead of the correct `334154286` (exit 46). The static variant
/// must keep its closed-form collapse while the runtime-opaque-bound variant is
/// kept correct (its modular loop must not be folded into a constant).
#[test]
fn test_fib_matrix_static_collapse_and_dynamic_correctness() {
    let static_src = r#"
        fn fib_coupled(n: i64) -> i64 {
            let mut a: i64 = 0;
            let mut b: i64 = 1;
            let mut i: i64 = 0;
            while i < n {
                let next_a: i64 = b;
                let next_b: i64 = (a + b) % 1000000007;
                a = next_a;
                b = next_b;
                i = i + 1;
            }
            return a;
        }

        fn main() -> i64 {
            let mut sum: i64 = 0;
            for i in 0..1000 {
                sum = (sum + fib_coupled(40)) % 1000000007;
            }
            println(sum);
            return sum % 256;
        }
    "#;

    let dynamic_src = r#"
        fn fib_coupled(n: i64) -> i64 {
            let mut a: i64 = 0;
            let mut b: i64 = 1;
            let mut i: i64 = 0;
            while i < n {
                let next_a: i64 = b;
                let next_b: i64 = (a + b) % 1000000007;
                a = next_a;
                b = next_b;
                i = i + 1;
            }
            return a;
        }

        fn main() -> i64 {
            let mut iters: i64 = read_i64();
            if iters <= 0 {
                iters = 1000;
            }
            let f40: i64 = fib_coupled(40);
            let mut sum: i64 = 0;
            let mut j: i64 = 0;
            while j < iters {
                sum = (sum + f40) % 1000000007;
                j = j + 1;
            }
            println(sum);
            return sum % 256;
        }
    "#;

    // Static variant: fib_coupled's loop must collapse to a closed form.
    let mut static_mir = lower_program(&typecheck(&parse(&tokenize(static_src).unwrap()).unwrap()).unwrap());
    let static_stats = supercompile_mir_program(&mut static_mir);
    assert!(
        static_stats.loops_collapsed >= 1,
        "static fib_matrix must collapse its coupled loop, got {:?}",
        static_stats
    );
    assert_eq!(
        compile_and_run_supercompiled(static_src, "fib_matrix_static_regression"),
        46,
        "static fib_matrix must exit 46"
    );

    // Dynamic variant: the runtime-opaque-bound modular loop must NOT be folded
    // into the unmodded constant. Detect the regression signature directly.
    let mut dynamic_mir = lower_program(&typecheck(&parse(&tokenize(dynamic_src).unwrap()).unwrap()).unwrap());
    supercompile_mir_program(&mut dynamic_mir);
    for func in &dynamic_mir.functions {
        for block in &func.blocks {
            for stmt in &block.statements {
                if let Statement::Assign(_, Rvalue::Constant(numlang::typecheck::typed_ast::TypedLiteral::Int(v, _))) = stmt {
                    assert_ne!(
                        *v, 102334155000i64,
                        "dynamic fib_matrix must not collapse the modular loop to the unmodded value"
                    );
                }
            }
        }
    }
    assert_eq!(
        compile_and_run_supercompiled(dynamic_src, "fib_matrix_dynamic_regression"),
        46,
        "dynamic fib_matrix must exit 46 for its runtime bound"
    );
}

#[test]
fn test_constant_array_store_load_fold_generic() {
    // Generic renamed kernel: constant arrays with constant-index stores/loads
    // should be folded away by const propagation + DCE.
    let src = r#"
    fn compute() -> i64 {
        let mut buf: [i64; 4] = [0, 0, 0, 0];
        let mut i: i64 = 0;
        while i < 4 {
            buf[i] = (i + 1) * (i + 2);
            i = i + 1;
        }
        let mut acc: i64 = 0;
        let mut j: i64 = 0;
        while j < 4 {
            acc = acc + buf[j];
            j = j + 1;
        }
        return acc;
    }
    fn main() -> i64 { return compute() % 256; }
    "#;

    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let mut typed = typecheck(&ast).unwrap();
    typed.desugar_for_loops();
    let mut mir = lower_program(&typed);
    supercompile_mir_program_with_mode(&mut mir, SupercompileMode::Distill, "size");
    let func = mir.functions.iter().find(|f| f.name == "compute").unwrap();
    let stmt_count: usize = func.blocks.iter().map(|b| b.statements.len()).sum();
    assert!(
        stmt_count <= 16,
        "constant array kernel should fold to small MIR, got {} stmts",
        stmt_count
    );

    assert_eq!(compile_and_run_supercompiled(src, "const_array_fold_generic"), 40);
}
