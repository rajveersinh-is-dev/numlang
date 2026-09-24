use std::fs;
use std::process::Command;

use numlang::ast::BinaryOp;
use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::generalize::solve_recurrence;
use numlang::mir::supercompiler::state::PathConstraintStore;
use numlang::mir::supercompiler::term::{SymTerm, TermInterner};
use numlang::mir::supercompiler::whistle::is_embedded;
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::mir::{BasicBlockId, Place};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::types::Type;

fn get_mir(source: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(source).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    lower_program(&typed)
}

#[test]
fn test_symbolic_term_simplification_and_hashcons() {
    let mut interner = TermInterner::new();

    let x_place = Place {
        local: "x".to_string(),
        projections: vec![],
    };
    let x = interner.intern_var(x_place, Type::I64);
    let zero = interner.intern_int(0);
    let one = interner.intern_int(1);

    // x + 0 => x
    let add_zero = interner.intern_binary(BinaryOp::Add, x, zero, Type::I64);
    assert_eq!(add_zero, x);

    // 0 + x => x
    let zero_add = interner.intern_binary(BinaryOp::Add, zero, x, Type::I64);
    assert_eq!(zero_add, x);

    // x * 1 => x
    let mul_one = interner.intern_binary(BinaryOp::Mul, x, one, Type::I64);
    assert_eq!(mul_one, x);

    // x * 0 => 0
    let mul_zero = interner.intern_binary(BinaryOp::Mul, x, zero, Type::I64);
    assert_eq!(mul_zero, zero);

    // x - x => 0
    let sub_self = interner.intern_binary(BinaryOp::Sub, x, x, Type::I64);
    assert_eq!(sub_self, zero);

    // x == x => true
    let eq_self = interner.intern_binary(BinaryOp::Eq, x, x, Type::Bool);
    assert_eq!(interner.get(eq_self), &SymTerm::ConstBool(true));

    // x != x => false
    let ne_self = interner.intern_binary(BinaryOp::Ne, x, x, Type::Bool);
    assert_eq!(interner.get(ne_self), &SymTerm::ConstBool(false));
}

#[test]
fn test_path_constraint_propagation() {
    let mut interner = TermInterner::new();
    let mut constraints = PathConstraintStore::new();

    let x_place = Place {
        local: "x".to_string(),
        projections: vec![],
    };
    let x = interner.intern_var(x_place, Type::I64);
    let y_place = Place {
        local: "y".to_string(),
        projections: vec![],
    };
    let y = interner.intern_var(y_place, Type::I64);

    let eq_cond = interner.intern_binary(BinaryOp::Eq, x, y, Type::Bool);
    constraints.add_condition(eq_cond, true, &interner);

    assert_eq!(constraints.evaluate_condition(eq_cond, &interner), Some(true));

    let z_place = Place {
        local: "z".to_string(),
        projections: vec![],
    };
    let z = interner.intern_var(z_place, Type::I64);
    let other_cond = interner.intern_binary(BinaryOp::Eq, x, z, Type::Bool);
    assert_eq!(constraints.evaluate_condition(other_cond, &interner), None);
}

#[test]
fn test_homeomorphic_embedding_growth_detection() {
    let mut interner = TermInterner::new();

    let x_place = Place {
        local: "x".to_string(),
        projections: vec![],
    };
    let x = interner.intern_var(x_place, Type::I64);
    let one = interner.intern_int(1);

    // t1 = x, t2 = x + 1 => x embeds in (x + 1)
    let x_plus_1 = interner.intern_binary(BinaryOp::Add, x, one, Type::I64);
    assert!(is_embedded(x, x_plus_1, &interner));

    // Nested: x + 1 embeds in (x + 1) + 1
    let x_plus_2 = interner.intern_binary(BinaryOp::Add, x_plus_1, one, Type::I64);
    assert!(is_embedded(x_plus_1, x_plus_2, &interner));
    assert!(is_embedded(x, x_plus_2, &interner));
}

#[test]
fn test_recurrence_solver_linear_and_triangular() {
    let mut interner = TermInterner::new();
    let n_place = Place {
        local: "n".to_string(),
        projections: vec![],
    };
    let n = interner.intern_var(n_place, Type::I64);

    // Linear sequence: 0, 5, 10, 15 => 5 * n
    let linear_samples = vec![0, 5, 10, 15];
    let linear_sol = solve_recurrence(&linear_samples, n, &mut interner);
    assert!(linear_sol.is_some());

    // Triangular sequence: 0, 1, 3, 6, 10 => n * (n + 1) / 2
    let tri_samples = vec![0, 1, 3, 6, 10];
    let tri_sol = solve_recurrence(&tri_samples, n, &mut interner);
    assert!(tri_sol.is_some());
}

#[test]
fn test_mir_supercompile_program_execution() {
    let code = r#"
    fn add_symbolic(a: i64, b: i64) -> i64 {
        let x: i64 = a + 0;
        let y: i64 = b * 1;
        return x + y;
    }

    fn loop_sum(n: i64) -> i64 {
        let mut s: i64 = 0;
        let mut i: i64 = 0;
        while i < n {
            s = s + 1;
            i = i + 1;
        }
        return s;
    }
    "#;

    let mut mir_program = get_mir(code);
    supercompile_mir_program(&mut mir_program);

    assert_eq!(mir_program.functions.len(), 2);
    for func in &mir_program.functions {
        assert!(!func.blocks.is_empty());
        assert_eq!(func.blocks[0].id, BasicBlockId(0));
    }
}

#[test]
fn test_cli_emit_supercompiled_mir() {
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join(format!(
        "test_cli_emit_sc_{}_{}.nl",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let code = r#"
    fn sum_to_n(n: i64) -> i64 {
        let mut acc: i64 = 0;
        let mut k: i64 = 0;
        while k < n {
            acc = acc + k;
            k = k + 1;
        }
        return acc;
    }
    "#;

    fs::write(&test_file, code).expect("Failed to write test file");

    let bin = env!("CARGO_BIN_EXE_numlang");
    let output = Command::new(bin)
        .arg("--emit-supercompiled-mir")
        .arg(&test_file)
        .output()
        .expect("Failed to execute numlang with --emit-supercompiled-mir");

    let _ = fs::remove_file(&test_file);

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("sum_to_n"),
        "Output should contain function name: {}",
        stdout
    );
    assert!(
        stdout.contains("BasicBlockId"),
        "Output should contain basic block identifiers: {}",
        stdout
    );
}

#[test]
fn test_recurrence_solver_cubic_and_geometric() {
    let mut interner = TermInterner::new();
    let n_place = Place {
        local: "n".to_string(),
        projections: vec![],
    };
    let n = interner.intern_var(n_place, Type::I64);

    // Cubic sequence: k^3 => [0, 1, 8, 27, 64]
    let cubic_samples = vec![0, 1, 8, 27, 64];
    let cubic_sol = solve_recurrence(&cubic_samples, n, &mut interner);
    assert!(cubic_sol.is_some());

    // Geometric sequence: 3 * 2^k => [3, 6, 12, 24]
    let geo_samples = vec![3, 6, 12, 24];
    let geo_sol = solve_recurrence(&geo_samples, n, &mut interner);
    assert!(geo_sol.is_some());
}

#[test]
fn test_symbolic_closed_form_helpers() {
    use numlang::mir::supercompiler::generalize::{
        solve_symbolic_accumulator, solve_symbolic_geometric, solve_symbolic_linear_induction,
    };

    let mut interner = TermInterner::new();
    let init = interner.intern_int(0);
    let step = interner.intern_int(1);
    let n_place = Place {
        local: "n".to_string(),
        projections: vec![],
    };
    let n = interner.intern_var(n_place, Type::I64);

    // Linear induction
    let lin = solve_symbolic_linear_induction(init, step, n, &mut interner);
    assert_ne!(lin, init);

    // Accumulator
    let acc = solve_symbolic_accumulator(init, init, step, n, &mut interner);
    assert_ne!(acc, init);

    // Geometric
    let ratio = interner.intern_int(2);
    let start = interner.intern_int(5);
    let geo = solve_symbolic_geometric(start, ratio, n, &mut interner);
    assert_ne!(geo, start);
}

#[test]
fn test_cli_emit_process_tree() {
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join(format!(
        "test_cli_emit_pt_{}_{}.nl",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let code = r#"
    fn test_tree(a: i64) -> i64 {
        if a > 10 {
            return 100;
        } else {
            return 200;
        }
    }
    "#;

    fs::write(&test_file, code).expect("Failed to write test file");

    let bin = env!("CARGO_BIN_EXE_numlang");
    let output = Command::new(bin)
        .arg("--emit-process-tree")
        .arg(&test_file)
        .output()
        .expect("Failed to execute numlang with --emit-process-tree");

    let _ = fs::remove_file(&test_file);

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Process Tree:"),
        "Output should contain Process Tree header: {}",
        stdout
    );
    assert!(
        stdout.contains("Node #0:"),
        "Output should contain root node: {}",
        stdout
    );
}

#[test]
fn test_cli_supercompile_stats() {
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join(format!(
        "test_cli_sc_stats_{}_{}.nl",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let code = r#"
    fn compute(n: i64) -> i64 {
        let mut sum: i64 = 0;
        let mut i: i64 = 0;
        while i < n {
            sum = sum + i;
            i = i + 1;
        }
        return sum;
    }
    "#;

    fs::write(&test_file, code).expect("Failed to write test file");

    let bin = env!("CARGO_BIN_EXE_numlang");
    let output = Command::new(bin)
        .arg("--supercompile-stats")
        .arg(&test_file)
        .output()
        .expect("Failed to execute numlang with --supercompile-stats");

    let _ = fs::remove_file(&test_file);

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("nodes:"),
        "Output should contain stats: {}",
        stdout
    );
}
