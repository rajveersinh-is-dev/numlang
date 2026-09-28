use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::drive::SupercompilerDriver;
use numlang::mir::supercompiler::term::SymTerm;
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
    let test_dir = std::env::temp_dir().join(format!("numlang_phase34_{}", id));
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
fn test_closure_driving_constant_fold() {
    let code = r#"
fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return f(x);
}
fn main() -> i64 {
    let f: fn(i64) -> i64 = |x: i64| x * 2;
    return apply(f, 21);
}
"#;
    let (sc, _, stderr) = run_numlang_code(code, true);
    assert_eq!(sc, Some(42), "stderr: {}", stderr);
}

#[test]
fn test_closure_captures_variable_fold() {
    let code = r#"
fn main() -> i64 {
    let factor: i64 = 6;
    let f: fn(i64) -> i64 = |x: i64| x * factor;
    return f(7);
}
"#;
    let (sc, _, stderr) = run_numlang_code(code, true);
    assert_eq!(sc, Some(42), "stderr: {}", stderr);
}

#[test]
fn test_closure_inline_in_loop() {
    let code = r#"
fn main() -> i64 {
    let f: fn(i64) -> i64 = |x: i64| x + 1;
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < 10 {
        sum = sum + f(i);
        i = i + 1;
    }
    return sum;
}
"#;
    let (sc, _, stderr) = run_numlang_code(code, true);
    assert_eq!(sc, Some(55), "stderr: {}", stderr);
}

#[test]
fn test_fn_ptr_driving() {
    let code = r#"
fn double(x: i64) -> i64 {
    return x * 2;
}
fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return f(x);
}
fn main() -> i64 {
    let f: fn(i64) -> i64 = double;
    return apply(f, 21);
}
"#;
    let (sc, _, stderr) = run_numlang_code(code, true);
    assert_eq!(sc, Some(42), "stderr: {}", stderr);
}

#[test]
fn test_two_closure_composition() {
    let code = r#"
fn main() -> i64 {
    let f: fn(i64) -> i64 = |x: i64| x * 2;
    let g: fn(i64) -> i64 = |x: i64| x + 10;
    let a: i64 = f(5);
    let b: i64 = g(a);
    return b;
}
"#;
    let (sc, _, stderr) = run_numlang_code(code, true);
    assert_eq!(sc, Some(20), "stderr: {}", stderr);
}

#[test]
fn test_closure_with_refinement_interaction() {
    let code = r#"
fn check(x: i64) -> i64 {
    if x > 100 {
        return 0 - 1;
    }
    return x * 2;
}

fn main() -> i64 {
    let f: fn(i64) -> i64 = |x: i64| check(x);
    let result: i64 = f(21);
    return result;
}
"#;

    let program = get_mir(code);
    let main_func = program.functions.iter().find(|f| f.name == "main").unwrap();

    let tree = SupercompilerDriver::new(main_func)
        .with_program_functions(&program.functions)
        .run();

    assert!(
        tree.stats.branches_pruned >= 1,
        "Expected at least 1 branch pruned, got {}",
        tree.stats.branches_pruned
    );

    let ret_node = tree
        .nodes
        .iter()
        .find(|n| n.return_term.is_some())
        .expect("Expected return node");
    let ret_term = ret_node.return_term.unwrap();
    if let SymTerm::ConstInt(val, _) = tree.interner.get(ret_term) {
        assert_eq!(*val, 42, "Expected returned constant 42, got {}", val);
    }

    let (sc, _, stderr) = run_numlang_code(code, true);
    assert_eq!(sc, Some(42), "stderr: {}", stderr);
}
