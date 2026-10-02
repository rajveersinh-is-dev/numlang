use std::path::PathBuf;
use std::process::Command;

fn get_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_benchmark(rel_path: &str, supercompile: bool) -> (Option<i32>, String, String) {
    let full_path = get_repo_root().join(rel_path);
    assert!(full_path.exists(), "Benchmark path does not exist: {}", full_path.display());

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run");
    if supercompile {
        cmd.arg("--supercompile");
    }
    cmd.arg(&full_path);

    let output = cmd.output().unwrap_or_else(|e| {
        panic!("Failed to execute numlang run on {}: {}", rel_path, e)
    });

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.code(), stdout, stderr)
}

#[test]
fn test_all_30_benchmarks_compile() {
    let benchmarks = [
        ("bench/numlang/kmp.nl", 88),
        ("bench/numlang/boyer_moore.nl", 142),
        ("bench/numlang/rabin_karp.nl", 142),
        ("bench/numlang/lcs.nl", 15),
        ("bench/numlang/merge_sort.nl", 1),
        ("bench/numlang/quick_sort.nl", 2),
        ("bench/numlang/radix_sort.nl", 2),
        ("bench/numlang/bfs.nl", 5),
        ("bench/numlang/dijkstra.nl", 25),
        ("bench/numlang/floyd_warshall.nl", 24),
        ("bench/numlang/newton_sqrt.nl", 134),
        ("bench/numlang/euler_pi.nl", 183),
        ("bench/numlang/sieve.nl", 44),
        ("bench/numlang/power_spec.nl", 202),
        ("bench/numlang/peano_mul.nl", 176),
        ("bench/numlang/fib_matrix.nl", 46),
        ("bench/numlang/tribonacci.nl", 127),
        ("bench/numlang/hofstadter.nl", 62),
        ("bench/numlang/ackermann.nl", 106),
        ("bench/numlang/nrev.nl", 224),
        ("bench/numlang/double_nrev.nl", 224),
        ("bench/numlang/append3.nl", 164),
        ("bench/numlang/tree_flip.nl", 224),
        ("bench/numlang/matvec_4x4.nl", 24),
        ("bench/numlang/matrix_multiply.nl", 11),
        ("bench/numlang/raytracer_sphere.nl", 77),
        ("bench/numlang/jacobi_stencil.nl", 33),
        ("bench/numlang/stream_fusion.nl", 0),
        ("bench/numlang/map_map_fusion.nl", 164),
        ("bench/numlang/fold_map.nl", 129),
    ];

    assert_eq!(benchmarks.len(), 30, "Suite must contain exactly 30 benchmarks");

    for (path, expected_code) in benchmarks {
        let (code, stdout, stderr) = run_benchmark(path, false);
        assert_eq!(
            code,
            Some(expected_code),
            "Benchmark {} failed with code {:?}, expected {}.\nStdout: {}\nStderr: {}",
            path,
            code,
            expected_code,
            stdout,
            stderr
        );
    }
}

#[test]
fn test_new_string_benchmarks_correctness() {
    let string_benchmarks = [
        ("bench/numlang/boyer_moore.nl", 142),
        ("bench/numlang/rabin_karp.nl", 142),
        ("bench/numlang/lcs.nl", 15),
    ];

    for (path, expected_code) in string_benchmarks {
        let (base_code, base_out, _) = run_benchmark(path, false);
        let (sc_code, sc_out, _) = run_benchmark(path, true);

        assert_eq!(base_code, Some(expected_code), "Baseline failed on {}", path);
        assert_eq!(sc_code, Some(expected_code), "Supercompiled failed on {}", path);
        assert_eq!(
            base_out.trim(),
            sc_out.trim(),
            "Outputs diverged on {}: base='{}', sc='{}'",
            path,
            base_out.trim(),
            sc_out.trim()
        );
    }
}

#[test]
fn test_new_sort_benchmarks_correctness() {
    let sort_benchmarks = [
        ("bench/numlang/merge_sort.nl", 1),
        ("bench/numlang/quick_sort.nl", 2),
        ("bench/numlang/radix_sort.nl", 2),
    ];

    for (path, expected_code) in sort_benchmarks {
        let (base_code, base_out, _) = run_benchmark(path, false);
        let (sc_code, sc_out, _) = run_benchmark(path, true);

        assert_eq!(base_code, Some(expected_code), "Baseline failed on {}", path);
        assert_eq!(sc_code, Some(expected_code), "Supercompiled failed on {}", path);
        assert_eq!(
            base_out.trim(),
            sc_out.trim(),
            "Sorting output diverged on {}: base='{}', sc='{}'",
            path,
            base_out.trim(),
            sc_out.trim()
        );
    }
}

#[test]
fn test_new_numerical_benchmarks_correctness() {
    let (sqrt_code, sqrt_out, _) = run_benchmark("bench/numlang/newton_sqrt.nl", true);
    assert!(
        sqrt_code == Some(134) || sqrt_code == Some(138),
        "Newton sqrt unexpected code: {:?}",
        sqrt_code
    );
    assert_eq!(sqrt_out.trim(), "1414", "Newton sqrt stdout should be 1414");

    let (pi_code, pi_out, _) = run_benchmark("bench/numlang/euler_pi.nl", true);
    assert_eq!(pi_code, Some(183), "Euler pi unexpected code: {:?}", pi_code);
    assert_eq!(pi_out.trim(), "31415", "Euler pi stdout should be 31415");
}

#[test]
fn test_fusion_benchmarks_produce_single_loop() {
    let full_path = get_repo_root().join("bench/numlang/map_map_fusion.nl");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg(&full_path);
    cmd.arg("--emit-supercompiled-mir");

    let output = cmd.output().expect("Failed to emit supercompiled MIR");
    assert!(output.status.success(), "Failed to emit supercompiled MIR");

    let mir_str = String::from_utf8_lossy(&output.stdout);
    // Parse or check MirFunction block count for main
    // main in map_map_fusion should have fused the loop, resulting in <= 5 basic blocks
    let main_part = mir_str.split("name: \"main\"").nth(1).unwrap_or("");
    let main_block_count = main_part.matches("MirBasicBlock {").count();
    assert!(
        main_block_count <= 5,
        "Supercompiled map_map_fusion residual for main should have <= 5 basic blocks (fused loop), found {}",
        main_block_count
    );

    // Also run with --supercompile to ensure correctness
    let (code, stdout, _) = run_benchmark("bench/numlang/map_map_fusion.nl", true);
    assert_eq!(code, Some(164), "map_map_fusion should exit with 164");
    assert_eq!(stdout.trim(), "420", "map_map_fusion should print 420");
}
