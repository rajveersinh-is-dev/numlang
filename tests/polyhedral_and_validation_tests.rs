use std::fs;
use std::process::Command;

use numlang::codegen::compile_supercompiled_to_obj_with_mode;
use numlang::codegen::linker::link_executable;
use numlang::mir::lower::{lower_program, MirProgram, Rvalue, Statement};
use numlang::mir::supercompiler::parallel::supercompile_mir_program_parallel;
use numlang::mir::supercompiler::polyhedral::fuse_polyhedral_stencils;
use numlang::mir::supercompiler::validate::{
    verify_program_equivalence, TranslationValidator, ValidationError,
};
use numlang::mir::supercompiler::{
    supercompile_mir_program_with_mode, SupercompileMode,
};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::typed_ast::TypedLiteral;

fn get_mir(source: &str) -> MirProgram {
    let tokens = tokenize(source).expect("Tokenize failed");
    let program = parse(&tokens).expect("Parse failed");
    let mut typed = typecheck(&program).expect("Typecheck failed");
    typed.desugar_for_loops();
    lower_program(&typed)
}

fn compile_and_run(src: &str, test_name: &str) -> i32 {
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");

    let obj_bytes = compile_supercompiled_to_obj_with_mode(
        &typed,
        SupercompileMode::Classic,
        "size",
    )
    .expect("Codegen failed");

    let test_dir = std::env::temp_dir().join(format!("numlang_phase19_{}", test_name));
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
fn test_polyhedral_stencil_fusion() {
    let code = r#"
    fn stencil_pipeline(xs: [i64; 4]) -> i64 {
        let mut temp: [i64; 4] = [0, 0, 0, 0];
        for i in 0..4 {
            temp[i] = xs[i] * 2 + 1;
        }
        let mut sum: i64 = 0;
        for j in 0..4 {
            sum = sum + temp[j] * 3;
        }
        return sum;
    }

    fn main() -> i64 {
        let xs: [i64; 4] = [10, 20, 30, 40];
        return stencil_pipeline(xs);
    }
    "#;

    // Direct execution test
    let res = compile_and_run(code, "test_polyhedral_stencil_fusion");
    // (10*2+1)*3 + (20*2+1)*3 + (30*2+1)*3 + (40*2+1)*3
    // = 63 + 123 + 183 + 243 = 612
    assert_eq!(res, 612, "Pipeline result must match 612");

    // Polyhedral analysis test: verify `temp` array allocation is eliminated
    let mut mir = get_mir(code);
    let func = mir.functions.iter_mut().find(|f| f.name == "stencil_pipeline").unwrap();
    let fusions = fuse_polyhedral_stencils(func);
    assert!(fusions > 0, "Polyhedral stencil fusion must fuse intermediate pipeline");

    let mut has_temp_alloc = false;
    for b in &func.blocks {
        for stmt in &b.statements {
            let Statement::Assign(dest, rval) = stmt;
            if dest.local == "temp" && matches!(rval, Rvalue::Array(_)) {
                has_temp_alloc = true;
            }
        }
    }
    assert!(!has_temp_alloc, "Intermediate array `temp` must be eliminated from MIR");
}

#[test]
fn test_translation_validation_equivalence() {
    let code = r#"
    fn add_scaled(x: i64, y: i64) -> i64 {
        let a: i64 = x * 2;
        let b: i64 = y * 2;
        return a + b;
    }

    fn main() -> i64 {
        return add_scaled(10, 20);
    }
    "#;

    let orig_mir = get_mir(code);
    let mut sc_mir = orig_mir.clone();
    supercompile_mir_program_with_mode(&mut sc_mir, SupercompileMode::Classic, "size");

    let orig_func = orig_mir.functions.iter().find(|f| f.name == "add_scaled").unwrap();
    let res_func = sc_mir.functions.iter().find(|f| f.name == "add_scaled").unwrap();

    let mut validator = TranslationValidator::new(orig_func, res_func);
    let cert = validator.verify().expect("Translation validation must succeed");

    assert!(cert.is_certified, "Validation certificate must be certified");
    assert_eq!(cert.function_name, "add_scaled");
    assert!(cert.paths_verified >= 1);

    // Verify program-wide equivalence
    let program_certs = verify_program_equivalence(&orig_mir, &sc_mir)
        .expect("Program-wide translation validation must succeed");
    assert!(!program_certs.is_empty(), "Must produce certificates for all functions");
}

#[test]
fn test_translation_validation_detects_unsoundness() {
    let code = r#"
    fn identity_plus_one(x: i64) -> i64 {
        return x + 1;
    }

    fn main() -> i64 {
        return identity_plus_one(5);
    }
    "#;

    let orig_mir = get_mir(code);
    let mut tampered_mir = orig_mir.clone();

    // Intentionally tamper with the return value in tampered_mir
    let func = tampered_mir.functions.iter_mut().find(|f| f.name == "identity_plus_one").unwrap();
    for b in &mut func.blocks {
        for stmt in &mut b.statements {
            let Statement::Assign(_, rval) = stmt;
            *rval = Rvalue::Constant(TypedLiteral::Int(999, numlang::typecheck::types::Type::I64));
        }
    }

    let orig_func = orig_mir.functions.iter().find(|f| f.name == "identity_plus_one").unwrap();
    let tampered_func = tampered_mir.functions.iter().find(|f| f.name == "identity_plus_one").unwrap();

    let mut validator = TranslationValidator::new(orig_func, tampered_func);
    let err = validator.verify().expect_err("Validation must fail on tampered residual");

    assert!(matches!(err, ValidationError::OutputMismatch { .. }), "Expected OutputMismatch error");
}

#[test]
fn test_parallel_supercompilation() {
    let code = r#"
    fn compute_a(x: i64) -> i64 {
        let a: i64 = x * 10;
        let b: i64 = 10;
        return a + b;
    }

    fn compute_b(y: i64) -> i64 {
        return y * 10 + 5;
    }

    fn main() -> i64 {
        return compute_a(2) + compute_b(3);
    }
    "#;

    let mut mir_parallel = get_mir(code);
    let stats = supercompile_mir_program_parallel(
        &mut mir_parallel,
        SupercompileMode::Classic,
        "size",
        4,
    );

    assert!(stats.nodes_explored > 0, "Parallel driving must explore nodes");
    assert_eq!(mir_parallel.functions.len(), 3, "All functions must be preserved");

    // Validate equivalence of parallel supercompiled program
    let orig_mir = get_mir(code);
    let certs = verify_program_equivalence(&orig_mir, &mir_parallel)
        .expect("Parallel supercompiled program must pass translation validation");
    assert_eq!(certs.len(), 3, "All 3 functions must be certified");

    let res = compile_and_run(code, "test_parallel_supercompilation");
    assert_eq!(res, 65, "Parallel supercompiled execution must match 65");
}

#[test]
fn test_cli_verify_equivalence() {
    let code = r#"
    fn square(x: i64) -> i64 {
        return x * x;
    }

    fn main() -> i64 {
        return square(8);
    }
    "#;

    let test_dir = std::env::temp_dir().join("numlang_test_cli_verify");
    let _ = fs::create_dir_all(&test_dir);
    let src_file = test_dir.join("verify_test.nl");
    fs::write(&src_file, code).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .args(["--verify-equivalence", src_file.to_str().unwrap()])
        .output()
        .expect("CLI execution failed");

    assert!(output.status.success(), "CLI run with --verify-equivalence must succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("translation validation certified"), "Stdout must confirm certification: {}", stdout);
    assert!(stdout.contains("square"), "Stdout must mention verified function `square`");

    let _ = fs::remove_dir_all(&test_dir);
}
