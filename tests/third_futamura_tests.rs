use std::fs;
use std::process::Command;

use numlang::opt::supercompiler::supercompile_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::typed_ast::{TypedBlock, TypedExpr, TypedLiteral, TypedStmt};

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
    let test_dir = std::env::temp_dir().join(format!("numlang_futamura3_{}", id));
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

const META_DEFS: &str = include_str!("../src/stdlib/meta.nl");

fn contains_interpreter_overhead(expr: &TypedExpr) -> bool {
    match expr {
        TypedExpr::Match { .. } => true,
        TypedExpr::Call { callee, .. }
            if callee == "min_eval"
                || callee == "env_lookup"
                || callee == "env_lookup_helper"
                || callee == "eval_op"
                || callee == "eval_if" =>
        {
            true
        }
        TypedExpr::Binary { left, right, .. } => {
            contains_interpreter_overhead(left) || contains_interpreter_overhead(right)
        }
        TypedExpr::Unary { expr, .. } => contains_interpreter_overhead(expr),
        _ => false,
    }
}

fn block_contains_interpreter_overhead(block: &TypedBlock) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        TypedStmt::Return(Some(expr), _) => contains_interpreter_overhead(expr),
        TypedStmt::Expr(expr) => contains_interpreter_overhead(expr),
        TypedStmt::Let { value, .. } => contains_interpreter_overhead(value),
        TypedStmt::Assign { value, .. } => contains_interpreter_overhead(value),
        TypedStmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            contains_interpreter_overhead(condition)
                || block_contains_interpreter_overhead(then_branch)
                || else_branch
                    .as_ref()
                    .is_some_and(block_contains_interpreter_overhead)
        }
        TypedStmt::While {
            condition, body, ..
        } => {
            contains_interpreter_overhead(condition)
                || block_contains_interpreter_overhead(body)
        }
        _ => false,
    })
}

#[test]
fn test_1st_futamura_interpreter_specialization() {
    let code = format!(
        r#"
{}

fn main() -> i64 {{
    let prog: MetaExpr = MetaExpr::Bin(
        MetaOp::Mul,
        box(MetaExpr::Bin(MetaOp::Add, box(MetaExpr::Lit(5)), box(MetaExpr::Lit(3)))),
        box(MetaExpr::Bin(MetaOp::Sub, box(MetaExpr::Lit(10)), box(MetaExpr::Lit(2))))
    );
    let env: MetaEnv = MetaEnv::Nil;
    return min_eval(prog, env);
}}
"#,
        META_DEFS
    );

    // 1. Normal run: (5 + 3) * (10 - 2) = 8 * 8 = 64
    let (code_norm, _, stderr_norm) = run_numlang_code(&code, false);
    assert_eq!(code_norm, Some(64), "stderr norm: {}", stderr_norm);

    // 2. Supercompiled run: must execute and yield 64
    let (code_sc, _, stderr_sc) = run_numlang_code(&code, true);
    assert_eq!(code_sc, Some(64), "stderr sc: {}", stderr_sc);

    // 3. Static verification: main must collapse to `return 64;` with zero min_eval calls
    let tokens = tokenize(&code).unwrap();
    let program = parse(&tokens).unwrap();
    let mut typed = typecheck(&program).unwrap();

    supercompile_program(&mut typed, None);

    let main_fn = typed
        .functions
        .iter()
        .find(|f| f.name == "main")
        .expect("main function exists");

    assert_eq!(main_fn.body.stmts.len(), 1);
    match &main_fn.body.stmts[0] {
        TypedStmt::Return(Some(TypedExpr::Literal { lit, .. }), _) => {
            assert_eq!(
                *lit,
                TypedLiteral::Int(64, numlang::typecheck::types::Type::I64)
            );
        }
        other => panic!("Expected return 64, got: {:?}", other),
    }
}

#[test]
fn test_2nd_futamura_compiler_generation() {
    let code = format!(
        r#"
{}

fn compiled_prog(x: i64, y: i64) -> i64 {{
    let prog: MetaExpr = MetaExpr::Bin(
        MetaOp::Mul,
        box(MetaExpr::Bin(MetaOp::Add, box(MetaExpr::Var(1)), box(MetaExpr::Lit(5)))),
        box(MetaExpr::Var(2))
    );
    let env: MetaEnv = MetaEnv::Cons(1, x, box(MetaEnv::Cons(2, y, box(MetaEnv::Nil))));
    return min_eval(prog, env);
}}

fn main() -> i64 {{
    return compiled_prog(3, 4);
}}
"#,
        META_DEFS
    );

    // 1. Normal run: (3 + 5) * 4 = 32
    let (code_norm, _, stderr_norm) = run_numlang_code(&code, false);
    assert_eq!(code_norm, Some(32), "stderr norm: {}", stderr_norm);

    // 2. Supercompiled run
    let (code_sc, _, stderr_sc) = run_numlang_code(&code, true);
    assert_eq!(code_sc, Some(32), "stderr sc: {}", stderr_sc);

    // 3. Static verification of 2nd Futamura Projection:
    // `compiled_prog(x: i64, y: i64)` must be specialized into direct arithmetic
    // with ZERO interpreter dispatch/calls
    let tokens = tokenize(&code).unwrap();
    let program = parse(&tokens).unwrap();
    let mut typed = typecheck(&program).unwrap();

    supercompile_program(&mut typed, None);

    let compiled_fn = typed
        .functions
        .iter()
        .find(|f| f.name == "compiled_prog")
        .expect("compiled_prog exists");

    assert!(
        !block_contains_interpreter_overhead(&compiled_fn.body),
        "Residual compiled_prog must be completely free of interpreter dispatch/calls! Found: {:#?}",
        compiled_fn.body
    );

    // 4. Test multiple inputs with supercompiled code
    let test_inputs = [(3, 4, 32), (10, 2, 30), (0, 7, 35)];
    for (x, y, expected) in test_inputs {
        let code_input = format!(
            r#"
{}

fn compiled_prog(x: i64, y: i64) -> i64 {{
    let prog: MetaExpr = MetaExpr::Bin(
        MetaOp::Mul,
        box(MetaExpr::Bin(MetaOp::Add, box(MetaExpr::Var(1)), box(MetaExpr::Lit(5)))),
        box(MetaExpr::Var(2))
    );
    let env: MetaEnv = MetaEnv::Cons(1, x, box(MetaEnv::Cons(2, y, box(MetaEnv::Nil))));
    return min_eval(prog, env);
}}

fn main() -> i64 {{
    return compiled_prog({}, {});
}}
"#,
            META_DEFS, x, y
        );
        let (code_res, _, _) = run_numlang_code(&code_input, true);
        assert_eq!(
            code_res,
            Some(expected),
            "compiled_prog({}, {}) must evaluate to {}",
            x,
            y,
            expected
        );
    }
}

#[test]
fn test_3rd_futamura_cogen_generation() {
    // 3rd Futamura Projection:
    // cogen = spec(spec, spec)
    // cogen(interp) = compiler
    // compiler(prog) = target_prog
    // target_prog(data) = result
    let code = format!(
        r#"
{}

fn interp_spec(interp_ast: MetaExpr, prog_id: i64, x: i64) -> i64 {{
    let env: MetaEnv = MetaEnv::Cons(1, prog_id, box(MetaEnv::Cons(2, x, box(MetaEnv::Nil))));
    return min_eval(interp_ast, env);
}}

// cogen applied to interpreter AST: produces a compiler function
fn compiled_vm_prog(x: i64) -> i64 {{
    // Interpreter AST:
    // If Var(1) == 1 (Program 1: double-add-ten):
    //     (x + 10) * 2
    // Else (Program 2: polynomial):
    //     (x * 3) + 7
    let interp_ast: MetaExpr = MetaExpr::If(
        box(MetaExpr::Bin(MetaOp::Eq, box(MetaExpr::Var(1)), box(MetaExpr::Lit(1)))),
        box(MetaExpr::Bin(
            MetaOp::Mul,
            box(MetaExpr::Bin(MetaOp::Add, box(MetaExpr::Var(2)), box(MetaExpr::Lit(10)))),
            box(MetaExpr::Lit(2))
        )),
        box(MetaExpr::Bin(
            MetaOp::Add,
            box(MetaExpr::Bin(MetaOp::Mul, box(MetaExpr::Var(2)), box(MetaExpr::Lit(3)))),
            box(MetaExpr::Lit(7))
        ))
    );
    // Compile Program 1 with input x
    return interp_spec(interp_ast, 1, x);
}}

fn main() -> i64 {{
    return compiled_vm_prog(5);
}}
"#,
        META_DEFS
    );

    // 1. Normal execution: (5 + 10) * 2 = 30
    let (code_norm, _, stderr_norm) = run_numlang_code(&code, false);
    assert_eq!(code_norm, Some(30), "stderr norm: {}", stderr_norm);

    // 2. Supercompiled execution: 30
    let (code_sc, _, stderr_sc) = run_numlang_code(&code, true);
    assert_eq!(code_sc, Some(30), "stderr sc: {}", stderr_sc);

    // 3. Static verification of 3rd Futamura projection:
    // The residual function compiled_vm_prog must contain:
    // - ZERO calls to min_eval / interp_spec
    // - ZERO calls to env_lookup
    // - ZERO Match expressions
    // - ZERO If branches (the interpreter branch was eliminated!)
    // Pure arithmetic: (x + 10) * 2
    let tokens = tokenize(&code).unwrap();
    let program = parse(&tokens).unwrap();
    let mut typed = typecheck(&program).unwrap();

    supercompile_program(&mut typed, None);

    let compiled_fn = typed
        .functions
        .iter()
        .find(|f| f.name == "compiled_vm_prog")
        .expect("compiled_vm_prog exists");

    assert!(
        !block_contains_interpreter_overhead(&compiled_fn.body),
        "Residual compiled_vm_prog must be completely free of interpreter dispatch/calls! Found: {:#?}",
        compiled_fn.body
    );

    // Assert that the body does NOT contain any If statements
    fn block_contains_if(block: &TypedBlock) -> bool {
        block.stmts.iter().any(|stmt| matches!(stmt, TypedStmt::If { .. }))
    }
    assert!(
        !block_contains_if(&compiled_fn.body),
        "Residual compiled_vm_prog must have resolved the interpreter branch at compile-time! Found: {:#?}",
        compiled_fn.body
    );

    // 4. Verify on multiple inputs: x = 0 -> 20, x = 5 -> 30, x = 10 -> 40
    let test_inputs = [(0, 20), (5, 30), (10, 40)];
    for (x, expected) in test_inputs {
        let code_input = format!(
            r#"
{}

fn interp_spec(interp_ast: MetaExpr, prog_id: i64, x: i64) -> i64 {{
    let env: MetaEnv = MetaEnv::Cons(1, prog_id, box(MetaEnv::Cons(2, x, box(MetaEnv::Nil))));
    return min_eval(interp_ast, env);
}}

fn compiled_vm_prog(x: i64) -> i64 {{
    let interp_ast: MetaExpr = MetaExpr::If(
        box(MetaExpr::Bin(MetaOp::Eq, box(MetaExpr::Var(1)), box(MetaExpr::Lit(1)))),
        box(MetaExpr::Bin(
            MetaOp::Mul,
            box(MetaExpr::Bin(MetaOp::Add, box(MetaExpr::Var(2)), box(MetaExpr::Lit(10)))),
            box(MetaExpr::Lit(2))
        )),
        box(MetaExpr::Bin(
            MetaOp::Add,
            box(MetaExpr::Bin(MetaOp::Mul, box(MetaExpr::Var(2)), box(MetaExpr::Lit(3)))),
            box(MetaExpr::Lit(7))
        ))
    );
    return interp_spec(interp_ast, 1, x);
}}

fn main() -> i64 {{
    return compiled_vm_prog({});
}}
"#,
            META_DEFS, x
        );
        let (code_res, _, _) = run_numlang_code(&code_input, true);
        assert_eq!(
            code_res,
            Some(expected),
            "compiled_vm_prog({}) must evaluate to {}",
            x,
            expected
        );
    }
}

