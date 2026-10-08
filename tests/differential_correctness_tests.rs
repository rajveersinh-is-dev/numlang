use numlang::testing::random_program_gen::generate_random_program;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

fn run_numlang_code(code: &str, supercompile: bool) -> Option<i32> {
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
    let test_dir = std::env::temp_dir().join(format!("numlang_diff_{}", id));
    fs::create_dir_all(&test_dir).expect("Failed to create test_dir");
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).expect("Failed to write test source");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run");
    if supercompile {
        cmd.arg("--supercompile");
    }
    cmd.arg(&src_file);

    let output = cmd.output().expect("Failed to run numlang program");

    let _ = fs::remove_dir_all(&test_dir);
    output.status.code()
}

#[test]
fn test_differential_random_programs_small() {
    // 1000 programs — fast enough for CI using thread pool
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
        .min(16);
    let current_seed = AtomicU64::new(0);
    let divergences = Mutex::new(Vec::new());

    std::thread::scope(|s| {
        for _ in 0..num_threads {
            s.spawn(|| loop {
                let seed = current_seed.fetch_add(1, Ordering::Relaxed);
                if seed >= 1000 {
                    break;
                }
                let prog = generate_random_program(seed);
                let norm = run_numlang_code(&prog, false);
                let sc = run_numlang_code(&prog, true);
                if norm != sc {
                    let mut divs = divergences.lock().expect("mutex lock");
                    divs.push((seed, norm, sc, prog));
                }
            });
        }
    });

    let divergences = divergences.into_inner().expect("mutex into_inner");
    assert!(
        divergences.is_empty(),
        "{} divergences found:\n{}",
        divergences.len(),
        divergences
            .iter()
            .take(3)
            .map(|(s, n, sc, p)| format!("seed={}: norm={:?} sc={:?}\n---\n{}\n---", s, n, sc, p))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
#[ignore] // Run manually: cargo test -- --ignored test_differential_10k
fn test_differential_10k() {
    let mut divergences = Vec::new();
    for seed in 0u64..10_000 {
        let prog = generate_random_program(seed);
        let norm = run_numlang_code(&prog, false);
        let sc = run_numlang_code(&prog, true);
        if norm != sc {
            divergences.push((seed, norm, sc));
        }
    }
    assert!(
        divergences.is_empty(),
        "{} divergences in 10K programs: {:?}",
        divergences.len(),
        &divergences[..5.min(divergences.len())]
    );
}
