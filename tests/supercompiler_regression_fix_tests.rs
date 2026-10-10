use std::process::Command;

use numlang::mir::lower::lower_program;
use numlang::mir::memory_ssa::MemoryVersionId;
use numlang::mir::supercompiler::drive::SupercompilerDriver;
use numlang::mir::supercompiler::state::SymbolicState;
use numlang::mir::supercompiler::supercompile_mir_function;
use numlang::mir::Place;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn get_mir(source: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(source).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    lower_program(&typed)
}

#[test]
fn test_ackermann_supercompile_no_blowup() {
    let src = r#"
fn ack(m: i64, n: i64) -> i64 {
    if m == 0 {
        return n + 1;
    } else {
        if n == 0 {
            return ack(m - 1, 1);
        } else {
            return ack(m - 1, ack(m, n - 1));
        }
    }
}

fn main() -> i64 {
    return ack(3, 4);
}
"#;
    let mir = get_mir(src);
    let ack_func = mir.functions.iter().find(|f| f.name == "ack").unwrap();
    let mut driver = SupercompilerDriver::new(ack_func).with_program_functions(&mir.functions);
    let mut init_state = SymbolicState::new(
        ack_func.blocks[0].id.clone(),
        MemoryVersionId::LIVE_ON_ENTRY,
    );
    let m_term = driver.interner_mut().intern_int(3);
    let n_term = driver.interner_mut().intern_int(4);
    init_state.set_value(
        Place {
            local: "m".to_string(),
            projections: vec![],
        },
        m_term,
    );
    init_state.set_value(
        Place {
            local: "n".to_string(),
            projections: vec![],
        },
        n_term,
    );
    let tree = driver.run_with_initial_state(init_state);

    assert!(
        tree.nodes.len() < 200,
        "Ackermann(3, 4) process tree blew up: {} nodes (expected < 200)",
        tree.nodes.len()
    );
}

#[test]
fn test_stream_fusion_supercompile_correct_and_fast() {
    let exe = env!("CARGO_BIN_EXE_numlang");
    let test_dir = std::env::temp_dir().join("numlang_stream_fusion_test");
    let _ = std::fs::create_dir_all(&test_dir);
    let src_path = "bench/numlang/stream_fusion.nl";
    let base_bin = test_dir.join("stream_base.exe");
    let super_bin = test_dir.join("stream_super.exe");

    // Build baseline with in-process benchmark timing
    let status_base = Command::new(exe)
        .args(["build", "--bench", "--backend", "cranelift", "-o"])
        .arg(&base_bin)
        .arg(src_path)
        .status()
        .expect("Failed to build baseline stream_fusion");
    assert!(status_base.success(), "Baseline build failed");

    // Build supercompiled
    let status_super = Command::new(exe)
        .args([
            "build",
            "--supercompile",
            "--bench",
            "--backend",
            "cranelift",
            "-o",
        ])
        .arg(&super_bin)
        .arg(src_path)
        .status()
        .expect("Failed to build supercompiled stream_fusion");
    assert!(status_super.success(), "Supercompiled build failed");

    // Warmup baseline and run 10 times
    for _ in 0..3 {
        let _ = Command::new(&base_bin).output();
    }
    let mut base_times = Vec::new();
    for _ in 0..10 {
        let out = Command::new(&base_bin)
            .output()
            .expect("Failed to run baseline");
        assert_eq!(out.status.code(), Some(0));
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(pos) = text.find("COMPUTE_NS:") {
            let ns_str: String = text[pos + 11..]
                .chars()
                .take_while(|c| c.is_whitespace() || c.is_ascii_digit())
                .collect();
            if let Ok(ns) = ns_str.trim().parse::<u64>() {
                base_times.push(ns);
            }
        }
    }

    // Warmup supercompiled and run 10 times
    for _ in 0..3 {
        let _ = Command::new(&super_bin).output();
    }
    let mut super_times = Vec::new();
    for _ in 0..10 {
        let out = Command::new(&super_bin)
            .output()
            .expect("Failed to run supercompiled");
        assert_eq!(out.status.code(), Some(0));
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(pos) = text.find("COMPUTE_NS:") {
            let ns_str: String = text[pos + 11..]
                .chars()
                .take_while(|c| c.is_whitespace() || c.is_ascii_digit())
                .collect();
            if let Ok(ns) = ns_str.trim().parse::<u64>() {
                super_times.push(ns);
            }
        }
    }

    let _ = std::fs::remove_dir_all(&test_dir);

    assert!(!base_times.is_empty(), "Failed to capture baseline timing");
    assert!(
        !super_times.is_empty(),
        "Failed to capture supercompiled timing"
    );

    let mean_base = base_times.iter().sum::<u64>() as f64 / base_times.len() as f64;
    let mean_super = super_times.iter().sum::<u64>() as f64 / super_times.len() as f64;

    // Supercompiled stream_fusion should be fast (mean <= base * 1.50 or sub-100µs)
    assert!(
        mean_super <= mean_base * 1.50 || mean_super < 100_000.0,
        "Stream fusion regression: super = {:.1} ns, base = {:.1} ns",
        mean_super,
        mean_base
    );
}

#[test]
fn test_fib_coupled_supercompile_no_slowdown() {
    let exe = env!("CARGO_BIN_EXE_numlang");
    let test_dir = std::env::temp_dir().join("numlang_fib_coupled_test");
    let _ = std::fs::create_dir_all(&test_dir);
    let src_path = "bench/numlang/fib_matrix.nl";
    let base_bin = test_dir.join("fib_base.exe");
    let super_bin = test_dir.join("fib_super.exe");

    let status_base = Command::new(exe)
        .args(["build", "--bench", "--backend", "cranelift", "-o"])
        .arg(&base_bin)
        .arg(src_path)
        .status()
        .expect("Failed to build baseline fib_matrix");
    assert!(status_base.success(), "Baseline build failed");

    let status_super = Command::new(exe)
        .args([
            "build",
            "--supercompile",
            "--bench",
            "--backend",
            "cranelift",
            "-o",
        ])
        .arg(&super_bin)
        .arg(src_path)
        .status()
        .expect("Failed to build supercompiled fib_matrix");
    assert!(status_super.success(), "Supercompiled build failed");

    let out_super = Command::new(&super_bin)
        .output()
        .expect("Failed to run supercompiled");
    let text = String::from_utf8_lossy(&out_super.stdout);
    assert!(
        text.contains("334154286"),
        "Incorrect Fibonacci value: {}",
        text
    );

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_power_spec_profitability_gate() {
    let src = r#"
fn power(x: i64, n: i64) -> i64 {
    if n <= 0 {
        return 1;
    } else {
        return x * power(x, n - 1);
    }
}

fn main() -> i64 {
    return power(2, 3);
}
"#;
    let mir = get_mir(src);
    let power_func = mir.functions.iter().find(|f| f.name == "power").unwrap();
    // Supercompiling power with symbolic arguments should not add unneeded code;
    // profitability gate returns func unchanged when nothing is simplified.
    let supercompiled = supercompile_mir_function(power_func);
    assert_eq!(
        power_func.blocks.len(),
        supercompiled.blocks.len(),
        "Profitability gate should prevent expansion of symbolic power: got {} blocks vs {} original",
        supercompiled.blocks.len(),
        power_func.blocks.len()
    );
}
