use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::compact::{compact_mir_function, compact_process_tree};
use numlang::mir::supercompiler::drive::SupercompilerDriver;
use numlang::mir::supercompiler::residualize::residualize_process_tree;
use numlang::mir::supercompiler::state::Interval;
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::mir::{BasicBlockId, Place, Terminator};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use std::fs;
use std::process::Command;

fn run_numlang_code(code: &str, supercompile: bool) -> (Option<i32>, String, String) {
    let id = format!(
        "{}_{:?}_{}",
        std::process::id(),
        std::thread::current().id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid time")
            .as_nanos()
    )
    .replace(['(', ')', ' '], "_");
    let test_dir = std::env::temp_dir().join(format!("numlang_phase35_{}", id));
    fs::create_dir_all(&test_dir).expect("create test dir");
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).expect("write src file");

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

fn get_mir(source: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(source).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    lower_program(&typed)
}

#[test]
fn test_dead_node_elimination() {
    let src = r#"
fn dead_branch(x: i64) -> i64 {
    if 1 > 2 { return 999; }
    return x * 2;
}
fn main() -> i64 { return dead_branch(21); }
"#;
    let program = get_mir(src);
    let dead_fn = program
        .functions
        .iter()
        .find(|f| f.name == "dead_branch")
        .unwrap();

    let mut tree = SupercompilerDriver::new(dead_fn)
        .with_param_refinement("x", Interval::exact(21))
        .run();

    let (dead_eliminated, _) = compact_process_tree(&mut tree);
    assert!(
        dead_eliminated >= 1 || tree.stats.branches_pruned >= 1,
        "Expected dead_eliminated >= 1 or branches_pruned >= 1, got dead={}, pruned={}",
        dead_eliminated,
        tree.stats.branches_pruned
    );

    // Verify idempotence: calling a second time eliminates 0 additional nodes
    let (dead_second, _) = compact_process_tree(&mut tree);
    assert_eq!(
        dead_second, 0,
        "compact_process_tree must be idempotent on second invocation"
    );

    let (code, _, stderr) = run_numlang_code(src, true);
    assert_eq!(code, Some(42), "stderr: {}", stderr);
}

#[test]
fn test_noop_assignment_removal() {
    let src = r#"
fn main() -> i64 {
    let mut s: i64 = 0;
    let mut i: i64 = 0;
    while i < 10 { s = s + i; i = i + 1; }
    return s;
}
"#;
    let program = get_mir(src);
    let main_fn = program.functions.iter().find(|f| f.name == "main").unwrap();
    let tree = SupercompilerDriver::new(main_fn).run();
    let mut residual = residualize_process_tree(&tree, main_fn);

    compact_mir_function(&mut residual);

    // Assert no Statement::Assign(p, Rvalue::Use(q)) with p == q remains
    for block in &residual.blocks {
        for stmt in &block.statements {
            if let numlang::mir::lower::Statement::Assign(p, numlang::mir::lower::Rvalue::Use(q)) =
                stmt
            {
                assert_ne!(p, q, "Found identity assignment {:?} = Use({:?})", p, q);
            }
        }
    }

    let (code, _, stderr) = run_numlang_code(src, true);
    assert_eq!(code, Some(45), "stderr: {}", stderr);
}

#[test]
fn test_eta_reduction_copy_binding() {
    use numlang::mir::lower::{MirBasicBlock, MirFunction, MirLocalDecl, Rvalue, Statement};
    use numlang::typecheck::types::Type;

    let mut func = MirFunction {
        name: "test_eta".to_string(),
        params: vec![("x".to_string(), Type::I64)],
        return_ty: Type::I64,
        locals: vec![
            MirLocalDecl {
                name: "x".to_string(),
                ty: Type::I64,
                mutable: false,
            },
            MirLocalDecl {
                name: "_sc_0".to_string(),
                ty: Type::I64,
                mutable: false,
            },
        ],
        blocks: vec![MirBasicBlock {
            id: BasicBlockId(0),
            arguments: vec![],
            statements: vec![Statement::Assign(
                Place {
                    local: "_sc_0".to_string(),
                    projections: vec![],
                },
                Rvalue::Use(Place {
                    local: "x".to_string(),
                    projections: vec![],
                }),
            )],
            terminator: Terminator::Return {
                value: Some(Place {
                    local: "_sc_0".to_string(),
                    projections: vec![],
                }),
            },
        }],
        is_distilled: false,
    };

    let stmts_removed = compact_mir_function(&mut func);
    assert!(
        stmts_removed >= 1,
        "Expected at least 1 statement removed, got {}",
        stmts_removed
    );
    assert_eq!(
        func.blocks[0].terminator,
        Terminator::Return {
            value: Some(Place {
                local: "x".to_string(),
                projections: vec![]
            })
        },
        "Expected return directly uses x"
    );
    assert!(
        func.blocks[0].statements.is_empty(),
        "Expected assignment statement was deleted"
    );

    // Verify idempotence
    let second_removed = compact_mir_function(&mut func);
    assert_eq!(
        second_removed, 0,
        "compact_mir_function must be idempotent on second invocation"
    );
}

#[test]
fn test_code_size_metric_populated() {
    let src = r#"
fn double(x: i64) -> i64 { return x * 2; }
fn main() -> i64 { return double(21); }
"#;
    let mut program = get_mir(src);
    let total_stats = supercompile_mir_program(&mut program);
    assert!(
        total_stats.residual_block_count > 0,
        "residual_block_count should be > 0, got {}",
        total_stats.residual_block_count
    );
    assert!(
        total_stats.residual_stmt_count > 0,
        "residual_stmt_count should be > 0, got {}",
        total_stats.residual_stmt_count
    );
}

#[test]
fn test_compaction_preserves_correctness() {
    let prog1 = "fn main() -> i64 { return 42; }";
    let prog2 = r#"
fn compute_sum(n: i64) -> i64 {
    let mut s: i64 = 0;
    let mut i: i64 = 1;
    while i <= n { s = s + i; i = i + 1; }
    return s;
}
fn main() -> i64 { return compute_sum(10); }
"#;
    let prog3 = r#"
fn max(a: i64, b: i64) -> i64 {
    if a > b { return a; } else { return b; }
}
fn main() -> i64 { return max(10, 5); }
"#;
    let prog4 = r#"
fn main() -> i64 {
    let mut s: i64 = 0;
    let mut i: i64 = 0;
    while i < 10 { s = s + i; i = i + 1; }
    return s;
}
"#;
    let prog5 = r#"
fn id(x: i64) -> i64 { return x; }
fn main() -> i64 { return id(42); }
"#;

    let cases = [
        (prog1, 42),
        (prog2, 55),
        (prog3, 10),
        (prog4, 45),
        (prog5, 42),
    ];

    for (src, expected) in cases {
        let (code, _, stderr) = run_numlang_code(src, true);
        assert_eq!(
            code,
            Some(expected),
            "Failed for program with expected exit code {}: stderr: {}",
            expected,
            stderr
        );
    }
}
