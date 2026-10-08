use std::fs;
use std::process::Command;

use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::{Projection, Terminator};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn get_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).expect("tokenization failed");
    let program = parse(&tokens).expect("parsing failed");
    let typed = typecheck(&program).expect("typecheck failed");
    lower_program(&typed)
}

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
    let test_dir = std::env::temp_dir().join(format!("numlang_defun_{}", id));
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

#[test]
fn test_defun_01_synthetic_enums_and_closure_rewriting() {
    let src = r#"
fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return f(x);
}
fn main() -> i64 {
    let f1: fn(i64) -> i64 = |x: i64| x + 1;
    let f2: fn(i64) -> i64 = |x: i64| x * 2;
    return apply(f1, 10) + apply(f2, 10);
}
"#;
    let mir = get_mir(src);

    // DEFUN-01: Synthetic closure enum must be created in program.enums
    let closure_enum = mir
        .enums
        .iter()
        .find(|e| e.name.starts_with("ClosureTag_"))
        .expect("Must have synthesized ClosureTag enum");
    assert_eq!(
        closure_enum.variants.len(),
        2,
        "Must contain 2 closure variants"
    );

    // DEFUN-02: ClosureAlloc must be eliminated and replaced by EnumVariant
    let mut enum_variant_count = 0;
    let mut closure_alloc_count = 0;
    for func in &mir.functions {
        for block in &func.blocks {
            for stmt in &block.statements {
                let Statement::Assign(_, rval) = stmt;
                match rval {
                    Rvalue::EnumVariant { enum_name, .. }
                        if enum_name.starts_with("ClosureTag_") =>
                    {
                        enum_variant_count += 1;
                    }
                    Rvalue::ClosureAlloc { .. } => {
                        closure_alloc_count += 1;
                    }
                    _ => {}
                }
            }
        }
    }
    assert_eq!(
        closure_alloc_count, 0,
        "All ClosureAlloc must be eliminated"
    );
    assert_eq!(
        enum_variant_count, 2,
        "Must have lowered to 2 EnumVariant instantiations"
    );

    // DEFUN-03: IndirectCall in apply must be lowered into Terminator::Switch
    let apply_fn = mir
        .functions
        .iter()
        .find(|f| f.name == "apply")
        .expect("apply fn found");
    let mut switch_found = false;
    let mut indirect_found = false;
    for block in &apply_fn.blocks {
        match &block.terminator {
            Terminator::Switch { targets, .. } => {
                assert_eq!(
                    targets.len(),
                    2,
                    "Switch must dispatch to 2 closure variants"
                );
                switch_found = true;
            }
            Terminator::IndirectCall { .. } => {
                indirect_found = true;
            }
            _ => {}
        }
    }
    assert!(
        switch_found,
        "Terminator::Switch must replace indirect call"
    );
    assert!(
        !indirect_found,
        "Terminator::IndirectCall must be eliminated"
    );
}

#[test]
fn test_defun_02_captured_environment_payload_unpacking() {
    let src = r#"
fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return f(x);
}
fn main() -> i64 {
    let factor: i64 = 7;
    let f: fn(i64) -> i64 = |x: i64| x * factor;
    return apply(f, 6);
}
"#;
    let mir = get_mir(src);

    let apply_fn = mir
        .functions
        .iter()
        .find(|f| f.name == "apply")
        .expect("apply fn found");
    let mut payload_projection_found = false;
    for block in &apply_fn.blocks {
        for stmt in &block.statements {
            let Statement::Assign(_, rval) = stmt;
            if let Rvalue::Use(place) = rval {
                if place.projections.contains(&Projection::Payload(0)) {
                    payload_projection_found = true;
                }
            }
        }
    }
    assert!(
        payload_projection_found,
        "Dispatch block must unpack captured environment via Projection::Payload(0)"
    );
}

#[test]
fn test_defun_03_execution_direct_multiple_closures() {
    // Both f1 and f2 dispatched through the same higher-order function without supercompilation
    let src = r#"
fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return f(x);
}
fn main() -> i64 {
    let f1: fn(i64) -> i64 = |x: i64| x + 5;
    let f2: fn(i64) -> i64 = |x: i64| x * 3;
    return apply(f1, 10) + apply(f2, 10);
}
"#;
    // apply(f1, 10) = 15; apply(f2, 10) = 30; total = 45
    let (code, _, stderr) = run_numlang_code(src, false);
    assert_eq!(code, Some(45), "stderr: {}", stderr);
}

#[test]
fn test_defun_04_execution_captured_environment() {
    let src = r#"
fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return f(x);
}
fn main() -> i64 {
    let base: i64 = 100;
    let f: fn(i64) -> i64 = |x: i64| x + base;
    return apply(f, 42);
}
"#;
    // apply(f, 42) = 142
    let (code, _, stderr) = run_numlang_code(src, false);
    assert_eq!(code, Some(142), "stderr: {}", stderr);
}

#[test]
fn test_defun_05_higher_order_deforestation_supercompilation() {
    // Pipeline with multiple applications and supercompiler deforestation
    let src = r#"
fn pipe(f: fn(i64) -> i64, g: fn(i64) -> i64, x: i64) -> i64 {
    return g(f(x));
}
fn main() -> i64 {
    let add3: fn(i64) -> i64 = |x: i64| x + 3;
    let mul4: fn(i64) -> i64 = |x: i64| x * 4;
    return pipe(add3, mul4, 7);
}
"#;
    // pipe(add3, mul4, 7) = (7 + 3) * 4 = 40
    let (code, _, stderr) = run_numlang_code(src, true);
    assert_eq!(code, Some(40), "stderr: {}", stderr);
}
