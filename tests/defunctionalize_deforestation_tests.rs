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
            // Either the captured environment is unpacked into a temp
            // (`Rvalue::Use` with `Projection::Payload(0)`), or the scalar field
            // is unboxed: the direct call passes a place with `Projection::Payload(0)`.
            let found = match rval {
                Rvalue::Use(place) => place.projections.contains(&Projection::Payload(0)),
                Rvalue::Call(_, args) => args
                    .iter()
                    .any(|a| a.projections.contains(&Projection::Payload(0))),
                _ => false,
            };
            if found {
                payload_projection_found = true;
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

#[test]
fn test_non_escaping_closure_direct_call() {
    // A closure that is only ever directly dispatched (never stored, passed, or
    // returned) must be inlined/direct-called rather than tagged by defunctionalization.
    let src = r#"
fn main() -> i64 {
    let f: fn(i64) -> i64 = |x: i64| x * 3;
    let a: i64 = f(7);
    return a;
}
"#;
    let mir = get_mir(src);

    // No synthetic ClosureTag enum should be created.
    assert!(
        !mir.enums.iter().any(|e| e.name.starts_with("ClosureTag_")),
        "Non-escaping closures must not produce tagged variants"
    );
    // No ClosureAlloc or IndirectCall should remain.
    for func in &mir.functions {
        for block in &func.blocks {
            for stmt in &block.statements {
                let Statement::Assign(_, rval) = stmt;
                assert!(
                    !matches!(rval, Rvalue::ClosureAlloc { .. }),
                    "ClosureAlloc of a non-escaping closure must be rewritten"
                );
            }
            assert!(
                !matches!(block.terminator, Terminator::IndirectCall { .. }),
                "IndirectCall of a non-escaping closure must be rewritten to a direct Call"
            );
        }
    }
    // A direct Call to the closure function must exist.
    let direct_calls: Vec<String> = mir
        .functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .flat_map(|b| b.statements.iter())
        .filter_map(|s| match s {
            Statement::Assign(_, Rvalue::Call(c, _)) => Some(c.clone()),
            _ => None,
        })
        .collect();
    assert!(
        direct_calls.iter().any(|c| c.contains("closure")),
        "Must contain a direct call to the closure function: {:?}",
        direct_calls
    );

    // Execution still returns the right value: 7 * 3 = 21.
    let (code, _, stderr) = run_numlang_code(src, true);
    assert_eq!(code, Some(21), "stderr: {}", stderr);
}

#[test]
fn test_escaping_closure_captured_scalar_unboxed() {
    // An escaping closure with a scalar captured field stays tagged, but the
    // scalar payload is unboxed at the dispatch site (read directly from the
    // enum payload instead of being copied into a temporary).
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

    // The closure is escaping: a synthetic enum must exist and be dispatched via Switch.
    let closure_enum = mir
        .enums
        .iter()
        .find(|e| e.name.starts_with("ClosureTag_"))
        .expect("escaping closure must produce a ClosureTag enum");
    assert_eq!(closure_enum.variants.len(), 1, "one variant for one closure");

    // The dispatch of the escaping closure must go through a Switch.
    let mut switch_found = false;
    let mut unboxed_found = false;
    for func in &mir.functions {
        for block in &func.blocks {
            if let Terminator::Switch { targets, .. } = &block.terminator {
                assert!(
                    !targets.is_empty(),
                    "escaping closure must dispatch via Switch"
                );
                switch_found = true;
            }
            for stmt in &block.statements {
                let Statement::Assign(_, rval) = stmt;
                if let Rvalue::Call(_, args) = rval {
                    if args
                        .iter()
                        .any(|a| a.projections.contains(&Projection::Payload(0)))
                    {
                        unboxed_found = true;
                    }
                }
            }
        }
    }
    assert!(switch_found, "escaping closure dispatch must be a Switch");
    assert!(
        unboxed_found,
        "escaping closure with scalar captured field must be unboxed (Payload passed as call arg)"
    );

    // Exec: apply(f, 6) = 6 * 7 = 42
    let (code, _, stderr) = run_numlang_code(src, false);
    assert_eq!(code, Some(42), "stderr: {}", stderr);
}
