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

fn compile_numlang(src: &str, test_dir: &Path, name: &str) -> PathBuf {
    let src_file = test_dir.join(format!("{}_nl.nl", name));
    let exe_file = test_dir.join(format!("{}_nl.exe", name));
    fs::write(&src_file, src).expect("Failed to write numlang source");

    let numlang_bin = env!("CARGO_BIN_EXE_numlang");
    let output = Command::new(numlang_bin)
        .arg("build")
        .arg(&src_file)
        .arg("-o")
        .arg(&exe_file)
        .output()
        .expect("Failed to invoke numlang compiler");

    assert!(
        output.status.success(),
        "numlang compilation failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    exe_file
}

fn compile_rust(src: &str, test_dir: &Path, name: &str) -> Option<PathBuf> {
    let src_file = test_dir.join(format!("{}_rs.rs", name));
    let exe_file = test_dir.join(format!("{}_rs.exe", name));
    fs::write(&src_file, src).expect("Failed to write Rust source");

    let output = Command::new("rustc")
        .arg("-O")
        .arg(&src_file)
        .arg("-o")
        .arg(&exe_file)
        .output()
        .ok()?;

    if output.status.success() {
        Some(exe_file)
    } else {
        eprintln!(
            "rustc compilation failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        None
    }
}

fn compile_c(src: &str, test_dir: &Path, name: &str) -> Option<PathBuf> {
    let vcvars = find_vcvars64()?;
    let src_file = test_dir.join(format!("{}_c.c", name));
    let exe_file = test_dir.join(format!("{}_c.exe", name));
    fs::write(&src_file, src).expect("Failed to write C source");

    let bat_path = test_dir.join("compile_msvc.bat");
    let bat_content = format!(
        "@echo off\r\ncall \"{}\" >nul 2>&1\r\ncl /O2 /nologo /Fe:\"%~2\" \"%~1\" >nul 2>&1\r\n",
        vcvars.display()
    );
    fs::write(&bat_path, bat_content).ok()?;

    let output = Command::new("cmd.exe")
        .args(&["/c", bat_path.to_str()?, src_file.to_str()?, exe_file.to_str()?])
        .output()
        .ok()?;

    if output.status.success() && exe_file.exists() {
        Some(exe_file)
    } else {
        eprintln!(
            "MSVC C compilation failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        None
    }
}

fn benchmark_exe(exe: &Path, expected_exit: i32, iterations: usize) -> (Duration, Duration, i32, bool) {
    // Warmup
    let _ = Command::new(exe).output();

    let mut times = Vec::with_capacity(iterations);
    let mut last_code = -1;

    for _ in 0..iterations {
        let start = Instant::now();
        let out = Command::new(exe).output().expect("Failed to run benchmark binary");
        let elapsed = start.elapsed();
        times.push(elapsed);
        last_code = out.status.code().unwrap_or(-1);
    }

    let min_time = *times.iter().min().unwrap();
    let total_nanos: u128 = times.iter().map(|d| d.as_nanos()).sum();
    let avg_time = Duration::from_nanos((total_nanos / iterations as u128) as u64);
    let passed = last_code == expected_exit;

    (min_time, avg_time, last_code, passed)
}

#[test]
fn test_comparative_benchmarks() {
    let test_dir = std::env::temp_dir().join("numlang_comparative_benchmarks");
    fs::create_dir_all(&test_dir).unwrap();

    let benchmarks = vec![
        (
            "Recursive Fibonacci (fib 32)",
            5,
            r#"
fn fib(n: i64) -> i64 {
    if n <= 1 {
        return n;
    } else {
        return fib(n - 1) + fib(n - 2);
    }
}

fn main() -> i64 {
    let res: i64 = fib(32);
    return res % 256;
}
            "#,
            r#"
fn fib(n: i64) -> i64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
fn main() {
    let res = fib(32);
    std::process::exit((res % 256) as i32);
}
            "#,
            r#"
long long fib(long long n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
int main() {
    long long res = fib(32);
    return (int)(res % 256);
}
            "#,
        ),
        (
            "Math Loop Accumulator (5M iters)",
            27,
            r#"
fn math_accumulator(iters: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let diff: i64 = i * 3 - 7;
        let t: i64 = abs(diff);
        acc = (acc + t) % 1000000007;
        i = i + 1;
    }
    return acc % 256;
}

fn main() -> i64 {
    return math_accumulator(5000000);
}
            "#,
            r#"
fn math_accumulator(iters: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let diff: i64 = i * 3 - 7;
        let t: i64 = diff.abs();
        acc = (acc + t) % 1000000007;
        i += 1;
    }
    acc % 256
}
fn main() {
    std::process::exit(math_accumulator(5000000) as i32);
}
            "#,
            r#"
#include <stdlib.h>
long long math_accumulator(long long iters) {
    long long acc = 0;
    long long i = 0;
    while (i < iters) {
        long long diff = i * 3 - 7;
        long long t = llabs(diff);
        acc = (acc + t) % 1000000007;
        i++;
    }
    return acc % 256;
}
int main() {
    return (int)math_accumulator(5000000);
}
            "#,
        ),
        (
            "Contiguous Vector Accumulation (5M iters)",
            89,
            r#"
fn vector_bench(iters: i64) -> i64 {
    let mut arr: [i64; 8] = [10, 20, 30, 40, 50, 60, 70, 80];
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let mut j: i64 = 0;
        while j < 8 {
            acc = (acc + arr[j]) % 1000000007;
            j = j + 1;
        }
        arr[0] = (arr[0] + 1) % 100;
        i = i + 1;
    }
    return acc % 256;
}

fn main() -> i64 {
    return vector_bench(5000000);
}
            "#,
            r#"
fn vector_bench(iters: i64) -> i64 {
    let mut arr: [i64; 8] = [10, 20, 30, 40, 50, 60, 70, 80];
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let mut j: usize = 0;
        while j < 8 {
            acc = (acc + arr[j]) % 1000000007;
            j += 1;
        }
        arr[0] = (arr[0] + 1) % 100;
        i += 1;
    }
    acc % 256
}
fn main() {
    std::process::exit(vector_bench(5000000) as i32);
}
            "#,
            r#"
long long vector_bench(long long iters) {
    long long arr[8] = {10, 20, 30, 40, 50, 60, 70, 80};
    long long acc = 0;
    long long i = 0;
    while (i < iters) {
        long long j = 0;
        while (j < 8) {
            acc = (acc + arr[j]) % 1000000007;
            j++;
        }
        arr[0] = (arr[0] + 1) % 100;
        i++;
    }
    return acc % 256;
}
int main() {
    return (int)vector_bench(5000000);
}
            "#,
        ),
    ];

    println!("\n==========================================================================================");
    println!("                           NUMLANG COMPARATIVE BENCHMARK SUITE                            ");
    println!("==========================================================================================");
    println!("{:<42} | {:<8} | {:<12} | {:<12} | {:<6}", "Benchmark", "Language", "Min Time", "Avg Time", "Status");
    println!("------------------------------------------------------------------------------------------");

    for (name, expected_code, nl_code, rs_code, c_code) in benchmarks {
        let slug = name.to_lowercase().replace(' ', "_").replace('(', "").replace(')', "");

        // 1. Compile & Benchmark numlang
        let nl_exe = compile_numlang(nl_code, &test_dir, &slug);
        let (nl_min, nl_avg, nl_code_out, nl_pass) = benchmark_exe(&nl_exe, expected_code, 3);
        println!(
            "{:<42} | {:<8} | {:>10.2?} | {:>10.2?} | {:<6}",
            name, "numlang", nl_min, nl_avg, if nl_pass { "PASS" } else { "FAIL" }
        );
        assert!(nl_pass, "numlang benchmark failed: expected exit {}, got {}", expected_code, nl_code_out);

        // 2. Compile & Benchmark Rust
        if let Some(rs_exe) = compile_rust(rs_code, &test_dir, &slug) {
            let (rs_min, rs_avg, rs_code_out, rs_pass) = benchmark_exe(&rs_exe, expected_code, 3);
            println!(
                "{:<42} | {:<8} | {:>10.2?} | {:>10.2?} | {:<6}",
                "", "Rust -O", rs_min, rs_avg, if rs_pass { "PASS" } else { "FAIL" }
            );
            assert!(rs_pass, "Rust benchmark failed: expected exit {}, got {}", expected_code, rs_code_out);
        }

        // 3. Compile & Benchmark C
        if let Some(c_exe) = compile_c(c_code, &test_dir, &slug) {
            let (c_min, c_avg, c_code_out, c_pass) = benchmark_exe(&c_exe, expected_code, 3);
            println!(
                "{:<42} | {:<8} | {:>10.2?} | {:>10.2?} | {:<6}",
                "", "C (/O2)", c_min, c_avg, if c_pass { "PASS" } else { "FAIL" }
            );
            assert!(c_pass, "C benchmark failed: expected exit {}, got {}", expected_code, c_code_out);
        }

        println!("------------------------------------------------------------------------------------------");
    }

    println!("==========================================================================================\n");
}
