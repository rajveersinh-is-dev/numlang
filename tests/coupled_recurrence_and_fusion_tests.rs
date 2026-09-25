use std::fs;
use std::process::Command;

use numlang::ast::BinaryOp;
use numlang::codegen::linker::link_executable;
use numlang::codegen::compile_supercompiled_to_obj;
use numlang::mir::lower::{lower_program, Rvalue, Statement};
use numlang::mir::supercompiler::generalize::{
    solve_coupled_2var_recurrence, solve_order2_recurrence, solve_order3_recurrence,
};
use numlang::mir::supercompiler::supercompile_mir_program;
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
    let sol10 = solve_order2_recurrence(&samples, n10, &mut interner).expect("Failed to solve F(10)");
    if let SymTerm::ConstInt(val, _) = interner.get(sol10) {
        assert_eq!(*val, 55, "F(10) must be 55");
    } else {
        panic!("Expected ConstInt for F(10)");
    }

    let n20 = interner.intern_int(20);
    let sol20 = solve_order2_recurrence(&samples, n20, &mut interner).expect("Failed to solve F(20)");
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
        other => panic!("Expected Binary Add term for closed form, found: {:?}", other),
    }

    // Now test with concrete n=5: 2^6 - 1 = 63
    let n5 = interner.intern_int(5);
    let sol5 = solve_order2_recurrence(&samples, n5, &mut interner).expect("Failed to solve for n=5");
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
    let sol7 = solve_order3_recurrence(&samples, n7, &mut interner).expect("Failed to solve Tribonacci T(7)");
    if let SymTerm::ConstInt(val, _) = interner.get(sol7) {
        assert_eq!(*val, 13, "T(7) must be 13");
    } else {
        panic!("Expected ConstInt 13");
    }

    let n9 = interner.intern_int(9);
    let sol9 = solve_order3_recurrence(&samples, n9, &mut interner).expect("Failed to solve Tribonacci T(9)");
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
    assert!(!has_call, "Call to square should be completely eliminated via interprocedural inlining");

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

    // Symbolic test
    let place = numlang::mir::Place {
        local: "n".to_string(),
        projections: vec![],
    };
    let n_sym = interner.intern_var(place, Type::I64);
    let (sol_sym_a, sol_sym_b) = solve_coupled_2var_recurrence(&samples_a, &samples_b, n_sym, &mut interner)
        .expect("Failed to solve coupled 2-variable recurrence symbolically");

    match interner.get(sol_sym_a) {
        SymTerm::Call(callee, args, _) => {
            assert_eq!(callee, "__coupled_a");
            assert_eq!(args.len(), 9);
        }
        other => panic!("Expected Call to __coupled_a, found: {:?}", other),
    }
    match interner.get(sol_sym_b) {
        SymTerm::Call(callee, args, _) => {
            assert_eq!(callee, "__coupled_b");
            assert_eq!(args.len(), 9);
        }
        other => panic!("Expected Call to __coupled_b, found: {:?}", other),
    }
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

