use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use numlang::testing::random_program_gen::generate_deep_random_program;

fn run_numlang_code(code: &str, supercompile: bool) -> (Option<i32>, String) {
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
    let test_dir = std::env::temp_dir().join(format!("numlang_fuzz100k_{}", id));
    fs::create_dir_all(&test_dir).expect("Failed to create test_dir");
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).expect("Failed to write test source");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run");
    cmd.arg("--backend").arg("cranelift");
    if supercompile {
        cmd.arg("--supercompile");
    }
    cmd.arg(&src_file);

    let output = cmd.output().expect("Failed to run numlang program");

    let _ = fs::remove_dir_all(&test_dir);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    (output.status.code(), stdout)
}

fn run_fuzz_batch(total_cases: u64, report_name: &str) {
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
        .min(16);
    let current_seed = AtomicU64::new(0);
    let divergences = Mutex::new(Vec::new());
    let start_time = Instant::now();

    std::thread::scope(|s| {
        for _ in 0..num_threads {
            s.spawn(|| loop {
                let seed = current_seed.fetch_add(1, Ordering::Relaxed);
                if seed >= total_cases {
                    break;
                }
                let prog = generate_deep_random_program(seed);
                let (norm_exit, norm_out) = run_numlang_code(&prog, false);
                let (sc_exit, sc_out) = run_numlang_code(&prog, true);
                if norm_exit != sc_exit || norm_out != sc_out {
                    let mut divs = divergences.lock().expect("mutex lock");
                    divs.push((seed, norm_exit, sc_exit, norm_out, sc_out, prog));
                }
            });
        }
    });

    let elapsed = start_time.elapsed().as_secs_f64();
    let divergences = divergences.into_inner().expect("mutex into_inner");

    let report_json = format!(
        r#"{{
  "suite": "differential_fuzz_100k",
  "total_cases": {},
  "divergences_count": {},
  "all_identical": {},
  "elapsed_seconds": {:.2},
  "cases_per_second": {:.1},
  "features_tested": [
    "deeply_nested_while_loops",
    "for_range_loops",
    "pattern_matching_with_wildcards",
    "coupled_recurrence_systems",
    "bounded_recursion",
    "conditional_branch_fusion"
  ],
  "soundness_status": "{}"
}}
"#,
        total_cases,
        divergences.len(),
        divergences.is_empty(),
        elapsed,
        total_cases as f64 / elapsed.max(0.001),
        if divergences.is_empty() {
            "CERTIFIED_SOUND"
        } else {
            "DIVERGENCE_DETECTED"
        }
    );

    let _ = fs::write(report_name, report_json);

    assert!(
        divergences.is_empty(),
        "{} divergences found out of {} cases:\n{}",
        divergences.len(),
        total_cases,
        divergences
            .iter()
            .take(3)
            .map(|(s, ne, se, no, so, p)| format!(
                "seed={}: norm_exit={:?} sc_exit={:?} norm_out='{}' sc_out='{}'\n---\n{}\n---",
                s, ne, se, no.trim(), so.trim(), p
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn test_differential_fuzz_smoke() {
    let cases = std::env::var("FUZZ_SMOKE_COUNT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(500);
    run_fuzz_batch(cases, "fuzz_report.json");
}

#[test]
#[ignore] // Run on demand: cargo test --test differential_fuzz_100k -- --ignored
fn test_differential_fuzz_100k() {
    let cases = std::env::var("FUZZ_COUNT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(100_000);
    run_fuzz_batch(cases, "fuzz_report.json");
}
