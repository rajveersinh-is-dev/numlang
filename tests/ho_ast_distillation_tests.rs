use std::fs;
use std::process::Command;

use numlang::ast::hodistill::{
    distill_program, is_alpha_equivalent, AstProcessTerm,
    AstProcessTree,
};
use numlang::ast::BinaryOp;
use numlang::codegen::linker::link_executable;
use numlang::compiler::{compile_pipeline, CompilerConfig};
use numlang::parser::parse;
use numlang::span::Span;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::typed_ast::{
    TypedBlock, TypedExpr, TypedLiteral, TypedProgram, TypedStmt,
};
use numlang::typecheck::types::Type;

fn get_typed_program(src: &str) -> TypedProgram {
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    typecheck(&ast).expect("Typecheck failed")
}

fn compile_and_run(src: &str, test_name: &str) -> i32 {
    let mut typed = get_typed_program(src);
    let _stats = distill_program(&mut typed);
    numlang::opt::optimize_program(&mut typed);

    let obj_bytes = numlang::codegen::compile_to_obj_with_opt(&typed, false)
        .expect("Codegen failed");

    let test_dir = std::env::temp_dir().join(format!("numlang_hodistill_{}", test_name));
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
fn test_process_tree_representation_and_alpha_equivalence() {
    let mut tree = AstProcessTree::new();
    let span = Span::default();

    // Build Node 1: λ(x: i64). x + 1
    let x_var = tree.alloc(AstProcessTerm::Var("x".to_string(), Type::I64), span, None, 1);
    let one_lit = tree.alloc(
        AstProcessTerm::Lit(TypedLiteral::Int(1, Type::I64), Type::I64),
        span,
        None,
        1,
    );
    let body1 = tree.alloc(
        AstProcessTerm::Binary {
            op: BinaryOp::Add,
            left: x_var,
            right: one_lit,
            ty: Type::I64,
        },
        span,
        None,
        1,
    );
    let lam1 = tree.alloc(
        AstProcessTerm::Lam {
            params: vec![("x".to_string(), Type::I64)],
            body: body1,
            ty: Type::Fn(vec![Type::I64], Box::new(Type::I64)),
        },
        span,
        None,
        0,
    );

    // Build Node 2: λ(y: i64). y + 1 (alpha-equivalent to Node 1 under renaming x <-> y)
    let y_var = tree.alloc(AstProcessTerm::Var("y".to_string(), Type::I64), span, None, 1);
    let one_lit2 = tree.alloc(
        AstProcessTerm::Lit(TypedLiteral::Int(1, Type::I64), Type::I64),
        span,
        None,
        1,
    );
    let body2 = tree.alloc(
        AstProcessTerm::Binary {
            op: BinaryOp::Add,
            left: y_var,
            right: one_lit2,
            ty: Type::I64,
        },
        span,
        None,
        1,
    );
    let lam2 = tree.alloc(
        AstProcessTerm::Lam {
            params: vec![("y".to_string(), Type::I64)],
            body: body2,
            ty: Type::Fn(vec![Type::I64], Box::new(Type::I64)),
        },
        span,
        None,
        0,
    );

    // Build Node 3: λ(z: i64). z + 2 (NOT alpha-equivalent to Node 1)
    let z_var = tree.alloc(AstProcessTerm::Var("z".to_string(), Type::I64), span, None, 1);
    let two_lit = tree.alloc(
        AstProcessTerm::Lit(TypedLiteral::Int(2, Type::I64), Type::I64),
        span,
        None,
        1,
    );
    let body3 = tree.alloc(
        AstProcessTerm::Binary {
            op: BinaryOp::Add,
            left: z_var,
            right: two_lit,
            ty: Type::I64,
        },
        span,
        None,
        1,
    );
    let lam3 = tree.alloc(
        AstProcessTerm::Lam {
            params: vec![("z".to_string(), Type::I64)],
            body: body3,
            ty: Type::Fn(vec![Type::I64], Box::new(Type::I64)),
        },
        span,
        None,
        0,
    );

    let mut map1 = std::collections::HashMap::new();
    let mut map2 = std::collections::HashMap::new();
    assert!(
        is_alpha_equivalent(&tree, lam1, lam2, &mut map1, &mut map2),
        "λx. x+1 must be alpha-equivalent to λy. y+1"
    );

    let mut map3 = std::collections::HashMap::new();
    let mut map4 = std::collections::HashMap::new();
    assert!(
        !is_alpha_equivalent(&tree, lam1, lam3, &mut map3, &mut map4),
        "λx. x+1 must not be alpha-equivalent to λz. z+2"
    );
}

#[test]
fn test_compose_5_deep_chain_deforestation() {
    let src = r#"
    fn add1(x: i64) -> i64 {
        return x + 1;
    }

    fn mul2(x: i64) -> i64 {
        return x * 2;
    }

    fn add3(x: i64) -> i64 {
        return x + 3;
    }

    fn sub5(x: i64) -> i64 {
        return x - 5;
    }

    fn add10(x: i64) -> i64 {
        return x + 10;
    }

    fn compose(f: fn(i64) -> i64, g: fn(i64) -> i64) -> fn(i64) -> i64 {
        return |x: i64| f(g(x));
    }

    fn run_chain(val: i64) -> i64 {
        let c4 = compose(sub5, add10);
        let c3 = compose(add3, c4);
        let c2 = compose(mul2, c3);
        let c1 = compose(add1, c2);
        return c1(val);
    }

    fn main() -> i64 {
        let ans: i64 = run_chain(4);
        // Calculation trace:
        // add10(4) = 14
        // sub5(14) = 9
        // add3(9) = 12
        // mul2(12) = 24
        // add1(24) = 25
        if ans == 25 {
            return 0;
        }
        return 1;
    }
    "#;

    let mut typed = get_typed_program(src);
    let stats = distill_program(&mut typed);

    // 5-deep compose chain should eliminate closures and perform beta-reductions
    assert!(
        stats.closures_eliminated >= 4,
        "Distillation must eliminate at least 4 intermediate closures, eliminated: {}",
        stats.closures_eliminated
    );
    assert!(
        stats.beta_reductions >= 4,
        "Distillation must perform at least 4 beta-reductions, performed: {}",
        stats.beta_reductions
    );

    // Inspect the run_chain function AST to verify that compose calls were eliminated
    let run_chain_fn = typed
        .functions
        .iter()
        .find(|f| f.name == "run_chain")
        .expect("run_chain function must exist");

    let count_compose_calls = count_callee_calls(&run_chain_fn.body, "compose");
    assert_eq!(
        count_compose_calls, 0,
        "Distilled run_chain must contain zero calls to compose"
    );

    // Verify end-to-end execution
    let exit_code = compile_and_run(src, "compose_5_deep");
    assert_eq!(exit_code, 0, "5-deep compose must execute correctly and return 0");
}

#[test]
fn test_mutual_recursion_distillation() {
    let src = r#"
    fn mutual_a(n: i64, acc: i64) -> i64 {
        if n <= 0 {
            return acc;
        }
        return mutual_b(n - 1, acc + 1);
    }

    fn mutual_b(n: i64, acc: i64) -> i64 {
        if n <= 0 {
            return acc;
        }
        return mutual_c(n - 1, acc * 2);
    }

    fn mutual_c(n: i64, acc: i64) -> i64 {
        if n <= 0 {
            return acc;
        }
        return mutual_a(n - 1, acc + 3);
    }

    fn main() -> i64 {
        let ans: i64 = mutual_a(6, 1);
        if ans > 0 {
            return 0;
        }
        return 1;
    }
    "#;

    let mut typed = get_typed_program(src);
    let stats = distill_program(&mut typed);

    assert!(
        stats.folds_performed >= 1,
        "Inter-procedural mutual recursion must perform at least 1 fold, performed: {}",
        stats.folds_performed
    );

    let exit_code = compile_and_run(src, "mutual_recursion");
    assert_eq!(exit_code, 0, "Mutual recursion program must execute correctly and return 0");
}

#[test]
fn test_zero_intermediate_closure_returning_applications() {
    let src = r#"
    fn step(x: i64) -> i64 {
        return x * 3;
    }

    fn make_pipeline(f: fn(i64) -> i64) -> fn(i64) -> i64 {
        return |x: i64| f(f(x));
    }

    fn apply_pipeline(val: i64) -> i64 {
        let p = make_pipeline(step);
        return p(val);
    }

    fn main() -> i64 {
        let res: i64 = apply_pipeline(2);
        // step(2) = 6, step(6) = 18
        if res == 18 {
            return 0;
        }
        return 1;
    }
    "#;

    let mut typed = get_typed_program(src);
    let stats = distill_program(&mut typed);

    assert!(stats.closures_eliminated >= 1);
    let apply_pipeline_fn = typed
        .functions
        .iter()
        .find(|f| f.name == "apply_pipeline")
        .expect("apply_pipeline function must exist");

    // Verify zero calls to make_pipeline remain in apply_pipeline
    let calls_to_make = count_callee_calls(&apply_pipeline_fn.body, "make_pipeline");
    assert_eq!(calls_to_make, 0);

    let exit_code = compile_and_run(src, "zero_intermediate_closures");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_compiler_pipeline_with_ho_distill_flag() {
    let src = r#"
    fn inc(x: i64) -> i64 {
        return x + 1;
    }

    fn wrap(f: fn(i64) -> i64) -> fn(i64) -> i64 {
        return |y: i64| f(y);
    }

    fn run(val: i64) -> i64 {
        let w = wrap(inc);
        return w(val);
    }
    "#;

    let mut typed = get_typed_program(src);
    let config = CompilerConfig {
        ho_distill: true,
        supercompile: true,
        ..Default::default()
    };
    let (mir, stats) = compile_pipeline(&mut typed, &config);

    assert!(stats.closures_eliminated >= 1);
    assert!(!mir.functions.is_empty(), "Pipeline must lower to MIR functions");
}

fn count_callee_calls(block: &TypedBlock, target_callee: &str) -> usize {
    let mut count = 0;
    for stmt in &block.stmts {
        match stmt {
            TypedStmt::Let { value, .. } => {
                count += count_calls_in_expr(value, target_callee);
            }
            TypedStmt::Assign { value, .. } => {
                count += count_calls_in_expr(value, target_callee);
            }
            TypedStmt::Return(Some(expr), _) => {
                count += count_calls_in_expr(expr, target_callee);
            }
            TypedStmt::Expr(expr) => {
                count += count_calls_in_expr(expr, target_callee);
            }
            TypedStmt::If { condition, then_branch, else_branch, .. } => {
                count += count_calls_in_expr(condition, target_callee);
                count += count_callee_calls(then_branch, target_callee);
                if let Some(ref eb) = else_branch {
                    count += count_callee_calls(eb, target_callee);
                }
            }
            _ => {}
        }
    }
    count
}

fn count_calls_in_expr(expr: &TypedExpr, target_callee: &str) -> usize {
    match expr {
        TypedExpr::Call { callee, args, .. } => {
            let mut c = if callee == target_callee { 1 } else { 0 };
            for a in args {
                c += count_calls_in_expr(a, target_callee);
            }
            c
        }
        TypedExpr::CallIndirect { callee, args, .. } => {
            let mut c = count_calls_in_expr(callee, target_callee);
            for a in args {
                c += count_calls_in_expr(a, target_callee);
            }
            c
        }
        TypedExpr::Binary { left, right, .. } => {
            count_calls_in_expr(left, target_callee) + count_calls_in_expr(right, target_callee)
        }
        TypedExpr::Unary { expr, .. } => count_calls_in_expr(expr, target_callee),
        TypedExpr::Lambda { body, .. } => count_calls_in_expr(body, target_callee),
        _ => 0,
    }
}
