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

fn expected_fib(n: i64) -> i32 {
    let mut a = 0i64;
    let mut b = 1i64;
    for _ in 0..n {
        let t = a + b;
        a = b;
        b = t;
    }
    (a % 256) as i32
}

fn expected_math_acc(iters: i64) -> i32 {
    let mut acc = 0i64;
    let mut i = 0i64;
    while i < iters {
        let diff = i * 3 - 7;
        let t = diff.abs();
        acc = (acc + t) % 1000000007;
        i += 1;
    }
    (acc % 256) as i32
}

fn expected_dot(iters: i64) -> i32 {
    let mut a: [i64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    let b: [i64; 8] = [2, 3, 4, 5, 6, 7, 8, 9];
    let mut acc = 0i64;
    let mut i = 0i64;
    while i < iters {
        let mut d = 0i64;
        let mut j = 0usize;
        while j < 8 {
            d += a[j] * b[j];
            j += 1;
        }
        acc = (acc + d) % 1000000007;
        a[0] = (a[0] + 1) % 100;
        i += 1;
    }
    (acc % 256) as i32
}

fn expected_matvec(iters: i64) -> i32 {
    let r0: [i64; 4] = [1, 2, 3, 4];
    let r1: [i64; 4] = [5, 6, 7, 8];
    let r2: [i64; 4] = [9, 10, 11, 12];
    let r3: [i64; 4] = [13, 14, 15, 16];
    let mut v: [i64; 4] = [2, 3, 4, 5];

    let mut acc = 0i64;
    let mut i = 0i64;
    while i < iters {
        let y0 = r0[0]*v[0] + r0[1]*v[1] + r0[2]*v[2] + r0[3]*v[3];
        let y1 = r1[0]*v[0] + r1[1]*v[1] + r1[2]*v[2] + r1[3]*v[3];
        let y2 = r2[0]*v[0] + r2[1]*v[1] + r2[2]*v[2] + r2[3]*v[3];
        let y3 = r3[0]*v[0] + r3[1]*v[1] + r3[2]*v[2] + r3[3]*v[3];

        acc = (acc + y0 + y1 + y2 + y3) % 1000000007;
        v[0] = (v[0] + 1) % 50;
        i += 1;
    }
    (acc % 256) as i32
}

#[test]
fn test_comparative_benchmarks() {
    let test_dir = std::env::temp_dir().join("numlang_comparative_benchmarks");
    fs::create_dir_all(&test_dir).unwrap();

    let benchmarks = vec![
        (
            "Recursive Fibonacci (fib 35)",
            expected_fib(35),
            r#"
fn fib(n: i64) -> i64 {
    if n <= 1 {
        return n;
    } else {
        return fib(n - 1) + fib(n - 2);
    }
}

fn main() -> i64 {
    let res: i64 = fib(35);
    return res % 256;
}
            "#,
            r#"
fn fib(n: i64) -> i64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
fn main() {
    let res = fib(35);
    std::process::exit((res % 256) as i32);
}
            "#,
            r#"
long long fib(long long n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
int main() {
    long long res = fib(35);
    return (int)(res % 256);
}
            "#,
        ),
        (
            "Math Loop Accumulator (10M iters)",
            expected_math_acc(10000000),
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
    return math_accumulator(10000000);
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
    std::process::exit(math_accumulator(10000000) as i32);
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
    return (int)math_accumulator(10000000);
}
            "#,
        ),
        (
            "Hardware SIMD Vector Dot (10M iters)",
            expected_dot(10000000),
            r#"
fn dot_bench(iters: i64) -> i64 {
    let mut a: [i64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    let b: [i64; 8] = [2, 3, 4, 5, 6, 7, 8, 9];
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let d: i64 = dot(a, b);
        acc = (acc + d) % 1000000007;
        a[0] = (a[0] + 1) % 100;
        i = i + 1;
    }
    return acc % 256;
}

fn main() -> i64 {
    return dot_bench(10000000);
}
            "#,
            r#"
fn dot_bench(iters: i64) -> i64 {
    let mut a: [i64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    let b: [i64; 8] = [2, 3, 4, 5, 6, 7, 8, 9];
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let mut d: i64 = 0;
        let mut j: usize = 0;
        while j < 8 {
            d += a[j] * b[j];
            j += 1;
        }
        acc = (acc + d) % 1000000007;
        a[0] = (a[0] + 1) % 100;
        i += 1;
    }
    acc % 256
}
fn main() {
    std::process::exit(dot_bench(10000000) as i32);
}
            "#,
            r#"
long long dot_bench(long long iters) {
    long long a[8] = {1, 2, 3, 4, 5, 6, 7, 8};
    long long b[8] = {2, 3, 4, 5, 6, 7, 8, 9};
    long long acc = 0;
    long long i = 0;
    while (i < iters) {
        long long d = 0;
        long long j = 0;
        while (j < 8) {
            d += a[j] * b[j];
            j++;
        }
        acc = (acc + d) % 1000000007;
        a[0] = (a[0] + 1) % 100;
        i++;
    }
    return acc % 256;
}
int main() {
    return (int)dot_bench(10000000);
}
            "#,
        ),
        (
            "Matrix-Vector Multiplication (1M iters)",
            expected_matvec(1000000),
            r#"
fn matvec_bench(iters: i64) -> i64 {
    let r0: [i64; 4] = [1, 2, 3, 4];
    let r1: [i64; 4] = [5, 6, 7, 8];
    let r2: [i64; 4] = [9, 10, 11, 12];
    let r3: [i64; 4] = [13, 14, 15, 16];
    let mut v: [i64; 4] = [2, 3, 4, 5];

    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let y0: i64 = dot(r0, v);
        let y1: i64 = dot(r1, v);
        let y2: i64 = dot(r2, v);
        let y3: i64 = dot(r3, v);

        acc = (acc + y0 + y1 + y2 + y3) % 1000000007;
        v[0] = (v[0] + 1) % 50;
        i = i + 1;
    }
    return acc % 256;
}

fn main() -> i64 {
    return matvec_bench(1000000);
}
            "#,
            r#"
fn matvec_bench(iters: i64) -> i64 {
    let r0: [i64; 4] = [1, 2, 3, 4];
    let r1: [i64; 4] = [5, 6, 7, 8];
    let r2: [i64; 4] = [9, 10, 11, 12];
    let r3: [i64; 4] = [13, 14, 15, 16];
    let mut v: [i64; 4] = [2, 3, 4, 5];

    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let y0 = r0[0]*v[0] + r0[1]*v[1] + r0[2]*v[2] + r0[3]*v[3];
        let y1 = r1[0]*v[0] + r1[1]*v[1] + r1[2]*v[2] + r1[3]*v[3];
        let y2 = r2[0]*v[0] + r2[1]*v[1] + r2[2]*v[2] + r2[3]*v[3];
        let y3 = r3[0]*v[0] + r3[1]*v[1] + r3[2]*v[2] + r3[3]*v[3];

        acc = (acc + y0 + y1 + y2 + y3) % 1000000007;
        v[0] = (v[0] + 1) % 50;
        i += 1;
    }
    acc % 256
}
fn main() {
    std::process::exit(matvec_bench(1000000) as i32);
}
            "#,
            r#"
long long matvec_bench(long long iters) {
    long long r0[4] = {1, 2, 3, 4};
    long long r1[4] = {5, 6, 7, 8};
    long long r2[4] = {9, 10, 11, 12};
    long long r3[4] = {13, 14, 15, 16};
    long long v[4] = {2, 3, 4, 5};

    long long acc = 0;
    long long i = 0;
    while (i < iters) {
        long long y0 = r0[0]*v[0] + r0[1]*v[1] + r0[2]*v[2] + r0[3]*v[3];
        long long y1 = r1[0]*v[0] + r1[1]*v[1] + r1[2]*v[2] + r1[3]*v[3];
        long long y2 = r2[0]*v[0] + r2[1]*v[1] + r2[2]*v[2] + r2[3]*v[3];
        long long y3 = r3[0]*v[0] + r3[1]*v[1] + r3[2]*v[2] + r3[3]*v[3];

        acc = (acc + y0 + y1 + y2 + y3) % 1000000007;
        v[0] = (v[0] + 1) % 50;
        i++;
    }
    return acc % 256;
}
int main() {
    return (int)matvec_bench(1000000);
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
        assert!(nl_pass, "numlang benchmark failed on '{}': expected exit {}, got {}", name, expected_code, nl_code_out);

        // 2. Compile & Benchmark Rust
        if let Some(rs_exe) = compile_rust(rs_code, &test_dir, &slug) {
            let (rs_min, rs_avg, rs_code_out, rs_pass) = benchmark_exe(&rs_exe, expected_code, 3);
            println!(
                "{:<42} | {:<8} | {:>10.2?} | {:>10.2?} | {:<6}",
                "", "Rust -O", rs_min, rs_avg, if rs_pass { "PASS" } else { "FAIL" }
            );
            assert!(rs_pass, "Rust benchmark failed on '{}': expected exit {}, got {}", name, expected_code, rs_code_out);
        }

        // 3. Compile & Benchmark C
        if let Some(c_exe) = compile_c(c_code, &test_dir, &slug) {
            let (c_min, c_avg, c_code_out, c_pass) = benchmark_exe(&c_exe, expected_code, 3);
            println!(
                "{:<42} | {:<8} | {:>10.2?} | {:>10.2?} | {:<6}",
                "", "C (/O2)", c_min, c_avg, if c_pass { "PASS" } else { "FAIL" }
            );
            assert!(c_pass, "C benchmark failed on '{}': expected exit {}, got {}", name, expected_code, c_code_out);
        }

        println!("------------------------------------------------------------------------------------------");
    }

    println!("==========================================================================================\n");
}
