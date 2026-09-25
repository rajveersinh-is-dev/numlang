use std::fs;
use std::process::Command;

use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::build_process_tree;
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::opt::supercompiler::supercompile_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::typed_ast::{TypedExpr, TypedLiteral, TypedStmt};

fn run_numlang_code(code: &str, supercompile: bool) -> (Option<i32>, String, String) {
    let id = format!(
        "{}_{:?}_{}",
        std::process::id(),
        std::thread::current().id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
    .replace(['(', ')', ' '], "_");
    let test_dir = std::env::temp_dir().join(format!("numlang_futamura_{}", id));
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run");
    if supercompile {
        cmd.arg("--supercompile");
    }
    cmd.arg(&src_file);

    let output = cmd.output().expect("Failed to run numlang program");

    let _ = fs::remove_dir_all(&test_dir);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.code(), stdout, stderr)
}

#[test]
fn test_1st_futamura_arithmetic_ast_interpreter() {
    let code = r#"
enum Expr {
    Lit(i64),
    Add(Expr, Expr),
    Sub(Expr, Expr),
    Mul(Expr, Expr),
}

fn eval(e: Expr) -> i64 {
    return match e {
        Lit(val) => val,
        Add(l, r) => eval(l) + eval(r),
        Sub(l, r) => eval(l) - eval(r),
        Mul(l, r) => eval(l) * eval(r),
    };
}

fn main() -> i64 {
    let expr: Expr = Mul(Add(Lit(10), Lit(5)), Sub(Lit(8), Lit(6)));
    return eval(expr);
}
"#;
    // 1. Direct runtime execution: (10 + 5) * (8 - 6) = 15 * 2 = 30
    let (code_norm, _, _) = run_numlang_code(code, false);
    assert_eq!(code_norm, Some(30));

    // 2. Supercompiled runtime execution
    let (code_sc, _, _) = run_numlang_code(code, true);
    assert_eq!(code_sc, Some(30));

    // 3. Static verification of 1st Futamura Projection:
    // When supercompiled, main must completely reduce to `return 30;` with NO residual calls to eval!
    let tokens = tokenize(code).unwrap();
    let program = parse(&tokens).unwrap();
    let mut typed = typecheck(&program).unwrap();

    supercompile_program(&mut typed, None);

    let main_fn = typed
        .functions
        .iter()
        .find(|f| f.name == "main")
        .expect("main function exists");

    // Body must contain exactly one return statement with literal 30
    assert_eq!(main_fn.body.stmts.len(), 1);
    match &main_fn.body.stmts[0] {
        TypedStmt::Return(Some(TypedExpr::Literal { lit, .. }), _) => {
            assert_eq!(*lit, TypedLiteral::Int(30, numlang::typecheck::types::Type::I64));
        }
        other => panic!("Expected direct literal return in specialized main, got: {:?}", other),
    }
}

#[test]
fn test_1st_futamura_peano_inductive_numbers() {
    let code = r#"
enum Nat {
    Zero,
    Succ(Nat),
}

fn nat_to_int(n: Nat) -> i64 {
    return match n {
        Zero => 0,
        Succ(prev) => 1 + nat_to_int(prev),
    };
}

fn nat_mul(n: Nat, factor: i64) -> i64 {
    return match n {
        Zero => 0,
        Succ(prev) => factor + nat_mul(prev, factor),
    };
}

fn main() -> i64 {
    let five: Nat = Succ(Succ(Succ(Succ(Succ(Zero)))));
    let val: i64 = nat_to_int(five);
    let three: Nat = Succ(Succ(Succ(Zero)));
    return nat_mul(three, val);
}
"#;
    // 1. Normal run: 3 * 5 = 15
    let (code_norm, _, _) = run_numlang_code(code, false);
    assert_eq!(code_norm, Some(15));

    // 2. Supercompiled run
    let (code_sc, _, _) = run_numlang_code(code, true);
    assert_eq!(code_sc, Some(15));

    // 3. Static check: main completely collapses to `return 15;`
    let tokens = tokenize(code).unwrap();
    let program = parse(&tokens).unwrap();
    let mut typed = typecheck(&program).unwrap();

    supercompile_program(&mut typed, None);

    let main_fn = typed.functions.iter().find(|f| f.name == "main").unwrap();
    assert_eq!(main_fn.body.stmts.len(), 1);
    match &main_fn.body.stmts[0] {
        TypedStmt::Return(Some(TypedExpr::Literal { lit, .. }), _) => {
            assert_eq!(*lit, TypedLiteral::Int(15, numlang::typecheck::types::Type::I64));
        }
        other => panic!("Expected direct literal 15 return, got: {:?}", other),
    }
}

#[test]
fn test_1st_futamura_boolean_tree_evaluator() {
    let code = r#"
enum BoolExpr {
    BLit(bool),
    BAnd(BoolExpr, BoolExpr),
    BOr(BoolExpr, BoolExpr),
}

fn beval(b: BoolExpr) -> bool {
    return match b {
        BLit(v) => v,
        BAnd(l, r) => match beval(l) {
            true => beval(r),
            false => false,
        },
        BOr(l, r) => match beval(l) {
            true => true,
            false => beval(r),
        },
    };
}

fn main() -> i64 {
    let expr: BoolExpr = BAnd(BOr(BLit(false), BLit(true)), BLit(true));
    let res: bool = beval(expr);
    return match res {
        true => 1,
        false => 0,
    };
}
"#;
    let (code_norm, _, _) = run_numlang_code(code, false);
    assert_eq!(code_norm, Some(1));

    let (code_sc, _, _) = run_numlang_code(code, true);
    assert_eq!(code_sc, Some(1));

    // Verify specialization collapses to 1
    let tokens = tokenize(code).unwrap();
    let program = parse(&tokens).unwrap();
    let mut typed = typecheck(&program).unwrap();

    supercompile_program(&mut typed, None);

    let main_fn = typed.functions.iter().find(|f| f.name == "main").unwrap();
    assert_eq!(main_fn.body.stmts.len(), 1);
    match &main_fn.body.stmts[0] {
        TypedStmt::Return(Some(TypedExpr::Literal { lit, .. }), _) => {
            assert_eq!(*lit, TypedLiteral::Int(1, numlang::typecheck::types::Type::I64));
        }
        other => panic!("Expected return 1 in specialized main, got: {:?}", other),
    }
}

#[test]
fn test_mir_switch_and_discriminant_supercompilation() {
    let code = r#"
enum Shape {
    Circle(i64),
    Rect(i64, i64),
}

fn area(s: Shape) -> i64 {
    return match s {
        Circle(r) => 3 * r * r,
        Rect(w, h) => w * h,
    };
}

fn main() -> i64 {
    let s: Shape = Rect(7, 6);
    return area(s);
}
"#;
    let tokens = tokenize(code).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    let mut mir = lower_program(&typed);

    // Verify MIR contains Discriminant and Switch
    let area_fn = mir.functions.iter().find(|f| f.name == "area").unwrap();
    let has_discriminant = area_fn.blocks.iter().any(|b| {
        b.statements.iter().any(|stmt| {
            matches!(stmt, numlang::mir::lower::Statement::Assign(_, numlang::mir::lower::Rvalue::Discriminant(_)))
        })
    });
    assert!(has_discriminant, "MIR for match must use Rvalue::Discriminant");

    let has_switch = area_fn.blocks.iter().any(|b| {
        matches!(&b.terminator, numlang::mir::Terminator::Switch { .. })
    });
    assert!(has_switch, "MIR for match must terminate with Switch");

    // Build process tree for main and verify branches are pruned
    let main_mir = mir.functions.iter().find(|f| f.name == "main").unwrap();
    let tree = build_process_tree(main_mir);
    assert!(tree.stats.nodes_explored > 0);

    // Run full MIR supercompiler pass
    let stats = supercompile_mir_program(&mut mir);
    assert!(stats.nodes_explored > 0);

    // Verify execution of the program
    let (code_norm, _, _) = run_numlang_code(code, false);
    assert_eq!(code_norm, Some(42));

    let (code_sc, _, _) = run_numlang_code(code, true);
    assert_eq!(code_sc, Some(42));
}
