use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

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

fn wrap_rust(src: &str) -> String {
    let s = src.replace("fn main() {", "fn main() {\n    let _bench_t0 = std::time::Instant::now();");
    let s = s.replace("std::process::exit(", "__bench_exit(&_bench_t0, ");
    let helper = r#"
fn __bench_exit(t0: &std::time::Instant, code: i32) -> ! {
    let ns = t0.elapsed().as_nanos();
    println!("COMPUTE_NS: {}", ns);
    std::process::exit(code);
}
"#;
    format!("{}\n{}", helper, s)
}

fn wrap_c(src: &str) -> String {
    format!(
        r#"
#define main __user_main
{}
#undef main
#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <intrin.h>

int __user_main(void);

int main() {{
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    _ReadWriteBarrier();
    QueryPerformanceCounter(&t0);
    _ReadWriteBarrier();
    volatile int ret = __user_main();
    _ReadWriteBarrier();
    QueryPerformanceCounter(&t1);
    _ReadWriteBarrier();
    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart;
    if (ns < 0) ns = 0;
    printf("COMPUTE_NS: %lld\n", ns);
    return ret;
}}
#else
#define _POSIX_C_SOURCE 199309L
#include <stdio.h>
#include <time.h>

int __user_main(void);

int main() {{
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
    volatile int ret = __user_main();
    clock_gettime(CLOCK_MONOTONIC, &t1);
    long long ns = (long long)(t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec);
    if (ns < 0) ns = 0;
    printf("COMPUTE_NS: %lld\n", ns);
    return ret;
}}
#endif
"#,
        src
    )
}

fn compile_numlang(src: &str, test_dir: &Path, name: &str, supercompile: bool) -> (PathBuf, Duration) {
    let flag = if supercompile { "sc" } else { "base" };
    let src_file = test_dir.join(format!("{}_{}.nl", name, flag));
    let exe_file = test_dir.join(format!("{}_{}.exe", name, flag));
    fs::write(&src_file, src).expect("Failed to write numlang source");

    let numlang_bin = env!("CARGO_BIN_EXE_numlang");
    let mut cmd = Command::new(numlang_bin);
    cmd.arg("build")
        .arg(&src_file)
        .arg("--bench")
        .arg("-o")
        .arg(&exe_file);

    if supercompile {
        cmd.arg("--supercompile");
    }

    let t0 = Instant::now();
    let output = cmd.output().expect("Failed to invoke numlang compiler");
    let compile_time = t0.elapsed();

    assert!(
        output.status.success(),
        "numlang ({}) compilation failed: {}\n{}",
        flag,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (exe_file, compile_time)
}

fn compile_rust(src: &str, test_dir: &Path, name: &str) -> Option<(PathBuf, Duration)> {
    let src_file = test_dir.join(format!("{}_rs.rs", name));
    let exe_file = test_dir.join(format!("{}_rs.exe", name));
    fs::write(&src_file, wrap_rust(src)).expect("Failed to write Rust source");

    let t0 = Instant::now();
    let output = Command::new("rustc")
        .arg("-O")
        .arg(&src_file)
        .arg("-o")
        .arg(&exe_file)
        .output()
        .ok()?;
    let compile_time = t0.elapsed();

    if output.status.success() {
        Some((exe_file, compile_time))
    } else {
        None
    }
}

fn compile_c(src: &str, test_dir: &Path, name: &str) -> Option<(PathBuf, Duration)> {
    let vcvars = find_vcvars64()?;
    let src_file = test_dir.join(format!("{}_c.c", name));
    let exe_file = test_dir.join(format!("{}_c.exe", name));
    fs::write(&src_file, wrap_c(src)).expect("Failed to write C source");

    let bat_path = test_dir.join(format!("compile_msvc_{}.bat", name));
    let bat_content = format!(
        "@echo off\r\ncall \"{}\" >nul 2>&1\r\ncl /O2 /nologo /Fo:\"%~dp2\" /Fe:\"%~2\" \"%~1\" >nul 2>&1\r\n",
        vcvars.display()
    );
    fs::write(&bat_path, bat_content).ok()?;

    let t0 = Instant::now();
    let output = Command::new("cmd.exe")
        .args(["/c", bat_path.to_str()?, src_file.to_str()?, exe_file.to_str()?])
        .output()
        .ok()?;
    let compile_time = t0.elapsed();

    let _ = fs::remove_file(&bat_path);

    if output.status.success() && exe_file.exists() {
        Some((exe_file, compile_time))
    } else {
        None
    }
}

fn benchmark_cmd(cmd: &Path, expected_exit: i32, iterations: usize) -> (Duration, i32, bool) {
    let mut compute_times = Vec::with_capacity(iterations);
    let mut last_code = -1;

    for _ in 0..iterations {
        let start = Instant::now();
        let output = Command::new(cmd).output().expect("Failed to run benchmark binary");
        let elapsed = start.elapsed();
        last_code = output.status.code().unwrap_or(-1);

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let mut compute_ns = None;
        for line in stdout_str.lines() {
            if let Some(rest) = line.strip_prefix("COMPUTE_NS: ") {
                if let Ok(ns) = rest.trim().parse::<u64>() {
                    compute_ns = Some(ns);
                    break;
                }
            }
        }

        if let Some(ns) = compute_ns {
            compute_times.push(Duration::from_nanos(ns));
        } else {
            compute_times.push(elapsed);
        }
    }

    let min_compute = *compute_times.iter().min().unwrap_or(&Duration::ZERO);
    let passed = last_code == expected_exit;
    (min_compute, last_code, passed)
}

struct HeadToHeadBench {
    name: &'static str,
    asymptotics_unoptimized: &'static str,
    asymptotics_supercompiled: &'static str,
    expected_exit: i32,
    nl_code: &'static str,
    rs_code: &'static str,
    c_code: &'static str,
    iterations: usize,
}

#[test]
fn test_run_supercompiler_head_to_head_benchmarks() {
    let test_dir = std::env::temp_dir().join("numlang_sc_benchmarks");
    fs::create_dir_all(&test_dir).unwrap();

    let benchmarks = [
        // 1. Coupled Fibonacci Recurrence (1M iterations)
        HeadToHeadBench {
            name: "Coupled Fibonacci Recurrence (1M iters)",
            asymptotics_unoptimized: "O(N)",
            asymptotics_supercompiled: "O(log N) / O(1)",
            expected_exit: -69, // fib(1_000_000) % 256
            nl_code: r#"
fn fib(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let t: i64 = a + b;
        a = b;
        b = t;
        i = i + 1;
    }
    return a;
}
fn main() -> i64 {
    return fib(1000000) % 256;
}
"#,
            rs_code: r#"
fn fib(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let t: i64 = a.wrapping_add(b);
        a = b;
        b = t;
        i = i + 1;
    }
    a
}
fn main() {
    let r = fib(1000000) % 256;
    std::process::exit(r as i32);
}
"#,
            c_code: r#"
long long fib(long long n) {
    long long a = 0;
    long long b = 1;
    long long i = 0;
    while (i < n) {
        long long t = a + b;
        a = b;
        b = t;
        i = i + 1;
    }
    return a;
}
int __user_main(void) {
    return (int)(fib(1000000) % 256);
}
"#,
            iterations: 5,
        },

        // 2. Triangular Summation (50M iterations)
        HeadToHeadBench {
            name: "Triangular Summation Loop (50M iters)",
            asymptotics_unoptimized: "O(N)",
            asymptotics_supercompiled: "O(1)",
            expected_exit: 64, // tri(50_000_000) % 256
            nl_code: r#"
fn tri_sum(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc + i;
        i = i + 1;
    }
    return acc;
}
fn main() -> i64 {
    return tri_sum(50000000) % 256;
}
"#,
            rs_code: r#"
fn tri_sum(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc.wrapping_add(i);
        i = i + 1;
    }
    acc
}
fn main() {
    let r = tri_sum(50000000) % 256;
    std::process::exit(r as i32);
}
"#,
            c_code: r#"
long long tri_sum(long long n) {
    long long acc = 0;
    long long i = 1;
    while (i <= n) {
        acc = acc + i;
        i = i + 1;
    }
    return acc;
}
int __user_main(void) {
    return (int)(tri_sum(50000000) % 256);
}
"#,
            iterations: 5,
        },

        // 3. Cubic Polynomial Sum (10M iterations)
        HeadToHeadBench {
            name: "Cubic Polynomial Sum (10M iters)",
            asymptotics_unoptimized: "O(N)",
            asymptotics_supercompiled: "O(1)",
            expected_exit: 192,
            nl_code: r#"
fn cubic_sum(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc + (i * i);
        i = i + 1;
    }
    return acc;
}
fn main() -> i64 {
    return cubic_sum(10000000) % 256;
}
"#,
            rs_code: r#"
fn cubic_sum(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc.wrapping_add(i * i);
        i = i + 1;
    }
    acc
}
fn main() {
    let r = cubic_sum(10000000) % 256;
    std::process::exit(r as i32);
}
"#,
            c_code: r#"
long long cubic_sum(long long n) {
    long long acc = 0;
    long long i = 1;
    while (i <= n) {
        acc = acc + (i * i);
        i = i + 1;
    }
    return acc;
}
int __user_main(void) {
    return (int)(cubic_sum(10000000) % 256);
}
"#,
            iterations: 5,
        },

        // 4. Interprocedural Call & Triangular Loop Fusion
        HeadToHeadBench {
            name: "Interprocedural Loop Fusion (10M iters)",
            asymptotics_unoptimized: "O(N) + calls",
            asymptotics_supercompiled: "O(1) fused",
            expected_exit: 64,
            nl_code: r#"
fn compute_tri(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc + i;
        i = i + 1;
    }
    return acc;
}
fn runner(limit: i64) -> i64 {
    return compute_tri(limit);
}
fn main() -> i64 {
    return runner(10000000) % 256;
}
"#,
            rs_code: r#"
#[inline(never)]
fn compute_tri(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc.wrapping_add(i);
        i = i + 1;
    }
    acc
}
fn runner(limit: i64) -> i64 {
    compute_tri(limit)
}
fn main() {
    let r = runner(10000000) % 256;
    std::process::exit(r as i32);
}
"#,
            c_code: r#"
__declspec(noinline) long long compute_tri(long long n) {
    long long acc = 0;
    long long i = 1;
    while (i <= n) {
        acc = acc + i;
        i = i + 1;
    }
    return acc;
}
long long runner(long long limit) {
    return compute_tri(limit);
}
int __user_main(void) {
    return (int)(runner(10000000) % 256);
}
"#,
            iterations: 5,
        },

        // 5. Geometric Power of Two Loop (100 iters)
        HeadToHeadBench {
            name: "Geometric Power Loop (100 iters)",
            asymptotics_unoptimized: "O(N)",
            asymptotics_supercompiled: "O(1)",
            expected_exit: 0,
            nl_code: r#"
fn pow2(n: i64) -> i64 {
    let mut acc: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        acc = acc * 2;
        i = i + 1;
    }
    return acc % 256;
}
fn main() -> i64 {
    return pow2(100);
}
"#,
            rs_code: r#"
fn pow2(n: i64) -> i64 {
    let mut acc: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        acc = acc.wrapping_mul(2);
        i = i + 1;
    }
    acc % 256
}
fn main() {
    let r = pow2(100);
    std::process::exit(r as i32);
}
"#,
            c_code: r#"
long long pow2(long long n) {
    long long acc = 1;
    long long i = 0;
    while (i < n) {
        acc = acc * 2;
        i = i + 1;
    }
    return acc % 256;
}
int __user_main(void) {
    return (int)pow2(100);
}
"#,
            iterations: 5,
        },
    ];

    println!("\n=====================================================================================================");
    println!("NUM-LANG SUPERCOMPILER HEAD-TO-HEAD BENCHMARK AUDIT");
    println!("Comparing NumLang Supercompiled vs NumLang Baseline vs rustc -O vs MSVC cl.exe /O2");
    println!("=====================================================================================================\n");

    for (idx, b) in benchmarks.iter().enumerate() {
        let safe_name = format!("bench_{}", idx + 1);

        // Compile NumLang Supercompiled
        let (nl_sc_exe, nl_sc_compile) = compile_numlang(b.nl_code, &test_dir, &safe_name, true);
        let (nl_sc_time, nl_sc_exit, nl_sc_ok) = benchmark_cmd(&nl_sc_exe, b.expected_exit, b.iterations);

        // Compile NumLang Baseline
        let (nl_base_exe, nl_base_compile) = compile_numlang(b.nl_code, &test_dir, &safe_name, false);
        let (nl_base_time, nl_base_exit, nl_base_ok) = benchmark_cmd(&nl_base_exe, b.expected_exit, b.iterations);

        // Compile Rust -O
        let rust_res = compile_rust(b.rs_code, &test_dir, &safe_name);

        // Compile C /O2
        let c_res = compile_c(b.c_code, &test_dir, &safe_name);

        println!("-----------------------------------------------------------------------------------------------------");
        println!("Benchmark #{}: {}", idx + 1, b.name);
        println!("Asymptotics: Baseline [{}] -> Supercompiled [{}]", b.asymptotics_unoptimized, b.asymptotics_supercompiled);
        println!("-----------------------------------------------------------------------------------------------------");

        println!("  NumLang Supercompiled : {:>10.3?} | Compile: {:>8.2?} | Exit: {:>4} (match: {})",
            nl_sc_time, nl_sc_compile, nl_sc_exit, nl_sc_ok);
        println!("  NumLang Baseline      : {:>10.3?} | Compile: {:>8.2?} | Exit: {:>4} (match: {})",
            nl_base_time, nl_base_compile, nl_base_exit, nl_base_ok);

        if let Some((rust_exe, rust_compile)) = rust_res {
            let (rust_time, rust_exit, rust_ok) = benchmark_cmd(&rust_exe, b.expected_exit, b.iterations);
            let speedup_vs_rust = rust_time.as_nanos() as f64 / nl_sc_time.as_nanos().max(1) as f64;
            println!("  Rust (rustc -O)       : {:>10.3?} | Compile: {:>8.2?} | Exit: {:>4} (match: {}) | Speedup vs Rust: {:>8.2}x",
                rust_time, rust_compile, rust_exit, rust_ok, speedup_vs_rust);
        }

        if let Some((c_exe, c_compile)) = c_res {
            let (c_time, c_exit, c_ok) = benchmark_cmd(&c_exe, b.expected_exit, b.iterations);
            let speedup_vs_c = c_time.as_nanos() as f64 / nl_sc_time.as_nanos().max(1) as f64;
            println!("  MSVC C (cl /O2)       : {:>10.3?} | Compile: {:>8.2?} | Exit: {:>4} (match: {}) | Speedup vs C:    {:>8.2}x",
                c_time, c_compile, c_exit, c_ok, speedup_vs_c);
        }

        let speedup_sc_vs_base = nl_base_time.as_nanos() as f64 / nl_sc_time.as_nanos().max(1) as f64;
        println!("  >>> Supercompiler Acceleration Factor: {:>8.2}x", speedup_sc_vs_base);
        println!();
    }
}
