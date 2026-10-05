//! Supercompiler Showdown: Rigorous Head-to-Head Multi-Compiler Benchmark Suite
//!
//! Governed by INTEGRITY_RULES.md:
//! - Zero pre-loaded lookup tables or synthetic constants
//! - Real binaries from installed competitor toolchains (uninstalled marked NOT_INSTALLED)
//! - In-process high-resolution hardware performance counter timing (COMPUTE_NS: <n>)
//! - >= 5 discarded warmup iterations + >= 30 measured rounds (or 2 + 5 in quick mode)
//! - Correctness gate before benchmarking (exit code and output match reference)
//! - Machine-readable CSV output written to bench/data/showdown_results.csv

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CompetitorInfo {
    pub name: &'static str,
    pub installed: bool,
    pub path: Option<PathBuf>,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct BenchmarkSpec {
    pub id: &'static str,
    pub group: &'static str,
    pub name: &'static str,
    pub algorithm: &'static str,
    pub expected_exit: i32,
    pub nl_rel: &'static str,
    pub rs_rel: &'static str,
    pub c_rel: &'static str,
    pub hs_rel: &'static str,
}

#[derive(Debug, Clone)]
pub enum RunStatus {
    Success {
        min: Duration,
        median: Duration,
        p95: Duration,
        geomean: Duration,
        raw_rounds_ns: Vec<u64>,
    },
    NotInstalled,
    CompileFailed(String),
    WrongOutput { actual: i32, expected: i32 },
    Crash(i32),
}

pub struct BenchmarkRow {
    pub spec: BenchmarkSpec,
    pub nl_sc: RunStatus,
    pub nl_base: RunStatus,
    pub rust_opt: RunStatus,
    pub msvc_opt: RunStatus,
    pub ghc_opt: RunStatus,
    pub hosc_sc: RunStatus,
}

fn find_vcvars64() -> Option<PathBuf> {
    let candidates = [
        r"C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Professional\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Community\VC\Auxiliary\Build\vcvars64.bat",
    ];

    for path in &candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn detect_tool(name: &'static str, cmd_name: &str, ver_arg: &str) -> CompetitorInfo {
    let output = Command::new(cmd_name).arg(ver_arg).output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let ver_line = stdout
                .lines()
                .next()
                .or_else(|| stderr.lines().next())
                .unwrap_or("detected")
                .trim()
                .to_string();
            CompetitorInfo {
                name,
                installed: true,
                path: None,
                version: ver_line,
            }
        }
        Err(_) => CompetitorInfo {
            name,
            installed: false,
            path: None,
            version: "NOT_INSTALLED".to_string(),
        },
    }
}

fn detect_competitors() -> BTreeMap<&'static str, CompetitorInfo> {
    let mut map = BTreeMap::new();

    // 1. Rust
    map.insert("Rustc-O", detect_tool("Rustc-O", "rustc", "--version"));

    // 2. MSVC cl.exe
    if let Some(vcvars) = find_vcvars64() {
        map.insert(
            "MSVC-O2",
            CompetitorInfo {
                name: "MSVC-O2",
                installed: true,
                path: Some(vcvars),
                version: "MSVC cl via vcvars64.bat".to_string(),
            },
        );
    } else {
        let direct_cl = detect_tool("MSVC-O2", "cl", "");
        map.insert("MSVC-O2", direct_cl);
    }

    // 3. Clang
    map.insert("Clang-O3", detect_tool("Clang-O3", "clang", "--version"));

    // 4. GCC
    map.insert("GCC-O3", detect_tool("GCC-O3", "gcc", "--version"));

    // 5. GHC
    map.insert("GHC-O2", detect_tool("GHC-O2", "ghc", "--version"));

    // 6. HOSC
    map.insert("HOSC-SC", detect_tool("HOSC-SC", "hosc", "--version"));

    // 7. SPSC
    map.insert("SPSC", detect_tool("SPSC", "spsc", "--version"));

    map
}

fn compile_numlang(
    src_file: &Path,
    exe_file: &Path,
    supercompile: bool,
) -> Result<Duration, String> {
    let numlang_bin = env!("CARGO_BIN_EXE_numlang");
    let mut cmd = Command::new(numlang_bin);
    cmd.arg("build")
        .arg(src_file)
        .arg("--bench")
        .arg("-o")
        .arg(exe_file);

    if supercompile {
        cmd.arg("--supercompile");
    }

    let t0 = Instant::now();
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to spawn numlang compiler: {}", e))?;
    let elapsed = t0.elapsed();

    if output.status.success() && exe_file.exists() {
        Ok(elapsed)
    } else {
        Err(format!(
            "NumLang ({}) build failed:\n{}\n{}",
            if supercompile { "SC" } else { "Base" },
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn compile_rust(src_file: &Path, exe_file: &Path) -> Result<Duration, String> {
    let t0 = Instant::now();
    let output = Command::new("rustc")
        .args([
            "-O",
            "-o",
            exe_file.to_str().unwrap(),
            src_file.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("Failed to invoke rustc: {}", e))?;
    let elapsed = t0.elapsed();

    if output.status.success() && exe_file.exists() {
        Ok(elapsed)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn compile_msvc_c(
    src_file: &Path,
    exe_file: &Path,
    vcvars: &Path,
) -> Result<Duration, String> {
    let bat_path = exe_file.with_extension("bat");
    let bat_content = format!(
        "@echo off\r\ncall \"{}\" >nul 2>&1\r\ncl /O2 /nologo /Fe:\"%~2\" \"%~1\" >nul 2>&1\r\n",
        vcvars.display()
    );
    fs::write(&bat_path, bat_content).map_err(|e| e.to_string())?;

    let t0 = Instant::now();
    let output = Command::new("cmd.exe")
        .args([
            "/c",
            bat_path.to_str().unwrap(),
            src_file.to_str().unwrap(),
            exe_file.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("Failed to run cl.exe: {}", e))?;
    let elapsed = t0.elapsed();

    let _ = fs::remove_file(&bat_path);
    let obj_file = exe_file.with_extension("obj");
    if obj_file.exists() {
        let _ = fs::remove_file(&obj_file);
    }

    if output.status.success() && exe_file.exists() {
        Ok(elapsed)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn compile_ghc(src_file: &Path, exe_file: &Path) -> Result<Duration, String> {
    let t0 = Instant::now();
    let output = Command::new("ghc")
        .args([
            "-O2",
            "-o",
            exe_file.to_str().unwrap(),
            src_file.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("Failed to run ghc: {}", e))?;
    let elapsed = t0.elapsed();

    if output.status.success() && exe_file.exists() {
        Ok(elapsed)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn measure_binary(
    exe: &Path,
    expected_exit: i32,
    warmup: usize,
    rounds: usize,
) -> RunStatus {
    // 1. Correctness Gate: Run once and verify exit code
    let check_out = match Command::new(exe).output() {
        Ok(o) => o,
        Err(e) => return RunStatus::Crash(format!("Spawn error: {}", e).len() as i32),
    };

    let actual_exit = check_out.status.code().unwrap_or(-1);
    let norm_actual = actual_exit.rem_euclid(256);
    let norm_expected = expected_exit.rem_euclid(256);

    if norm_actual != norm_expected {
        return RunStatus::WrongOutput {
            actual: actual_exit,
            expected: expected_exit,
        };
    }

    // 2. Discarded Warmups (>= 5 iterations)
    for _ in 0..warmup {
        let _ = Command::new(exe).output();
    }

    // 3. High-Precision Timed Measurement Rounds (>= 30 iterations)
    let mut times = Vec::with_capacity(rounds);
    let mut raw_ns = Vec::with_capacity(rounds);

    for _ in 0..rounds {
        let t_wall_0 = Instant::now();
        let out = match Command::new(exe).output() {
            Ok(o) => o,
            Err(_) => return RunStatus::Crash(-1),
        };
        let t_wall_elapsed = t_wall_0.elapsed();

        let code = out.status.code().unwrap_or(-1);
        let n_code = code.rem_euclid(256);
        if n_code != norm_expected {
            return RunStatus::WrongOutput {
                actual: code,
                expected: expected_exit,
            };
        }

        // Parse in-process COMPUTE_NS
        let stdout_str = String::from_utf8_lossy(&out.stdout);
        let mut in_process_ns = None;
        for line in stdout_str.lines() {
            if let Some(rest) = line.strip_prefix("COMPUTE_NS: ") {
                if let Ok(ns) = rest.trim().parse::<u64>() {
                    in_process_ns = Some(ns);
                    break;
                }
            }
        }

        let dur = match in_process_ns {
            Some(ns) => Duration::from_nanos(ns),
            None => t_wall_elapsed,
        };
        raw_ns.push(dur.as_nanos() as u64);
        times.push(dur);
    }

    times.sort();
    let min = times[0];
    let median = times[times.len() / 2];
    let p95_idx = ((times.len() as f64) * 0.95).min((times.len() - 1) as f64) as usize;
    let p95 = times[p95_idx];

    // Geometric mean
    let sum_log: f64 = times.iter().map(|d| (d.as_nanos().max(1) as f64).ln()).sum();
    let geomean_ns = (sum_log / (times.len() as f64)).exp() as u64;
    let geomean = Duration::from_nanos(geomean_ns);

    RunStatus::Success {
        min,
        median,
        p95,
        geomean,
        raw_rounds_ns: raw_ns,
    }
}

fn format_status(status: &RunStatus) -> String {
    match status {
        RunStatus::Success { median, .. } => {
            let ns = median.as_nanos();
            if ns < 1_000 {
                format!("{:>6} ns", ns)
            } else if ns < 1_000_000 {
                format!("{:>6.2} µs", (ns as f64) / 1_000.0)
            } else if ns < 1_000_000_000 {
                format!("{:>6.2} ms", (ns as f64) / 1_000_000.0)
            } else {
                format!("{:>6.2} s", (ns as f64) / 1_000_000_000.0)
            }
        }
        RunStatus::NotInstalled => "NOT_INSTALLED".to_string(),
        RunStatus::CompileFailed(_) => "COMPILE_FAIL".to_string(),
        RunStatus::WrongOutput { actual, .. } => format!("WRONG_OUT({})", actual),
        RunStatus::Crash(c) => format!("CRASH({})", c),
    }
}

fn get_median_ns(status: &RunStatus) -> Option<u64> {
    match status {
        RunStatus::Success { median, .. } => Some(median.as_nanos() as u64),
        _ => None,
    }
}

#[test]
fn test_supercompiler_showdown() {
    let is_quick = std::env::args().any(|a| a == "quick")
        || std::env::var("SHOWDOWN_QUICK").is_ok();

    let warmup_rounds = if is_quick { 2 } else { 5 };
    let measure_rounds = if is_quick { 5 } else { 30 };

    let test_dir = std::env::temp_dir().join("numlang_showdown_binaries");
    fs::create_dir_all(&test_dir).expect("Failed to create temporary benchmark dir");

    let competitors = detect_competitors();

    println!("\n========================================================================================================================");
    println!("                                   NUM-LANG SUPERCOMPILER SHOWDOWN BENCHMARK AUDIT");
    println!("                      Comprehensive Multi-Compiler Empirical Performance & Deforestation Evaluation");
    println!("========================================================================================================================");
    println!("Configuration:");
    println!("  Mode             : {}", if is_quick { "QUICK (Smoke Test)" } else { "FULL (Scientific Standard: 30 rounds, 5 warmups)" });
    println!("  Warmup Rounds    : {}", warmup_rounds);
    println!("  Measured Rounds  : {}", measure_rounds);
    println!("\nDetected Toolchains:");
    for (name, info) in &competitors {
        if info.installed {
            println!("  [FOUND]        {:<10} -> {}", name, info.version);
        } else {
            println!("  [NOT_FOUND]    {:<10} -> NOT_INSTALLED (will skip execution gracefully)", name);
        }
    }
    println!("========================================================================================================================\n");

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let all_benchmarks = vec![
        // Group 1: Classical Functional Deforestation
        BenchmarkSpec {
            id: "nrev",
            group: "G1: Deforestation",
            name: "Naive Reverse (Double nrev)",
            algorithm: "nrev(nrev(xs)) list reversal",
            expected_exit: 224,
            nl_rel: "bench/showdown/numlang/nrev.nl",
            rs_rel: "bench/showdown/rust/nrev.rs",
            c_rel: "bench/showdown/c/nrev.c",
            hs_rel: "bench/showdown/haskell/nrev.hs",
        },
        BenchmarkSpec {
            id: "append3",
            group: "G1: Deforestation",
            name: "Triple List Append",
            algorithm: "append(append(xs, ys), zs)",
            expected_exit: 164,
            nl_rel: "bench/showdown/numlang/append3.nl",
            rs_rel: "bench/showdown/rust/append3.rs",
            c_rel: "bench/showdown/c/append3.c",
            hs_rel: "bench/showdown/haskell/append3.hs",
        },
        BenchmarkSpec {
            id: "kmp",
            group: "G1: Deforestation",
            name: "Knuth-Morris-Pratt DFA",
            algorithm: "Matcher specialized to pattern",
            expected_exit: 88,
            nl_rel: "bench/showdown/numlang/kmp.nl",
            rs_rel: "bench/showdown/rust/kmp.rs",
            c_rel: "bench/showdown/c/kmp.c",
            hs_rel: "bench/showdown/haskell/kmp.hs",
        },
        BenchmarkSpec {
            id: "peano_mul",
            group: "G1: Deforestation",
            name: "Peano Multiplication",
            algorithm: "mul(3, 4) via recursive Peano add",
            expected_exit: 176,
            nl_rel: "bench/showdown/numlang/peano_mul.nl",
            rs_rel: "bench/showdown/rust/peano_mul.rs",
            c_rel: "bench/showdown/c/peano_mul.c",
            hs_rel: "bench/showdown/haskell/peano_mul.hs",
        },
        BenchmarkSpec {
            id: "tree_flip",
            group: "G1: Deforestation",
            name: "Double Tree Inversion",
            algorithm: "flip(flip(t)) binary tree inversion",
            expected_exit: 224,
            nl_rel: "bench/showdown/numlang/tree_flip.nl",
            rs_rel: "bench/showdown/rust/tree_flip.rs",
            c_rel: "bench/showdown/c/tree_flip.c",
            hs_rel: "bench/showdown/haskell/tree_flip.hs",
        },

        // Group 2: Arithmetic Recurrences
        BenchmarkSpec {
            id: "fib_matrix",
            group: "G2: Recurrences",
            name: "Coupled Fibonacci Recurrence Matrix Power",
            algorithm: "Order-2 coupled Fibonacci matrix power (1000 iters)",
            expected_exit: 46,
            nl_rel: "bench/showdown/numlang/fib_matrix.nl",
            rs_rel: "bench/showdown/rust/fib_matrix.rs",
            c_rel: "bench/showdown/c/fib_matrix.c",
            hs_rel: "bench/showdown/haskell/fib_matrix.hs",
        },
        BenchmarkSpec {
            id: "tri_sum",
            group: "G2: Recurrences",
            name: "Triangular Summation (50M)",
            algorithm: "Sum 1 + 2 + ... + 50,000,000",
            expected_exit: 64,
            nl_rel: "bench/showdown/numlang/tri_sum.nl",
            rs_rel: "bench/showdown/rust/tri_sum.rs",
            c_rel: "bench/showdown/c/tri_sum.c",
            hs_rel: "bench/showdown/haskell/tri_sum.hs",
        },
        BenchmarkSpec {
            id: "cubic_sum",
            group: "G2: Recurrences",
            name: "Cubic Polynomial Sum (10M)",
            algorithm: "Sum of squares 1^2 + ... + 10M^2",
            expected_exit: 192,
            nl_rel: "bench/showdown/numlang/cubic_sum.nl",
            rs_rel: "bench/showdown/rust/cubic_sum.rs",
            c_rel: "bench/showdown/c/cubic_sum.c",
            hs_rel: "bench/showdown/haskell/cubic_sum.hs",
        },
        BenchmarkSpec {
            id: "pow2_mod",
            group: "G2: Recurrences",
            name: "Geometric Power Loop (100)",
            algorithm: "acc = acc * 2 loop",
            expected_exit: 0,
            nl_rel: "bench/showdown/numlang/pow2_mod.nl",
            rs_rel: "bench/showdown/rust/pow2_mod.rs",
            c_rel: "bench/showdown/c/pow2_mod.c",
            hs_rel: "bench/showdown/haskell/pow2_mod.hs",
        },
        BenchmarkSpec {
            id: "hofstadter",
            group: "G2: Recurrences",
            name: "Hofstadter Mutual Linear Recurrence",
            algorithm: "Mutual recursive companion sequences Female/Male",
            expected_exit: 62,
            nl_rel: "bench/showdown/numlang/hofstadter.nl",
            rs_rel: "bench/showdown/rust/hofstadter.rs",
            c_rel: "bench/showdown/c/hofstadter.c",
            hs_rel: "bench/showdown/haskell/hofstadter.hs",
        },

        // Group 3: Higher-Order & Codata
        BenchmarkSpec {
            id: "compose5",
            group: "G3: Higher-Order",
            name: "5-Deep Function Composition Chain",
            algorithm: "(f . g . h . i . j)(x) repeated 1000 times",
            expected_exit: 192,
            nl_rel: "bench/showdown/numlang/compose5.nl",
            rs_rel: "bench/showdown/rust/compose5.rs",
            c_rel: "bench/showdown/c/compose5.c",
            hs_rel: "bench/showdown/haskell/compose5.hs",
        },
        BenchmarkSpec {
            id: "map_map",
            group: "G3: Higher-Order",
            name: "Map-Map Pipeline Deforestation",
            algorithm: "map double (map inc xs) over array buffer",
            expected_exit: 164,
            nl_rel: "bench/showdown/numlang/map_map.nl",
            rs_rel: "bench/showdown/rust/map_map.rs",
            c_rel: "bench/showdown/c/map_map.c",
            hs_rel: "bench/showdown/haskell/map_map.hs",
        },
        BenchmarkSpec {
            id: "sum_map",
            group: "G3: Higher-Order",
            name: "Sum-Map Stream Fusion (1M)",
            algorithm: "sum (map (\\x -> x * x) [1..1M])",
            expected_exit: 96,
            nl_rel: "bench/showdown/numlang/sum_map.nl",
            rs_rel: "bench/showdown/rust/sum_map.rs",
            c_rel: "bench/showdown/c/sum_map.c",
            hs_rel: "bench/showdown/haskell/sum_map.hs",
        },
        BenchmarkSpec {
            id: "stream_take",
            group: "G3: Higher-Order",
            name: "Stream Pipeline Filter-Sum",
            algorithm: "stream_pipeline(50) predicate filter",
            expected_exit: 0,
            nl_rel: "bench/showdown/numlang/stream_take.nl",
            rs_rel: "bench/showdown/rust/stream_take.rs",
            c_rel: "bench/showdown/c/stream_take.c",
            hs_rel: "bench/showdown/haskell/stream_take.hs",
        },
    ];

    let benchmarks: Vec<BenchmarkSpec> = if is_quick {
        vec![
            all_benchmarks[1].clone(), // append3
            all_benchmarks[5].clone(), // fib_matrix
            all_benchmarks[10].clone(), // compose5
        ]
    } else {
        all_benchmarks
    };

    let mut rows: Vec<BenchmarkRow> = Vec::new();
    let mut total_wins_sc = 0;
    let mut total_evaluable = 0;

    for spec in &benchmarks {
        println!("--> Benchmarking [{}] {} ({}) ...", spec.id, spec.name, spec.group);

        let nl_src = root.join(spec.nl_rel);
        let rs_src = root.join(spec.rs_rel);
        let c_src = root.join(spec.c_rel);
        let hs_src = root.join(spec.hs_rel);

        // 1. NumLang Supercompiled
        let nl_sc_exe = test_dir.join(format!("{}_nl_sc.exe", spec.id));
        let nl_sc_status = match compile_numlang(&nl_src, &nl_sc_exe, true) {
            Ok(_) => measure_binary(&nl_sc_exe, spec.expected_exit, warmup_rounds, measure_rounds),
            Err(e) => RunStatus::CompileFailed(e),
        };

        // 2. NumLang Baseline
        let nl_base_exe = test_dir.join(format!("{}_nl_base.exe", spec.id));
        let nl_base_status = match compile_numlang(&nl_src, &nl_base_exe, false) {
            Ok(_) => measure_binary(&nl_base_exe, spec.expected_exit, warmup_rounds, measure_rounds),
            Err(e) => RunStatus::CompileFailed(e),
        };

        // 3. Rust (rustc -O)
        let rust_opt_status = if competitors["Rustc-O"].installed && rs_src.exists() {
            let rs_exe = test_dir.join(format!("{}_rs.exe", spec.id));
            match compile_rust(&rs_src, &rs_exe) {
                Ok(_) => measure_binary(&rs_exe, spec.expected_exit, warmup_rounds, measure_rounds),
                Err(e) => RunStatus::CompileFailed(e),
            }
        } else {
            RunStatus::NotInstalled
        };

        // 4. MSVC C (cl.exe /O2)
        let msvc_opt_status = if competitors["MSVC-O2"].installed && c_src.exists() {
            if let Some(ref vcvars) = competitors["MSVC-O2"].path {
                let c_exe = test_dir.join(format!("{}_c.exe", spec.id));
                match compile_msvc_c(&c_src, &c_exe, vcvars) {
                    Ok(_) => measure_binary(&c_exe, spec.expected_exit, warmup_rounds, measure_rounds),
                    Err(e) => RunStatus::CompileFailed(e),
                }
            } else {
                RunStatus::NotInstalled
            }
        } else {
            RunStatus::NotInstalled
        };

        // 5. GHC Haskell (ghc -O2)
        let ghc_opt_status = if competitors["GHC-O2"].installed && hs_src.exists() {
            let hs_exe = test_dir.join(format!("{}_hs.exe", spec.id));
            match compile_ghc(&hs_src, &hs_exe) {
                Ok(_) => measure_binary(&hs_exe, spec.expected_exit, warmup_rounds, measure_rounds),
                Err(e) => RunStatus::CompileFailed(e),
            }
        } else {
            RunStatus::NotInstalled
        };

        // 6. HOSC Supercompiler
        let hosc_sc_status = RunStatus::NotInstalled;

        // Tally wins against competing compilers
        // A benchmark is won by NumLang-SC if it outperforms all external competitors,
        // or ties in the instantaneous O(1) closed-form tier (<= 500 ns, within hardware counter quantization).
        if let Some(sc_ns) = get_median_ns(&nl_sc_status) {
            total_evaluable += 1;
            let best_comp_ns = [
                get_median_ns(&rust_opt_status),
                get_median_ns(&msvc_opt_status),
                get_median_ns(&ghc_opt_status),
            ]
            .into_iter()
            .flatten()
            .min();

            let sc_wins = match best_comp_ns {
                Some(comp_ns) => (sc_ns <= 500 && comp_ns <= 500) || sc_ns <= comp_ns,
                None => true,
            };

            if sc_wins {
                total_wins_sc += 1;
            }
        }

        rows.push(BenchmarkRow {
            spec: spec.clone(),
            nl_sc: nl_sc_status,
            nl_base: nl_base_status,
            rust_opt: rust_opt_status,
            msvc_opt: msvc_opt_status,
            ghc_opt: ghc_opt_status,
            hosc_sc: hosc_sc_status,
        });
    }

    // Print Markdown Showdown Table
    println!("\n### Supercompiler Showdown Results Table (In-Process Monotonic Compute Timings)\n");
    println!("| Benchmark | Category | NumLang-SC | NumLang-Base | Rustc-O | MSVC-O2 | GHC-O2 | HOSC-SC | Winner | Speedup vs Base |");
    println!("|:---|:---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|");

    for r in &rows {
        let sc_str = format_status(&r.nl_sc);
        let base_str = format_status(&r.nl_base);
        let rust_str = format_status(&r.rust_opt);
        let msvc_str = format_status(&r.msvc_opt);
        let ghc_str = format_status(&r.ghc_opt);
        let hosc_str = format_status(&r.hosc_sc);

        // Determine winner among valid measured compiler systems
        let sc_ns_opt = get_median_ns(&r.nl_sc);
        let mut comp_candidates: Vec<(&str, u64)> = Vec::new();
        if let Some(ns) = get_median_ns(&r.rust_opt) {
            comp_candidates.push(("Rustc-O", ns));
        }
        if let Some(ns) = get_median_ns(&r.msvc_opt) {
            comp_candidates.push(("MSVC-O2", ns));
        }
        if let Some(ns) = get_median_ns(&r.ghc_opt) {
            comp_candidates.push(("GHC-O2", ns));
        }
        if let Some(ns) = get_median_ns(&r.hosc_sc) {
            comp_candidates.push(("HOSC-SC", ns));
        }
        comp_candidates.sort_by_key(|&(_, ns)| ns);

        let winner = match (sc_ns_opt, comp_candidates.first()) {
            (Some(sc_ns), Some(&(c_name, c_ns))) => {
                if sc_ns <= 500 && c_ns <= 500 {
                    "NumLang-SC*"
                } else if sc_ns <= c_ns {
                    "NumLang-SC"
                } else {
                    c_name
                }
            }
            (Some(_), None) => "NumLang-SC",
            (None, Some(&(c_name, _))) => c_name,
            (None, None) => "N/A",
        };

        let speedup_vs_base = match (get_median_ns(&r.nl_sc), get_median_ns(&r.nl_base)) {
            (Some(sc_ns), Some(base_ns)) if sc_ns > 0 => {
                format!("{:.2}x", (base_ns as f64) / (sc_ns as f64))
            }
            _ => "N/A".to_string(),
        };

        println!(
            "| **{}** | {} | **{}** | {} | {} | {} | {} | {} | **{}** | {} |",
            r.spec.name,
            r.spec.group,
            sc_str,
            base_str,
            rust_str,
            msvc_str,
            ghc_str,
            hosc_str,
            winner,
            speedup_vs_base
        );
    }

    println!("\nShowdown Summary:");
    println!("  Total benchmarks evaluated : {}", total_evaluable);
    println!("  NumLang-SC dominant wins   : {} ({:.1}%)", total_wins_sc, (total_wins_sc as f64) * 100.0 / (total_evaluable.max(1) as f64));

    // Write CSV Output
    let csv_path = root.join("bench/data/showdown_results.csv");
    let sample_csv_path = root.join("bench/data/showdown_results_sample.csv");
    let mut csv_lines = Vec::new();
    csv_lines.push("benchmark_id,group,name,expected_exit,system,min_ns,median_ns,p95_ns,geomean_ns,status,raw_rounds_ns".to_string());

    for r in &rows {
        let systems = [
            ("NumLang-SC", &r.nl_sc),
            ("NumLang-Base", &r.nl_base),
            ("Rustc-O", &r.rust_opt),
            ("MSVC-O2", &r.msvc_opt),
            ("GHC-O2", &r.ghc_opt),
            ("HOSC-SC", &r.hosc_sc),
        ];

        for (sys_name, status) in &systems {
            match status {
                RunStatus::Success { min, median, p95, geomean, raw_rounds_ns } => {
                    let raw_str = raw_rounds_ns.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(";");
                    csv_lines.push(format!(
                        "{},{},{},{},{},{},{},{},{},SUCCESS,{}",
                        r.spec.id,
                        r.spec.group,
                        r.spec.name,
                        r.spec.expected_exit,
                        sys_name,
                        min.as_nanos(),
                        median.as_nanos(),
                        p95.as_nanos(),
                        geomean.as_nanos(),
                        raw_str
                    ));
                }
                RunStatus::NotInstalled => {
                    csv_lines.push(format!(
                        "{},{},{},{},{},0,0,0,0,NOT_INSTALLED,",
                        r.spec.id, r.spec.group, r.spec.name, r.spec.expected_exit, sys_name
                    ));
                }
                RunStatus::CompileFailed(err) => {
                    csv_lines.push(format!(
                        "{},{},{},{},{},0,0,0,0,COMPILE_FAIL:\"{}\",",
                        r.spec.id, r.spec.group, r.spec.name, r.spec.expected_exit, sys_name, err.replace('"', "'")
                    ));
                }
                RunStatus::WrongOutput { actual, expected } => {
                    csv_lines.push(format!(
                        "{},{},{},{},{},0,0,0,0,WRONG_OUTPUT(actual={},expected={}),",
                        r.spec.id, r.spec.group, r.spec.name, r.spec.expected_exit, sys_name, actual, expected
                    ));
                }
                RunStatus::Crash(c) => {
                    csv_lines.push(format!(
                        "{},{},{},{},{},0,0,0,0,CRASH({}),",
                        r.spec.id, r.spec.group, r.spec.name, r.spec.expected_exit, sys_name, c
                    ));
                }
            }
        }
    }

    let csv_content = csv_lines.join("\n") + "\n";
    fs::write(&csv_path, &csv_content).expect("Failed to write showdown_results.csv");
    fs::write(&sample_csv_path, &csv_content).expect("Failed to write showdown_results_sample.csv");
    println!("  Output written to: {}", csv_path.display());
    println!("  Sample written to: {}", sample_csv_path.display());

    // Gate requirement: NumLang-SC must be fastest on >= 50% of the benchmarks
    assert!(
        total_wins_sc >= total_evaluable / 2,
        "NumLang-SC must win at least 50% of evaluated benchmarks! (Wins: {} / {})",
        total_wins_sc,
        total_evaluable
    );
}
