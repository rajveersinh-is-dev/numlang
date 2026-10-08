use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

fn compile_numlang(
    src: &str,
    test_dir: &Path,
    name: &str,
    backend: &str,
    opt_level: &str,
    supercompile: bool,
) -> Result<(PathBuf, Duration), String> {
    let clean_name = name.replace(['/', ' ', '(', ')', ':'], "_");
    let suffix = format!("{}_{}_opt{}", clean_name, backend, opt_level);
    let src_file = test_dir.join(format!("{}.nl", suffix));
    let exe_file = test_dir.join(format!("{}.exe", suffix));
    fs::write(&src_file, src).map_err(|e| format!("Failed to write source: {}", e))?;

    let numlang_bin = env!("CARGO_BIN_EXE_numlang");
    let mut cmd = Command::new(numlang_bin);
    cmd.arg("build")
        .arg(&src_file)
        .arg("--bench")
        .arg("--backend")
        .arg(backend)
        .arg("--opt-level")
        .arg(opt_level)
        .arg("-o")
        .arg(&exe_file);

    if supercompile {
        cmd.arg("--supercompile");
    }

    let t0 = Instant::now();
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute compiler: {}", e))?;
    let compile_time = t0.elapsed();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        return Err(format!(
            "Compilation failed:\nSTDOUT: {}\nSTDERR: {}",
            stdout, stderr
        ));
    }

    Ok((exe_file, compile_time))
}

fn run_binary(exe: &Path) -> Result<(i32, u64), String> {
    let is_quick = std::env::var("QUICK_BENCHMARKS").is_ok();
    let warmup_rounds = if is_quick { 1 } else { 5 };
    let measure_rounds = if is_quick { 2 } else { 30 };

    let mut last_code = -1;

    // Warmup
    for _ in 0..warmup_rounds {
        let output = Command::new(exe)
            .output()
            .map_err(|e| format!("Failed to run binary {}: {}", exe.display(), e))?;
        last_code = output.status.code().unwrap_or(-1);
    }

    let mut total_ns = 0u64;
    for _ in 0..measure_rounds {
        let output = Command::new(exe)
            .output()
            .map_err(|e| format!("Failed to run binary {}: {}", exe.display(), e))?;
        last_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut compute_ns = 0u64;
        for line in stdout.lines() {
            if let Some(rest) = line.strip_prefix("COMPUTE_NS: ") {
                if let Ok(v) = rest.trim().parse::<u64>() {
                    compute_ns = v;
                }
            }
        }
        total_ns += compute_ns;
    }

    Ok((last_code, total_ns / measure_rounds as u64))
}

fn format_duration(ns: u64) -> String {
    if ns < 100 {
        "< 100ns".to_string()
    } else if ns < 1_000 {
        format!("{}ns", ns)
    } else if ns < 1_000_000 {
        format!("{:.1}µs", ns as f64 / 1_000.0)
    } else if ns < 1_000_000_000 {
        format!("{:.2}ms", ns as f64 / 1_000_000.0)
    } else {
        format!("{:.3}s", ns as f64 / 1_000_000_000.0)
    }
}

#[test]
fn test_llvm_vs_cranelift_comparative_benchmarks() {
    let test_dir = std::env::temp_dir().join(format!("numlang_llvm_bench_{}", std::process::id()));
    let _ = fs::create_dir_all(&test_dir);

    // 1. Fibonacci 1M (supercompiled)
    let fib_src = r#"
fn fib(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let temp: i64 = a + b;
        a = b;
        b = temp;
        i = i + 1;
    }
    return a;
}

fn main() -> i64 {
    let res: i64 = fib(1000000);
    return res % 1000;
}
"#;

    // 2. Triangular Sum 50M (supercompiled)
    let tri_src = r#"
fn triangular(n: i64) -> i64 {
    let mut s: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        s = s + i;
        i = i + 1;
    }
    return s;
}

fn main() -> i64 {
    let res: i64 = triangular(50000000);
    return res % 1000;
}
"#;

    // 3. Dense Matrix Multiply 100x100
    let matmul_src = r#"
fn main() -> i64 {
    let n: i64 = 100;
    let mut total: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        let mut j: i64 = 0;
        while j < n {
            let mut k: i64 = 0;
            let mut dot: i64 = 0;
            while k < n {
                dot = dot + (i + k) * (k + j);
                k = k + 1;
            }
            total = total + dot;
            j = j + 1;
        }
        i = i + 1;
    }
    return total % 1000;
}
"#;

    // 4. Sort workload (Bubble/Insertion sort simulation on 2000 elements)
    let sort_src = r#"
fn main() -> i64 {
    let mut s: i64 = 0;
    let n: i64 = 2000;
    let mut i: i64 = 0;
    while i < n {
        let mut j: i64 = i;
        while j < n {
            if (i * 37 + j * 17) % 7 == 0 {
                s = s + 1;
            }
            j = j + 1;
        }
        i = i + 1;
    }
    return s % 1000;
}
"#;

    struct Workload<'a> {
        name: &'a str,
        src: &'a str,
        supercompile: bool,
    }

    let workloads = [
        Workload {
            name: "Fibonacci 1M (SC)",
            src: fib_src,
            supercompile: true,
        },
        Workload {
            name: "Triangular Sum 50M(SC)",
            src: tri_src,
            supercompile: true,
        },
        Workload {
            name: "Matrix Mul 100x100",
            src: matmul_src,
            supercompile: false,
        },
        Workload {
            name: "Sort/Scan 2K Elements",
            src: sort_src,
            supercompile: false,
        },
    ];

    println!("\n==========================================================================================");
    println!(" NUMLANG DUAL-BACKEND BENCHMARK SUITE: CRANELIFT vs. LLVM");
    println!("==========================================================================================");

    let mut results = Vec::new();

    for w in &workloads {
        // Compile with Cranelift -O0
        let cl_res = compile_numlang(w.src, &test_dir, w.name, "cranelift", "0", w.supercompile);
        assert!(
            cl_res.is_ok(),
            "Cranelift build failed for {}: {:?}",
            w.name,
            cl_res.err()
        );
        let (cl_exe, cl_compile_dur) = cl_res.unwrap();
        let (_cl_code, cl_runtime_ns) =
            run_binary(&cl_exe).expect("Failed running Cranelift binary");

        // Try compiling with LLVM -O3
        let llvm_res = compile_numlang(w.src, &test_dir, w.name, "llvm", "3", w.supercompile);

        match llvm_res {
            Ok((llvm_exe, llvm_compile_dur)) => {
                let (_llvm_code, llvm_runtime_ns) =
                    run_binary(&llvm_exe).expect("Failed running LLVM binary");
                let speedup = if llvm_runtime_ns > 0 {
                    cl_runtime_ns as f64 / llvm_runtime_ns as f64
                } else {
                    1.0
                };
                results.push((
                    w.name,
                    cl_compile_dur,
                    format_duration(cl_runtime_ns),
                    llvm_compile_dur,
                    format_duration(llvm_runtime_ns),
                    format!("{:.1}x", speedup),
                ));
            }
            Err(err_msg) => {
                // If LLVM backend is not compiled into binary (feature flag disabled), record as N/A with explanation
                let is_disabled = err_msg.contains("LLVM backend is not enabled")
                    || err_msg.contains("llvm-backend");
                results.push((
                    w.name,
                    cl_compile_dur,
                    format_duration(cl_runtime_ns),
                    Duration::from_millis(0),
                    if is_disabled {
                        "Requires LLVM".to_string()
                    } else {
                        "Build Error".to_string()
                    },
                    if is_disabled {
                        "N/A (Cranelift only)".to_string()
                    } else {
                        "Err".to_string()
                    },
                ));
            }
        }
    }

    println!(
        "{:<24} | {:<15} | {:<15} | {:<12}",
        "Workload", "Cranelift -O0", "LLVM -O3", "Speedup"
    );
    println!("{:-<24}-|-{:-<15}-|-{:-<15}-|-{:-<12}", "", "", "", "");

    for (name, _cl_ct, cl_rt, _llvm_ct, llvm_rt, speedup) in &results {
        println!(
            "{:<24} | {:<15} | {:<15} | {:<12}",
            name, cl_rt, llvm_rt, speedup
        );
    }
    println!("==========================================================================================\n");

    let _ = fs::remove_dir_all(&test_dir);
}
