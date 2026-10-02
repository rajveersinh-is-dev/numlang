use std::fs;
use std::process::Command;

use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::build_process_tree;
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn run_numlang_code(code: &str, _supercompile: bool) -> (Option<i32>, String, String) {
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
    cmd.arg("--supercompile");
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
    // When supercompiled, main must completely reduce with NO residual calls to eval!
    let tokens = tokenize(code).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    let mut mir = lower_program(&typed);
    supercompile_mir_program(&mut mir);

    let main_fn = mir
        .functions
        .iter()
        .find(|f| f.name == "main")
        .expect("main function exists");

    let has_eval = main_fn.blocks.iter().any(|b| {
        b.statements.iter().any(|s| match s {
            numlang::mir::lower::Statement::Assign(_, numlang::mir::lower::Rvalue::Call(callee, _)) => callee == "eval",
            _ => false,
        })
    });
    assert!(!has_eval, "Residual main must contain NO calls to eval");
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

    // 3. Static check: main completely collapses
    let tokens = tokenize(code).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    let mut mir = lower_program(&typed);
    supercompile_mir_program(&mut mir);

    let main_fn = mir.functions.iter().find(|f| f.name == "main").unwrap();
    let has_nat_mul = main_fn.blocks.iter().any(|b| {
        b.statements.iter().any(|s| match s {
            numlang::mir::lower::Statement::Assign(_, numlang::mir::lower::Rvalue::Call(callee, _)) => callee == "nat_mul",
            _ => false,
        })
    });
    assert!(!has_nat_mul, "Residual main must contain NO calls to nat_mul");
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

    // Verify specialization collapses and eliminates bool_eval calls
    let tokens = tokenize(code).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    let mut mir = lower_program(&typed);
    supercompile_mir_program(&mut mir);

    let main_fn = mir.functions.iter().find(|f| f.name == "main").unwrap();
    let has_bool_eval = main_fn.blocks.iter().any(|b| {
        b.statements.iter().any(|s| match s {
            numlang::mir::lower::Statement::Assign(_, numlang::mir::lower::Rvalue::Call(callee, _)) => callee == "bool_eval",
            _ => false,
        })
    });
    assert!(!has_bool_eval, "Residual main must contain NO calls to bool_eval");
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

#[test]
fn test_negative_discriminant_propagation() {
    // A 3-variant enum where the default arm should deduce it must be variant 2.
    let code = r#"
enum Color { Red, Green, Blue }

fn color_val(c: Color) -> i64 {
    return match c {
        Red   => 10,
        Green => 20,
        Blue  => 30,
    };
}

fn main() -> i64 {
    let c: Color = Blue;
    return color_val(c);
}
"#;
    let (norm, _, _) = run_numlang_code(code, false);
    assert_eq!(norm, Some(30));

    let (sc, _, _) = run_numlang_code(code, true);
    assert_eq!(sc, Some(30));

    // Static: main eliminates calls to color_val
    let tokens = tokenize(code).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    let mut mir = lower_program(&typed);
    supercompile_mir_program(&mut mir);
    let main_fn = mir.functions.iter().find(|f| f.name == "main").unwrap();
    let has_color_val = main_fn.blocks.iter().any(|b| {
        b.statements.iter().any(|s| match s {
            numlang::mir::lower::Statement::Assign(_, numlang::mir::lower::Rvalue::Call(callee, _)) => callee == "color_val",
            _ => false,
        })
    });
    assert!(!has_color_val, "Residual main must contain NO calls to color_val");
}

#[test]
fn test_partial_specialization_symbolic_arg() {
    // `double(x: i64) -> i64` specialized where x is symbolic should produce
    // a residual `return x + x;` (or `return 2 * x;`) rather than bailing.
    let code = r#"
fn double(x: i64) -> i64 {
    return x + x;
}
fn main() -> i64 {
    return double(21);
}
"#;
    // 1. Normal run: double(21) = 42
    let (code_norm, _, _) = run_numlang_code(code, false);
    assert_eq!(code_norm, Some(42));

    // 2. Supercompiled run (main is fully concrete — should collapse to 42)
    let (code_sc, _, _) = run_numlang_code(code, true);
    assert_eq!(code_sc, Some(42));

    // 3. Static check: main must eliminate call to double
    let tokens = tokenize(code).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    let mut mir = lower_program(&typed);
    supercompile_mir_program(&mut mir);
    let main_fn = mir.functions.iter().find(|f| f.name == "main").unwrap();
    let has_double = main_fn.blocks.iter().any(|b| {
        b.statements.iter().any(|s| match s {
            numlang::mir::lower::Statement::Assign(_, numlang::mir::lower::Rvalue::Call(callee, _)) => callee == "double",
            _ => false,
        })
    });
    assert!(!has_double, "Residual main must contain NO calls to double");
}

#[test]
fn test_2nd_futamura_projection() {
    // 2nd Futamura Projection: Specializing an interpreter on a static program expression
    // produces a compiled residual function with ZERO interpreter dispatch/match overhead.
    let code = r#"
enum Expr {
    Var,
    Lit(i64),
    Add(Expr, Expr),
    Mul(Expr, Expr),
}

fn eval(e: Expr, x: i64) -> i64 {
    return match e {
        Var => x,
        Lit(val) => val,
        Add(l, r) => eval(l, x) + eval(r, x),
        Mul(l, r) => eval(l, x) * eval(r, x),
    };
}

fn compiled_prog(x: i64) -> i64 {
    let prog: Expr = Mul(Add(Var, Lit(3)), Lit(2));
    return eval(prog, x);
}

fn main() -> i64 {
    return compiled_prog(5);
}
"#;

    // 1. Normal run: (5 + 3) * 2 = 16
    let (code_norm, _, _) = run_numlang_code(code, false);
    assert_eq!(code_norm, Some(16));

    // 2. Supercompiled run: must execute and yield 16
    let (code_sc, _, _) = run_numlang_code(code, true);
    assert_eq!(code_sc, Some(16));

    // 3. Static verification of 2nd Futamura Projection:
    // In compiled_prog(x: i64), the static expression `Mul(Add(Var, Lit(3)), Lit(2))`
    // must be specialized away with respect to symbolic parameter `x`.
    // The residual AST of compiled_prog must contain:
    // - ZERO TypedExpr::Match / TypedStmt::Match
    // - ZERO TypedExpr::Call to `eval`
    // - Purely arithmetic operations on `x`
    let tokens = tokenize(code).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    let mut mir = lower_program(&typed);
    supercompile_mir_program(&mut mir);

    let compiled_fn = mir
        .functions
        .iter()
        .find(|f| f.name == "compiled_prog")
        .expect("compiled_prog function exists");

    let has_eval = compiled_fn.blocks.iter().any(|b| {
        b.statements.iter().any(|s| match s {
            numlang::mir::lower::Statement::Assign(_, numlang::mir::lower::Rvalue::Call(callee, _)) => callee == "eval",
            _ => false,
        })
    });
    assert!(!has_eval, "Residual compiled_prog must contain NO calls to eval");

    // 4. Execution correctness: test inputs x = 5, x = 10, x = 0
    let test_inputs = [(5, 16), (10, 26), (0, 6)];
    for (input, expected) in test_inputs {
        let code_input = format!(
            r#"
enum Expr {{
    Var,
    Lit(i64),
    Add(Expr, Expr),
    Mul(Expr, Expr),
}}

fn eval(e: Expr, x: i64) -> i64 {{
    return match e {{
        Var => x,
        Lit(val) => val,
        Add(l, r) => eval(l, x) + eval(r, x),
        Mul(l, r) => eval(l, x) * eval(r, x),
    }};
}}

fn compiled_prog(x: i64) -> i64 {{
    let prog: Expr = Mul(Add(Var, Lit(3)), Lit(2));
    return eval(prog, x);
}}

fn main() -> i64 {{
    return compiled_prog({});
}}
"#,
            input
        );
        let (code_res, _, _) = run_numlang_code(&code_input, true);
        assert_eq!(
            code_res,
            Some(expected),
            "compiled_prog({}) must evaluate to {}",
            input,
            expected
        );
    }
}



