//! Phase 48: Algorithmic Generality & Structural Name Decoupling Tests
//!
//! Validates that optimization passes, supercompiler pipelines, and codegen backends
//! operate purely on mathematical structure and type-directed properties rather than
//! benchmark function names (`ack`, `tak`, `append3`) or hardcoded field names (`"x"`, `"y"`).

use std::fs;
use std::process::Command;

use numlang::codegen::cranelift_backend::compile_to_obj;
use numlang::codegen::linker::link_executable;
use numlang::opt::recursion::{
    detect_nested_hyper_recurrence, detect_symmetric_permutation_recurrence,
    NestedHyperRecurrencePattern, PermutationRecurrencePattern,
};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn compile_and_run(src: &str, test_name: &str) -> (i32, String) {
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");

    let obj_bytes = compile_to_obj(&typed).expect("Codegen failed");

    let test_dir = std::env::temp_dir().join(format!("numlang_generality_test_{}", test_name));
    let _ = fs::create_dir_all(&test_dir);
    let obj_path = test_dir.join(format!("{}.obj", test_name));
    let exe_path = test_dir.join(format!("{}.exe", test_name));
    fs::write(&obj_path, obj_bytes).expect("Write obj failed");

    let link_res = link_executable(&obj_path, &exe_path);
    assert!(link_res.is_ok(), "Linking failed: {:?}", link_res.err());

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to execute binary");

    let _ = fs::remove_file(&obj_path);
    let _ = fs::remove_file(&exe_path);
    let _ = fs::remove_dir(&test_dir);

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let code = output.status.code().unwrap_or(-1);
    (code, stdout)
}

#[test]
fn test_structural_nested_hyper_recurrence_detection() {
    let src = r#"
fn custom_hyper_op_42(alpha: i64, beta: i64) -> i64 {
    if alpha == 0 {
        return beta + 1;
    } else {
        if beta == 0 {
            return custom_hyper_op_42(alpha - 1, 1);
        } else {
            return custom_hyper_op_42(alpha - 1, custom_hyper_op_42(alpha, beta - 1));
        }
    }
}
fn main() -> i64 {
    return custom_hyper_op_42(1, 2);
}
"#;
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");

    let func = typed.functions.iter().find(|f| f.name == "custom_hyper_op_42").unwrap();
    let pattern = detect_nested_hyper_recurrence(func);
    assert_eq!(
        pattern,
        Some(NestedHyperRecurrencePattern {
            param_m: "alpha".to_string(),
            param_n: "beta".to_string(),
        }),
        "Structural recurrence detector must match arbitrary function and variable names"
    );
}

#[test]
fn test_structural_nested_hyper_recurrence_execution() {
    let src = r#"
fn custom_hyper_op_42(alpha: i64, beta: i64) -> i64 {
    if alpha == 0 {
        return beta + 1;
    } else {
        if beta == 0 {
            return custom_hyper_op_42(alpha - 1, 1);
        } else {
            return custom_hyper_op_42(alpha - 1, custom_hyper_op_42(alpha, beta - 1));
        }
    }
}
fn main() -> i64 {
    // alpha=2, beta=3 => 2 * 3 + 3 = 9
    return custom_hyper_op_42(2, 3);
}
"#;
    let (code, _) = compile_and_run(src, "obfuscated_ackermann");
    assert_eq!(code, 9, "Obfuscated Ackermann recurrence must execute to 9");
}

#[test]
fn test_structural_permutation_recurrence_detection() {
    let src = r#"
fn cyclic_takeuchi_kernel(dim_u: i64, dim_v: i64, dim_w: i64) -> i64 {
    if dim_v < dim_u {
        return cyclic_takeuchi_kernel(
            cyclic_takeuchi_kernel(dim_u - 1, dim_v, dim_w),
            cyclic_takeuchi_kernel(dim_v - 1, dim_w, dim_u),
            cyclic_takeuchi_kernel(dim_w - 1, dim_u, dim_v)
        );
    } else {
        return dim_w;
    }
}
fn main() -> i64 {
    return cyclic_takeuchi_kernel(12, 8, 4);
}
"#;
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");

    let func = typed.functions.iter().find(|f| f.name == "cyclic_takeuchi_kernel").unwrap();
    let pattern = detect_symmetric_permutation_recurrence(func);
    assert_eq!(
        pattern,
        Some(PermutationRecurrencePattern {
            param_x: "dim_u".to_string(),
            param_y: "dim_v".to_string(),
            param_z: "dim_w".to_string(),
        }),
        "Symmetric permutation recurrence detector must match arbitrary names"
    );
}

#[test]
fn test_structural_permutation_recurrence_execution() {
    let src = r#"
fn cyclic_takeuchi_kernel(dim_u: i64, dim_v: i64, dim_w: i64) -> i64 {
    if dim_v < dim_u {
        return cyclic_takeuchi_kernel(
            cyclic_takeuchi_kernel(dim_u - 1, dim_v, dim_w),
            cyclic_takeuchi_kernel(dim_v - 1, dim_w, dim_u),
            cyclic_takeuchi_kernel(dim_w - 1, dim_u, dim_v)
        );
    } else {
        return dim_w;
    }
}
fn main() -> i64 {
    // tak(12, 8, 4) = 5
    return cyclic_takeuchi_kernel(12, 8, 4);
}
"#;
    let (code, _) = compile_and_run(src, "obfuscated_takeuchi");
    assert_eq!(code, 5, "Obfuscated Takeuchi recurrence must execute to 5");
}

#[test]
fn test_arbitrary_struct_field_layout_execution() {
    let src = r#"
struct TensorMetric {
    g00: i64,
    g01: i64,
    g10: i64,
    g11: i64,
}

fn main() -> i64 {
    let t: TensorMetric = TensorMetric {
        g00: 15,
        g01: 30,
        g10: 45,
        g11: 77,
    };
    return t.g11 - t.g00; // 77 - 15 = 62
}
"#;
    let (code, _) = compile_and_run(src, "arbitrary_struct_fields");
    assert_eq!(code, 62, "Arbitrary struct layout must compute exact offset (77 - 15 = 62)");
}

#[test]
fn test_distilled_flag_metadata_propagation() {
    use numlang::mir::lower::lower_program;

    let src = r#"
fn compute_sum(a: i64, b: i64) -> i64 {
    return a + b;
}
fn main() -> i64 {
    return compute_sum(10, 20);
}
"#;
    let tokens = tokenize(src).expect("Tokenize failed");
    let ast = parse(&tokens).expect("Parse failed");
    let typed = typecheck(&ast).expect("Typecheck failed");
    let mut mir = lower_program(&typed);

    assert_eq!(mir.functions.len(), 2);
    for func in &mir.functions {
        assert!(!func.is_distilled, "User-defined functions default to is_distilled=false");
    }

    // Setting is_distilled explicitly
    mir.functions[0].is_distilled = true;
    assert!(mir.functions[0].is_distilled);
}
