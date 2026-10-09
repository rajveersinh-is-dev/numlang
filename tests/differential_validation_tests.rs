//! Differential Compiler Validation Test Suite (Phase 57 Deliverable 3 & 4).
//!
//! Comprehensive cross-validation across 6 execution paths:
//! - Path A: `src/testing/oracle.rs` AST tree-walking interpreter (ground truth oracle)
//! - Path B: Cranelift AOT compilation (no supercompilation)
//! - Path C: LLVM AOT compilation (no supercompilation, if enabled)
//! - Path D: Tier 0 direct MIR lowering JIT (< 2ms cold start, zero SC)
//! - Path E: Tier 1 Supercompiled pipeline (distillation + classic/MRSC supercompilation)
//! - Path F: Lean 4 mechanized operational semantics model (`lake exe lean_eval`)
//!
//! Validates:
//! - DIFF-01: 10,000 random programs across paths A–E with zero divergences.
//! - DIFF-02: 1,000 programs across Path A (Oracle) and Path F (Lean model) with zero divergences.

use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use proptest::prelude::*;

use numlang::codegen::{
    compile_mir_to_obj, compile_to_obj_with_opt, link_executable, LlvmCompiler, OptLevel,
};
use numlang::compiler::{compile_tier0, compile_tier1};
use numlang::mir::lower::lower_program;
use numlang::parser::parse;
use numlang::testing::gen::{generate_well_typed_program, GenConfig};
use numlang::testing::lean_bridge::eval_with_lean_model;
use numlang::testing::oracle::{evaluate_program, OracleResult};
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn link_and_run(obj_bytes: &[u8], test_id: &str) -> Result<(Option<i32>, String), String> {
    let test_dir = std::env::temp_dir().join(format!(
        "nl_run_{}_{}_{}",
        std::process::id(),
        test_id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&test_dir).map_err(|e| e.to_string())?;
    let obj_path = test_dir.join("test.obj");
    let exe_path = test_dir.join("test.exe");
    fs::write(&obj_path, obj_bytes).map_err(|e| e.to_string())?;

    link_executable(&obj_path, &exe_path).map_err(|e| format!("Linker error: {:?}", e))?;

    let mut child = Command::new(&exe_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(3);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = fs::remove_dir_all(&test_dir);
                    return Err(format!("Execution timed out after {:?}", timeout));
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = fs::remove_dir_all(&test_dir);
                return Err(e.to_string());
            }
        }
    };

    let mut stdout = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        use std::io::Read;
        let _ = out.read_to_end(&mut stdout);
    }
    let _ = fs::remove_dir_all(&test_dir);
    let stdout_str = String::from_utf8_lossy(&stdout).trim().to_string();
    Ok((status.code(), stdout_str))
}

fn check_llvm_available() -> bool {
    let mut compiler = LlvmCompiler::new(OptLevel::O0);
    // Check if compiler initialized without BackendDisabled
    let test_src = "fn main() -> i64 { return 0; }\n";
    if let Ok(tokens) = tokenize(test_src) {
        if let Ok(ast) = parse(&tokens) {
            if let Ok(typed) = typecheck(&ast) {
                let mir = lower_program(&typed);
                return compiler.compile_mir_to_obj_bytes(&mir).is_ok();
            }
        }
    }
    false
}

/// Evaluates a single program across Paths A, B, D, E (and C if LLVM is supported).
fn validate_program_paths_a_to_e(
    src: &str,
    seed: u64,
    thread_id: usize,
    llvm_supported: bool,
) -> Result<(), String> {
    // 1. Path A: Oracle reference interpreter
    let tokens = tokenize(src).map_err(|e| format!("Tokenize error on seed {}: {:?}", seed, e))?;
    let ast = parse(&tokens).map_err(|e| format!("Parse error on seed {}: {:?}", seed, e))?;
    let oracle_val = match evaluate_program(&ast) {
        OracleResult::Value(v) => v,
        OracleResult::Diverged => return Err(format!("Oracle diverged on seed {}", seed)),
        OracleResult::Error(e) => return Err(format!("Oracle error on seed {}: {}", seed, e)),
    };

    let expected_exit = oracle_val as i32;

    let mut typed =
        typecheck(&ast).map_err(|e| format!("Typecheck error on seed {}: {:?}", seed, e))?;
    typed.desugar_for_loops();

    let obj_b = compile_to_obj_with_opt(&typed, false)
        .map_err(|e| format!("Cranelift AOT codegen failed on seed {}: {:?}", seed, e))?;
    let (b_exit, b_out) = link_and_run(&obj_b, &format!("{}_b", thread_id))?;
    if b_exit != Some(expected_exit) {
        return Err(format!(
            "Path B (Cranelift AOT) mismatch on seed {}: expected {}, got exit code {:?}",
            seed, expected_exit, b_exit
        ));
    }

    let mir_d = compile_tier0(&mut typed.clone());
    let obj_d = compile_mir_to_obj(&mir_d)
        .map_err(|e| format!("Tier 0 MIR codegen failed on seed {}: {:?}", seed, e))?;
    let (d_exit, _d_out) = link_and_run(&obj_d, &format!("{}_d", thread_id))?;
    if d_exit != Some(expected_exit) {
        return Err(format!(
            "Path D (Tier 0 MIR) mismatch on seed {}: expected {}, got exit code {:?}",
            seed, expected_exit, d_exit
        ));
    }

    let mut typed_e = typed.clone();
    let mir_e = compile_tier1(&mut typed_e);
    let obj_e = compile_mir_to_obj(&mir_e).map_err(|e| {
        format!(
            "Tier 1 Supercompiler codegen failed on seed {}: {:?}",
            seed, e
        )
    })?;
    let (e_exit, e_out) = link_and_run(&obj_e, &format!("{}_e", thread_id))?;
    if e_exit != Some(expected_exit) {
        return Err(format!(
            "Path E (Tier 1 Supercompiled) mismatch on seed {}: expected {}, got exit code {:?}",
            seed, expected_exit, e_exit
        ));
    }

    // Equivalence assertion between baseline and supercompiled
    if b_out != e_out {
        return Err(format!(
            "Stdout divergence between Path B and Path E on seed {}: B='{}' vs E='{}'",
            seed, b_out, e_out
        ));
    }

    // 5. Path C: LLVM AOT (if enabled in build)
    if llvm_supported {
        let mut compiler = LlvmCompiler::new(OptLevel::O2);
        if let Ok(obj_c) = compiler.compile_mir_to_obj_bytes(&mir_d) {
            let (c_exit, _c_out) = link_and_run(&obj_c, &format!("{}_c", thread_id))?;
            if c_exit != Some(expected_exit) {
                return Err(format!(
                    "Path C (LLVM AOT) mismatch on seed {}: expected {}, got exit code {:?}",
                    seed, expected_exit, c_exit
                ));
            }
        }
    }

    Ok(())
}

/// Evaluates a program across Path A (Oracle) and Path F (Lean 4 operational semantics model).
fn validate_program_paths_a_and_f(src: &str, seed: u64) -> Result<(), String> {
    // Path A: Oracle interpreter
    let tokens = tokenize(src).map_err(|e| format!("Tokenize error on seed {}: {:?}", seed, e))?;
    let ast = parse(&tokens).map_err(|e| format!("Parse error on seed {}: {:?}", seed, e))?;
    let oracle_val = match evaluate_program(&ast) {
        OracleResult::Value(v) => v,
        OracleResult::Diverged => return Err(format!("Oracle diverged on seed {}", seed)),
        OracleResult::Error(e) => return Err(format!("Oracle error on seed {}: {}", seed, e)),
    };

    // Path F: Lean 4 model
    let mut typed =
        typecheck(&ast).map_err(|e| format!("Typecheck error on seed {}: {:?}", seed, e))?;
    typed.desugar_for_loops();
    let mir = lower_program(&typed);
    let lean_val = eval_with_lean_model(&mir)
        .map_err(|e| format!("Lean eval error on seed {}: {}", seed, e))?;

    if oracle_val != lean_val {
        return Err(format!(
            "Path A (Oracle={}) != Path F (Lean={}) divergence on seed {}:\n{}",
            oracle_val, lean_val, seed, src
        ));
    }

    Ok(())
}

#[test]
fn test_regression_modular_recurrence_paths_a_to_e() {
    let src = r#"
fn f_0(n: i64, a_0: i64, a_1: i64) -> i64 {
    if n <= 0 {
        return a_0;
    } else {
        return f_0(n - 1, (a_0 * 1 + 6) % 500, (a_1 * 1 + 4) % 500);
    }
}

fn main() -> i64 {
    let mut x: i64 = 4;
    let mut y: i64 = 21;
    let mut iter_0: i64 = 0;
    while iter_0 < 3 {
        y = (y + iter_0) % 400;
        iter_0 = iter_0 + 1;
    }
    let mut iter_1: i64 = 0;
    while iter_1 < 4 {
        x = (x + iter_1) % 400;
        iter_1 = iter_1 + 1;
    }
    y = f_0(3, y, x) % 500;
    let final_val: i64 = (x + y) % 256;
    if final_val < 0 {
        return final_val + 256;
    } else {
        return final_val;
    }
}
"#;
    let res = validate_program_paths_a_to_e(src, 2, 0, false);
    assert!(res.is_ok(), "Validation failed: {:?}", res.err());
}

// Proptest property-based differential validation with shrinking.
proptest! {
    #![proptest_config(ProptestConfig { cases: 10, failure_persistence: None, .. ProptestConfig::default() })]
    #[test]
    fn test_differential_proptest_shrinking(seed in any::<u64>()) {
        let config = GenConfig {
            max_depth: 3,
            max_functions: 2,
            max_args: 2,
        };
        let src = generate_well_typed_program(seed, &config);
        let res = validate_program_paths_a_to_e(&src, seed, 0, false);
        prop_assert!(res.is_ok(), "Validation failed: {}", res.err().unwrap());
    }
}

/// DIFF-02: 1,000 programs evaluated through Path F (Lean 4 model) vs Path A (Oracle).
#[test]
fn test_lean_model_cross_validation_1k() {
    if numlang::testing::lean_bridge::get_lean_eval_exe().is_none() {
        println!("[Phase 57 DIFF-02] Skipping Lean 4 model cross validation: lean_eval binary not available");
        return;
    }

    let count: u64 = std::env::var("LEAN_VALIDATION_COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if std::env::var("QUICK_BENCHMARKS").is_ok() {
            50
        } else if cfg!(debug_assertions) {
            100
        } else {
            1000
        });

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
        .min(16);

    let current_seed = AtomicU64::new(0);
    let divergences = Mutex::new(Vec::new());
    let start_time = Instant::now();
    let config = GenConfig {
        max_depth: 3,
        max_functions: 2,
        max_args: 2,
    };

    println!(
        "\n[Phase 57 DIFF-02] Starting Lean 4 model vs. Oracle differential validation on {} programs across {} threads...",
        count, num_threads
    );

    std::thread::scope(|s| {
        for _ in 0..num_threads {
            s.spawn(|| loop {
                let seed = current_seed.fetch_add(1, Ordering::Relaxed);
                if seed >= count {
                    break;
                }
                let src = generate_well_typed_program(seed, &config);
                if let Err(err) = validate_program_paths_a_and_f(&src, seed) {
                    let mut divs = divergences.lock().expect("mutex lock");
                    divs.push((seed, err));
                }
            });
        }
    });

    let elapsed = start_time.elapsed();
    let divs = divergences.into_inner().expect("mutex into_inner");

    println!(
        "[Phase 57 DIFF-02] Completed {} Lean model validations in {:.2?} ({:.1} programs/sec). Divergences: {}",
        count,
        elapsed,
        count as f64 / elapsed.as_secs_f64().max(0.001),
        divs.len()
    );

    assert!(
        divs.is_empty(),
        "{} divergences found between Lean 4 model and Oracle in {} programs:\n{}",
        divs.len(),
        count,
        divs.iter()
            .take(3)
            .map(|(s, e)| format!("seed {}: {}", s, e))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// DIFF-01: 10,000 programs evaluated through Paths A–E with zero divergences.
#[test]
fn test_differential_cross_validation_10k() {
    let count: u64 = std::env::var("DIFF_VALIDATION_COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if std::env::var("QUICK_BENCHMARKS").is_ok() {
            50
        } else if cfg!(debug_assertions) {
            200
        } else {
            10000
        });

    let llvm_supported = check_llvm_available();
    println!(
        "\n[Phase 57 DIFF-01] Starting differential validation on {} programs (LLVM Path C active: {})...",
        count, llvm_supported
    );

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
        .min(16);

    let current_seed = AtomicU64::new(0);
    let divergences = Mutex::new(Vec::new());
    let start_time = Instant::now();
    let config = GenConfig::default();
    let seed_ref = &current_seed;
    let divs_ref = &divergences;
    let config_ref = &config;

    std::thread::scope(|s| {
        for tid in 0..num_threads {
            s.spawn(move || loop {
                let seed = seed_ref.fetch_add(1, Ordering::Relaxed);
                if seed >= count {
                    break;
                }
                let src = generate_well_typed_program(seed, config_ref);
                if let Err(err) = validate_program_paths_a_to_e(&src, seed, tid, llvm_supported) {
                    let mut divs = divs_ref.lock().expect("mutex lock");
                    divs.push((seed, err));
                }
            });
        }
    });

    let elapsed = start_time.elapsed();
    let divs = divergences.into_inner().expect("mutex into_inner");

    println!(
        "[Phase 57 DIFF-01] Completed {} validations across Paths A–E in {:.2?} ({:.1} programs/sec). Divergences: {}",
        count,
        elapsed,
        count as f64 / elapsed.as_secs_f64().max(0.001),
        divs.len()
    );

    assert!(
        divs.is_empty(),
        "{} divergences found across Paths A–E in {} programs:\n{}",
        divs.len(),
        count,
        divs.iter()
            .take(3)
            .map(|(s, e)| format!("seed {}: {}", s, e))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
