//! Phase 27: Benchmark Correctness Integration Tests
//! Verifies that canonical NumLang benchmarks compile cleanly both with and without
//! supercompilation, execute to completion without crashing, and produce
//! bit-for-bit identical checksums and return codes.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn run_benchmark(nl_path: &PathBuf, supercompile: bool) -> (i32, String) {
    let id = format!(
        "{}_{:?}_{}_{}",
        std::process::id(),
        std::thread::current().id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid time")
            .as_nanos(),
        if supercompile { "super" } else { "base" }
    )
    .replace(['(', ')', ' '], "_");

    let test_dir = std::env::temp_dir().join(format!("numlang_bench_{}", id));
    fs::create_dir_all(&test_dir).expect("create test_dir");
    let exe_path = test_dir.join("bench.exe");

    let numlang_bin = env!("CARGO_BIN_EXE_numlang");
    let mut build_cmd = Command::new(numlang_bin);
    build_cmd.arg("build");
    if supercompile {
        build_cmd.arg("--supercompile");
    }
    build_cmd.arg(nl_path);
    build_cmd.arg("-o");
    build_cmd.arg(&exe_path);

    let build_out = build_cmd.output().expect("execute numlang build");
    assert!(
        build_out.status.success(),
        "Build failed for {:?} (supercompile={}): {}",
        nl_path,
        supercompile,
        String::from_utf8_lossy(&build_out.stderr)
    );

    let mut run_cmd = Command::new(&exe_path);
    let run_out = run_cmd.output().expect("execute bench binary");

    let _ = fs::remove_dir_all(&test_dir);

    let code = run_out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&run_out.stdout).to_string();
    (code, stdout)
}

#[test]
fn test_canonical_kmp() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("bench/numlang/kmp.nl");
    assert!(src.exists(), "kmp.nl must exist");

    let (base_code, base_out) = run_benchmark(&src, false);
    let (sc_code, sc_out) = run_benchmark(&src, true);

    assert_eq!(
        base_code, 88,
        "KMP baseline exit code expected 88 (600 % 256)"
    );
    assert_eq!(sc_code, 88, "KMP supercompiled exit code expected 88");
    assert_eq!(base_out.trim(), "600");
    assert_eq!(sc_out.trim(), "600");
}

#[test]
fn test_canonical_double_nrev() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("bench/numlang/double_nrev.nl");
    assert!(src.exists(), "double_nrev.nl must exist");

    let (base_code, base_out) = run_benchmark(&src, false);
    let (sc_code, sc_out) = run_benchmark(&src, true);

    assert_eq!(
        base_code, 224,
        "double_nrev baseline exit code expected 224 (12000 % 256)"
    );
    assert_eq!(
        sc_code, 224,
        "double_nrev supercompiled exit code expected 224"
    );
    assert_eq!(base_out.trim(), "12000");
    assert_eq!(sc_out.trim(), "12000");
}

#[test]
fn test_canonical_peano_mul() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("bench/numlang/peano_mul.nl");
    assert!(src.exists(), "peano_mul.nl must exist");

    let (base_code, base_out) = run_benchmark(&src, false);
    let (sc_code, sc_out) = run_benchmark(&src, true);

    assert_eq!(
        base_code, 176,
        "peano_mul baseline exit code expected 176 (1200 % 256)"
    );
    assert_eq!(
        sc_code, 176,
        "peano_mul supercompiled exit code expected 176"
    );
    assert_eq!(base_out.trim(), "1200");
    assert_eq!(sc_out.trim(), "1200");
}

#[test]
fn test_canonical_power_spec() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("bench/numlang/power_spec.nl");
    assert!(src.exists(), "power_spec.nl must exist");

    let (base_code, base_out) = run_benchmark(&src, false);
    let (sc_code, sc_out) = run_benchmark(&src, true);

    assert_eq!(
        base_code, 202,
        "power_spec baseline exit code expected 202 (773133258 % 256)"
    );
    assert_eq!(
        sc_code, 202,
        "power_spec supercompiled exit code expected 202"
    );
    assert_eq!(base_out.trim(), "773133258");
    assert_eq!(sc_out.trim(), "773133258");
}

#[test]
fn test_canonical_nrev() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("bench/numlang/nrev.nl");
    assert!(src.exists(), "nrev.nl must exist");

    let (base_code, base_out) = run_benchmark(&src, false);
    let (sc_code, sc_out) = run_benchmark(&src, true);

    assert_eq!(
        base_code, 224,
        "nrev baseline exit code expected 224 (12000 % 256)"
    );
    assert_eq!(sc_code, 224, "nrev supercompiled exit code expected 224");
    assert_eq!(base_out.trim(), "12000");
    assert_eq!(sc_out.trim(), "12000");
}
