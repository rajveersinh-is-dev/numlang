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
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
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

fn wrap_node(src: &str) -> String {
    format!(
        r#"
const _bench_t0 = process.hrtime.bigint();
const _orig_exit = process.exit;
process.exit = function(code) {{
    const _bench_ns = process.hrtime.bigint() - _bench_t0;
    process.stdout.write("COMPUTE_NS: " + _bench_ns.toString() + "\n");
    _orig_exit.call(process, code);
}};
{}
"#,
        src
    )
}

fn wrap_py(src: &str) -> String {
    format!(
        r#"
import time, sys
_bench_t0 = time.perf_counter_ns()
_orig_exit = sys.exit
def _bench_exit(code=0):
    _bench_ns = time.perf_counter_ns() - _bench_t0
    sys.stdout.write(f"COMPUTE_NS: {{_bench_ns}}\n")
    sys.stdout.flush()
    _orig_exit(code)
sys.exit = _bench_exit
{}
"#,
        src
    )
}

fn compile_numlang(src: &str, test_dir: &Path, name: &str) -> PathBuf {
    let src_file = test_dir.join(format!("{}_nl.nl", name));
    let exe_file = test_dir.join(format!("{}_nl.exe", name));
    fs::write(&src_file, src).expect("Failed to write numlang source");

    let numlang_bin = env!("CARGO_BIN_EXE_numlang");
    let output = Command::new(numlang_bin)
        .arg("build")
        .arg(&src_file)
        .arg("--bench")
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
    fs::write(&src_file, wrap_rust(src)).expect("Failed to write Rust source");

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
        None
    }
}

fn compile_c(src: &str, test_dir: &Path, name: &str) -> Option<PathBuf> {
    let vcvars = find_vcvars64()?;
    let src_file = test_dir.join(format!("{}_c.c", name));
    let exe_file = test_dir.join(format!("{}_c.exe", name));
    fs::write(&src_file, wrap_c(src)).expect("Failed to write C source");

    let bat_path = test_dir.join(format!("compile_msvc_{}.bat", name));
    let bat_content = format!(
        "@echo off\r\ncall \"{}\" >nul 2>&1\r\ncl /O2 /nologo /Fe:\"%~2\" \"%~1\" >nul 2>&1\r\n",
        vcvars.display()
    );
    fs::write(&bat_path, bat_content).ok()?;

    let output = Command::new("cmd.exe")
        .args(["/c", bat_path.to_str()?, src_file.to_str()?, exe_file.to_str()?])
        .output()
        .ok()?;

    let _ = fs::remove_file(&bat_path);

    if output.status.success() && exe_file.exists() {
        Some(exe_file)
    } else {
        None
    }
}

fn benchmark_cmd(
    cmd: &str,
    args: &[&str],
    expected_exit: i32,
    iterations: usize,
) -> (Duration, Duration, Duration, Duration, i32, bool) {
    // Warmup
    let _ = Command::new(cmd).args(args).output();

    let mut wall_times = Vec::with_capacity(iterations);
    let mut compute_times = Vec::with_capacity(iterations);
    let mut last_code = -1;

    for _ in 0..iterations {
        let start = Instant::now();
        let output = Command::new(cmd)
            .args(args)
            .output()
            .expect("Failed to run benchmark binary");
        let elapsed = start.elapsed();

        wall_times.push(elapsed);
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

    let min_compute = *compute_times.iter().min().unwrap();
    let total_compute_nanos: u128 = compute_times.iter().map(|d| d.as_nanos()).sum();
    let avg_compute = Duration::from_nanos((total_compute_nanos / iterations as u128) as u64);

    let min_wall = *wall_times.iter().min().unwrap();
    let total_wall_nanos: u128 = wall_times.iter().map(|d| d.as_nanos()).sum();
    let avg_wall = Duration::from_nanos((total_wall_nanos / iterations as u128) as u64);

    let passed = last_code == expected_exit;

    (min_compute, avg_compute, min_wall, avg_wall, last_code, passed)
}

struct BenchmarkWorkload {
    name: &'static str,
    expected_exit: i32,
    nl_code: &'static str,
    rs_code: &'static str,
    c_code: &'static str,
    node_code: &'static str,
    py_code: &'static str,
}

#[test]
#[ignore = "comprehensive multi-language benchmark suite takes several minutes; run with -- --ignored"]
fn test_comprehensive_multi_language_benchmarks() {
    let test_dir = std::env::temp_dir().join("numlang_multi_lang_benchmarks");
    fs::create_dir_all(&test_dir).unwrap();

    let workloads: Vec<BenchmarkWorkload> = vec![
        // 1. Recursive Fibonacci
        BenchmarkWorkload {
            name: "Recursive Fibonacci (fib 35)",
            expected_exit: 201,
            nl_code: r#"
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
            rs_code: r#"
fn fib(n: i64) -> i64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
fn main() {
    let res = fib(35);
    std::process::exit((res % 256) as i32);
}
"#,
            c_code: r#"
long long fib(long long n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
int main() {
    long long res = fib(35);
    return (int)(res % 256);
}
"#,
            node_code: r#"
function fib(n) {
    if (n <= 1n) return n;
    return fib(n - 1n) + fib(n - 2n);
}
const res = fib(35n);
process.exit(Number(res % 256n));
"#,
            py_code: r#"
import sys
def fib(n):
    if n <= 1: return n
    return fib(n - 1) + fib(n - 2)
res = fib(35)
sys.exit(res % 256)
"#,
        },

        // 2. Math Loop Accumulator
        BenchmarkWorkload {
            name: "Math Loop Accumulator (10M iters)",
            expected_exit: 79,
            nl_code: r#"
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
            rs_code: r#"
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
            c_code: r#"
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
            node_code: r#"
function math_accumulator(iters) {
    let acc = 0n;
    let i = 0n;
    while (i < iters) {
        let diff = i * 3n - 7n;
        let t = diff < 0n ? -diff : diff;
        acc = (acc + t) % 1000000007n;
        i += 1n;
    }
    return acc % 256n;
}
process.exit(Number(math_accumulator(10000000n)));
"#,
            py_code: r#"
import sys
acc = 0
for i in range(10000000):
    diff = i * 3 - 7
    acc = (acc + abs(diff)) % 1000000007
sys.exit(acc % 256)
"#,
        },

        // 3. Hardware SIMD Vector Dot
        BenchmarkWorkload {
            name: "Hardware SIMD Vector Dot (10M iters)",
            expected_exit: 107,
            nl_code: r#"
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
            rs_code: r#"
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
            c_code: r#"
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
            node_code: r#"
function dot_bench(iters) {
    let a = [1n, 2n, 3n, 4n, 5n, 6n, 7n, 8n];
    let b = [2n, 3n, 4n, 5n, 6n, 7n, 8n, 9n];
    let acc = 0n;
    for (let i = 0; i < iters; i++) {
        let d = a[0]*b[0] + a[1]*b[1] + a[2]*b[2] + a[3]*b[3] + a[4]*b[4] + a[5]*b[5] + a[6]*b[6] + a[7]*b[7];
        acc = (acc + d) % 1000000007n;
        a[0] = (a[0] + 1n) % 100n;
    }
    return acc % 256n;
}
process.exit(Number(dot_bench(10000000)));
"#,
            py_code: r#"
import sys
a = [1, 2, 3, 4, 5, 6, 7, 8]
b = [2, 3, 4, 5, 6, 7, 8, 9]
acc = 0
for _ in range(10000000):
    d = a[0]*b[0] + a[1]*b[1] + a[2]*b[2] + a[3]*b[3] + a[4]*b[4] + a[5]*b[5] + a[6]*b[6] + a[7]*b[7]
    acc = (acc + d) % 1000000007
    a[0] = (a[0] + 1) % 100
sys.exit(acc % 256)
"#,
        },

        // 4. Dense Matrix-Vector Multiplication
        BenchmarkWorkload {
            name: "Matrix-Vector Multiplication (5M iters)",
            expected_exit: 93,
            nl_code: r#"
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
    return matvec_bench(5000000);
}
"#,
            rs_code: r#"
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
    std::process::exit(matvec_bench(5000000) as i32);
}
"#,
            c_code: r#"
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
    return (int)matvec_bench(5000000);
}
"#,
            node_code: r#"
function matvec_bench(iters) {
    let r0 = [1n, 2n, 3n, 4n];
    let r1 = [5n, 6n, 7n, 8n];
    let r2 = [9n, 10n, 11n, 12n];
    let r3 = [13n, 14n, 15n, 16n];
    let v = [2n, 3n, 4n, 5n];

    let acc = 0n;
    for (let i = 0; i < iters; i++) {
        let y0 = r0[0]*v[0] + r0[1]*v[1] + r0[2]*v[2] + r0[3]*v[3];
        let y1 = r1[0]*v[0] + r1[1]*v[1] + r1[2]*v[2] + r1[3]*v[3];
        let y2 = r2[0]*v[0] + r2[1]*v[1] + r2[2]*v[2] + r2[3]*v[3];
        let y3 = r3[0]*v[0] + r3[1]*v[1] + r3[2]*v[2] + r3[3]*v[3];

        acc = (acc + y0 + y1 + y2 + y3) % 1000000007n;
        v[0] = (v[0] + 1n) % 50n;
    }
    return acc % 256n;
}
process.exit(Number(matvec_bench(5000000)));
"#,
            py_code: r#"
import sys
r0 = [1, 2, 3, 4]
r1 = [5, 6, 7, 8]
r2 = [9, 10, 11, 12]
r3 = [13, 14, 15, 16]
v = [2, 3, 4, 5]
acc = 0
for _ in range(5000000):
    y0 = r0[0]*v[0] + r0[1]*v[1] + r0[2]*v[2] + r0[3]*v[3]
    y1 = r1[0]*v[0] + r1[1]*v[1] + r1[2]*v[2] + r1[3]*v[3]
    y2 = r2[0]*v[0] + r2[1]*v[1] + r2[2]*v[2] + r2[3]*v[3]
    y3 = r3[0]*v[0] + r3[1]*v[1] + r3[2]*v[2] + r3[3]*v[3]
    acc = (acc + y0 + y1 + y2 + y3) % 1000000007
    v[0] = (v[0] + 1) % 50
sys.exit(acc % 256)
"#,
        },

        // 5. Collatz Hailstone Trajectory
        BenchmarkWorkload {
            name: "Collatz Hailstone (100k seeds)",
            expected_exit: 48,
            nl_code: r#"
fn collatz_steps(limit: i64) -> i64 {
    let mut total_steps: i64 = 0;
    let mut n: i64 = 1;
    while n <= limit {
        let mut curr: i64 = n;
        while curr > 1 {
            if curr % 2 == 0 {
                curr = curr / 2;
            } else {
                curr = curr * 3 + 1;
            }
            total_steps = total_steps + 1;
        }
        n = n + 1;
    }
    return total_steps % 256;
}
fn main() -> i64 {
    return collatz_steps(100000);
}
"#,
            rs_code: r#"
fn collatz_steps(limit: i64) -> i64 {
    let mut total_steps: i64 = 0;
    let mut n: i64 = 1;
    while n <= limit {
        let mut curr: i64 = n;
        while curr > 1 {
            if curr % 2 == 0 {
                curr /= 2;
            } else {
                curr = curr * 3 + 1;
            }
            total_steps += 1;
        }
        n += 1;
    }
    total_steps % 256
}
fn main() {
    std::process::exit(collatz_steps(100000) as i32);
}
"#,
            c_code: r#"
long long collatz_steps(long long limit) {
    long long total_steps = 0;
    long long n = 1;
    while (n <= limit) {
        long long curr = n;
        while (curr > 1) {
            if (curr % 2 == 0) {
                curr /= 2;
            } else {
                curr = curr * 3 + 1;
            }
            total_steps++;
        }
        n++;
    }
    return total_steps % 256;
}
int main() {
    return (int)collatz_steps(100000);
}
"#,
            node_code: r#"
function collatz_steps(limit) {
    let total_steps = 0n;
    for (let n = 1n; n <= limit; n++) {
        let curr = n;
        while (curr > 1n) {
            if (curr % 2n === 0n) {
                curr /= 2n;
            } else {
                curr = curr * 3n + 1n;
            }
            total_steps += 1n;
        }
    }
    return total_steps % 256n;
}
process.exit(Number(collatz_steps(100000n)));
"#,
            py_code: r#"
import sys
total = 0
for n in range(1, 100001):
    curr = n
    while curr > 1:
        if curr % 2 == 0:
            curr //= 2
        else:
            curr = curr * 3 + 1
        total += 1
sys.exit(total % 256)
"#,
        },

        // 6. Prime Counting by Trial Division
        BenchmarkWorkload {
            name: "Prime Counting (400k limit)",
            expected_exit: 68,
            nl_code: r#"
fn is_prime(n: i64) -> i64 {
    if n <= 1 {
        return 0;
    }
    let mut d: i64 = 2;
    while d * d <= n {
        if n % d == 0 {
            return 0;
        }
        d = d + 1;
    }
    return 1;
}
fn count_primes(limit: i64) -> i64 {
    let mut count: i64 = 0;
    let mut n: i64 = 2;
    while n <= limit {
        if is_prime(n) == 1 {
            count = count + 1;
        }
        n = n + 1;
    }
    return count % 256;
}
fn main() -> i64 {
    return count_primes(400000);
}
"#,
            rs_code: r#"
fn is_prime(n: i64) -> bool {
    if n <= 1 { return false; }
    let mut d = 2;
    while d * d <= n {
        if n % d == 0 { return false; }
        d += 1;
    }
    true
}
fn count_primes(limit: i64) -> i64 {
    let mut count = 0;
    let mut n = 2;
    while n <= limit {
        if is_prime(n) { count += 1; }
        n += 1;
    }
    count % 256
}
fn main() {
    std::process::exit(count_primes(400000) as i32);
}
"#,
            c_code: r#"
int is_prime(long long n) {
    if (n <= 1) return 0;
    long long d = 2;
    while (d * d <= n) {
        if (n % d == 0) return 0;
        d++;
    }
    return 1;
}
long long count_primes(long long limit) {
    long long count = 0;
    long long n = 2;
    while (n <= limit) {
        if (is_prime(n)) count++;
        n++;
    }
    return count % 256;
}
int main() {
    return (int)count_primes(400000);
}
"#,
            node_code: r#"
function is_prime(n) {
    if (n <= 1) return false;
    for (let d = 2; d * d <= n; d++) {
        if (n % d === 0) return false;
    }
    return true;
}
function count_primes(limit) {
    let count = 0;
    for (let n = 2; n <= limit; n++) {
        if (is_prime(n)) count++;
    }
    return count % 256;
}
process.exit(count_primes(400000));
"#,
            py_code: r#"
import sys
def is_prime(n):
    if n <= 1: return False
    d = 2
    while d * d <= n:
        if n % d == 0: return False
        d += 1
    return True
count = 0
for n in range(2, 400001):
    if is_prime(n): count += 1
sys.exit(count % 256)
"#,
        },

        // 7. Horner's Polynomial Evaluation
        BenchmarkWorkload {
            name: "Horner Polynomial Eval (10M iters)",
            expected_exit: 211,
            nl_code: r#"
fn horner_bench(iters: i64) -> i64 {
    let coeffs: [i64; 8] = [3, -5, 2, 7, -4, 6, -1, 8];
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let x: i64 = (i % 7) + 1;
        let mut p: i64 = coeffs[7];
        p = p * x + coeffs[6];
        p = p * x + coeffs[5];
        p = p * x + coeffs[4];
        p = p * x + coeffs[3];
        p = p * x + coeffs[2];
        p = p * x + coeffs[1];
        p = p * x + coeffs[0];

        acc = (acc + p) % 1000000007;
        i = i + 1;
    }
    return acc % 256;
}
fn main() -> i64 {
    return horner_bench(10000000);
}
"#,
            rs_code: r#"
fn horner_bench(iters: i64) -> i64 {
    let coeffs: [i64; 8] = [3, -5, 2, 7, -4, 6, -1, 8];
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let x: i64 = (i % 7) + 1;
        let mut p: i64 = coeffs[7];
        p = p * x + coeffs[6];
        p = p * x + coeffs[5];
        p = p * x + coeffs[4];
        p = p * x + coeffs[3];
        p = p * x + coeffs[2];
        p = p * x + coeffs[1];
        p = p * x + coeffs[0];

        acc = (acc + p) % 1000000007;
        i += 1;
    }
    acc % 256
}
fn main() {
    std::process::exit(horner_bench(10000000) as i32);
}
"#,
            c_code: r#"
long long horner_bench(long long iters) {
    long long coeffs[8] = {3, -5, 2, 7, -4, 6, -1, 8};
    long long acc = 0;
    long long i = 0;
    while (i < iters) {
        long long x = (i % 7) + 1;
        long long p = coeffs[7];
        p = p * x + coeffs[6];
        p = p * x + coeffs[5];
        p = p * x + coeffs[4];
        p = p * x + coeffs[3];
        p = p * x + coeffs[2];
        p = p * x + coeffs[1];
        p = p * x + coeffs[0];

        acc = (acc + p) % 1000000007;
        i++;
    }
    return acc % 256;
}
int main() {
    return (int)horner_bench(10000000);
}
"#,
            node_code: r#"
function horner_bench(iters) {
    const coeffs = [3n, -5n, 2n, 7n, -4n, 6n, -1n, 8n];
    let acc = 0n;
    for (let i = 0n; i < iters; i++) {
        let x = (i % 7n) + 1n;
        let p = coeffs[7];
        p = p * x + coeffs[6];
        p = p * x + coeffs[5];
        p = p * x + coeffs[4];
        p = p * x + coeffs[3];
        p = p * x + coeffs[2];
        p = p * x + coeffs[1];
        p = p * x + coeffs[0];

        acc = (acc + p) % 1000000007n;
    }
    return acc % 256n;
}
process.exit(Number(horner_bench(10000000n)));
"#,
            py_code: r#"
import sys
coeffs = [3, -5, 2, 7, -4, 6, -1, 8]
acc = 0
for i in range(10000000):
    x = (i % 7) + 1
    p = coeffs[7]
    p = p * x + coeffs[6]
    p = p * x + coeffs[5]
    p = p * x + coeffs[4]
    p = p * x + coeffs[3]
    p = p * x + coeffs[2]
    p = p * x + coeffs[1]
    p = p * x + coeffs[0]
    acc = (acc + p) % 1000000007
sys.exit(acc % 256)
"#,
        },

        // 8. Takeuchi Ternary Recursion
        BenchmarkWorkload {
            name: "Takeuchi Recursion (tak 27, 18, 9)",
            expected_exit: 18,
            nl_code: r#"
fn tak(x: i64, y: i64, z: i64) -> i64 {
    if y < x {
        return tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y));
    } else {
        return z;
    }
}
fn main() -> i64 {
    return tak(27, 18, 9);
}
"#,
            rs_code: r#"
fn tak(x: i64, y: i64, z: i64) -> i64 {
    if y < x {
        tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y))
    } else {
        z
    }
}
fn main() {
    std::process::exit(tak(27, 18, 9) as i32);
}
"#,
            c_code: r#"
long long tak(long long x, long long y, long long z) {
    if (y < x) {
        return tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y));
    } else {
        return z;
    }
}
int main() {
    return (int)tak(27, 18, 9);
}
"#,
            node_code: r#"
function tak(x, y, z) {
    if (y < x) {
        return tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y));
    } else {
        return z;
    }
}
process.exit(tak(27, 18, 9));
"#,
            py_code: r#"
import sys
def tak(x, y, z):
    if y < x:
        return tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y))
    else:
        return z
sys.exit(tak(27, 18, 9))
"#,
        },

        // 9. Numerical Quadrature Pi Riemann Sum
        BenchmarkWorkload {
            name: "Numerical Quadrature Pi (50M iters)",
            expected_exit: 129,
            nl_code: r#"
fn pi_riemann(iters: i64) -> i64 {
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let x_scaled: i64 = (i * 1000) / iters;
        let denom: i64 = 1000000 + x_scaled * x_scaled;
        let term: i64 = 4000000000000 / denom;
        sum = (sum + term) % 1000000007;
        i = i + 1;
    }
    return sum % 256;
}
fn main() -> i64 {
    return pi_riemann(50000000);
}
"#,
            rs_code: r#"
fn pi_riemann(iters: i64) -> i64 {
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let x_scaled = (i * 1000) / iters;
        let denom = 1000000 + x_scaled * x_scaled;
        let term = 4000000000000 / denom;
        sum = (sum + term) % 1000000007;
        i += 1;
    }
    sum % 256
}
fn main() {
    std::process::exit(pi_riemann(50000000) as i32);
}
"#,
            c_code: r#"
long long pi_riemann(long long iters) {
    long long sum = 0;
    long long i = 0;
    while (i < iters) {
        long long x_scaled = (i * 1000) / iters;
        long long denom = 1000000 + x_scaled * x_scaled;
        long long term = 4000000000000LL / denom;
        sum = (sum + term) % 1000000007;
        i++;
    }
    return sum % 256;
}
int main() {
    return (int)pi_riemann(50000000);
}
"#,
            node_code: r#"
function pi_riemann(iters) {
    let sum = 0;
    for (let i = 0; i < iters; i++) {
        let x_scaled = Math.floor((i * 1000) / iters);
        let denom = 1000000 + x_scaled * x_scaled;
        let term = Math.floor(4000000000000 / denom);
        sum = (sum + term) % 1000000007;
    }
    return sum % 256;
}
process.exit(pi_riemann(50000000));
"#,
            py_code: r#"
import sys
iters = 50000000
total = 0
for i in range(iters):
    x = (i * 1000) // iters
    denom = 1000000 + x * x
    term = 4000000000000 // denom
    total = (total + term) % 1000000007
sys.exit(total % 256)
"#,
        },

        // 10. Ackermann Hyper-Recurrence
        BenchmarkWorkload {
            name: "Ackermann Recurrence (ack 3, 8)",
            expected_exit: 253,
            nl_code: r#"
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
    let res: i64 = ack(3, 8);
    return res % 256;
}
"#,
            rs_code: r#"
fn ack(m: i64, n: i64) -> i64 {
    if m == 0 {
        n + 1
    } else if n == 0 {
        ack(m - 1, 1)
    } else {
        ack(m - 1, ack(m, n - 1))
    }
}
fn main() {
    let res = ack(3, 8);
    std::process::exit((res % 256) as i32);
}
"#,
            c_code: r#"
long long ack(long long m, long long n) {
    if (m == 0) return n + 1;
    if (n == 0) return ack(m - 1, 1);
    return ack(m - 1, ack(m, n - 1));
}
int main() {
    long long res = ack(3, 8);
    return (int)(res % 256);
}
"#,
            node_code: r#"
function ack(m, n) {
    if (m === 0) return n + 1;
    if (n === 0) return ack(m - 1, 1);
    return ack(m - 1, ack(m, n - 1));
}
const res = ack(3, 8);
process.exit(res % 256);
"#,
            py_code: r#"
import sys
def ack(m, n):
    stack = [m]
    while stack:
        m = stack.pop()
        if m == 0:
            n = n + 1
        elif n == 0:
            stack.append(m - 1)
            n = 1
        else:
            stack.append(m - 1)
            stack.append(m)
            n = n - 1
    return n
res = ack(3, 8)
sys.exit(res % 256)
"#,
        },

        // 11. N-Queens Backtracking Problem
        BenchmarkWorkload {
            name: "N-Queens Backtracking (nqueens 12)",
            expected_exit: 120,
            nl_code: r#"
fn solve_nqueens(n: i64) -> i64 {
    let mut b: [i64; 12] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut row: i64 = 0;
    let mut col: i64 = 0;
    let mut count: i64 = 0;
    while row >= 0 {
        let mut found: bool = false;
        while col < n {
            let mut valid: bool = true;
            let mut i: i64 = 0;
            while i < row {
                let diff: i64 = b[i] - col;
                let c_diff: i64 = abs(diff);
                let r_diff: i64 = row - i;
                if b[i] == col {
                    valid = false;
                    break;
                } else {
                    if c_diff == r_diff {
                        valid = false;
                        break;
                    }
                }
                i = i + 1;
            }
            if valid {
                b[row] = col;
                found = true;
                break;
            } else {
                col = col + 1;
            }
        }
        if found {
            if row == n - 1 {
                count = count + 1;
                col = b[row] + 1;
            } else {
                row = row + 1;
                b[row] = 0;
                col = 0;
            }
        } else {
            row = row - 1;
            if row >= 0 {
                col = b[row] + 1;
            }
        }
    }
    return count;
}
fn main() -> i64 {
    let ans: i64 = solve_nqueens(12);
    return ans % 256;
}
"#,
            rs_code: r#"
fn solve_nqueens(n: i64) -> i64 {
    let mut b = [0i64; 12];
    let mut row = 0i64;
    let mut col = 0i64;
    let mut count = 0i64;
    while row >= 0 {
        let mut found = false;
        while col < n {
            let mut valid = true;
            let mut i = 0i64;
            while i < row {
                let diff = b[i as usize] - col;
                let c_diff = diff.abs();
                let r_diff = row - i;
                if b[i as usize] == col || c_diff == r_diff {
                    valid = false;
                    break;
                }
                i += 1;
            }
            if valid {
                b[row as usize] = col;
                found = true;
                break;
            } else {
                col += 1;
            }
        }
        if found {
            if row == n - 1 {
                count += 1;
                col = b[row as usize] + 1;
            } else {
                row += 1;
                b[row as usize] = 0;
                col = 0;
            }
        } else {
            row -= 1;
            if row >= 0 {
                col = b[row as usize] + 1;
            }
        }
    }
    count
}
fn main() {
    let ans = solve_nqueens(12);
    std::process::exit((ans % 256) as i32);
}
"#,
            c_code: r#"
#include <stdlib.h>
long long solve_nqueens(long long n) {
    long long b[12] = {0};
    long long row = 0;
    long long col = 0;
    long long count = 0;
    while (row >= 0) {
        int found = 0;
        while (col < n) {
            int valid = 1;
            long long i = 0;
            while (i < row) {
                long long diff = b[i] - col;
                long long c_diff = llabs(diff);
                long long r_diff = row - i;
                if (b[i] == col || c_diff == r_diff) {
                    valid = 0;
                    break;
                }
                i++;
            }
            if (valid) {
                b[row] = col;
                found = 1;
                break;
            } else {
                col++;
            }
        }
        if (found) {
            if (row == n - 1) {
                count++;
                col = b[row] + 1;
            } else {
                row++;
                b[row] = 0;
                col = 0;
            }
        } else {
            row--;
            if (row >= 0) {
                col = b[row] + 1;
            }
        }
    }
    return count;
}
int main() {
    long long ans = solve_nqueens(12);
    return (int)(ans % 256);
}
"#,
            node_code: r#"
function solve_nqueens(n) {
    const b = new Array(12).fill(0);
    let row = 0;
    let col = 0;
    let count = 0;
    while (row >= 0) {
        let found = false;
        while (col < n) {
            let valid = true;
            let i = 0;
            while (i < row) {
                const diff = b[i] - col;
                const c_diff = Math.abs(diff);
                const r_diff = row - i;
                if (b[i] === col || c_diff === r_diff) {
                    valid = false;
                    break;
                }
                i++;
            }
            if (valid) {
                b[row] = col;
                found = true;
                break;
            } else {
                col++;
            }
        }
        if (found) {
            if (row === n - 1) {
                count++;
                col = b[row] + 1;
            } else {
                row++;
                b[row] = 0;
                col = 0;
            }
        } else {
            row--;
            if (row >= 0) {
                col = b[row] + 1;
            }
        }
    }
    return count;
}
const ans = solve_nqueens(12);
process.exit(ans % 256);
"#,
            py_code: r#"
import sys
def solve_nqueens(n):
    b = [0] * 12
    row = 0
    col = 0
    count = 0
    while row >= 0:
        found = False
        while col < n:
            valid = True
            i = 0
            while i < row:
                diff = b[i] - col
                c_diff = abs(diff)
                r_diff = row - i
                if b[i] == col or c_diff == r_diff:
                    valid = False
                    break
                i += 1
            if valid:
                b[row] = col
                found = True
                break
            else:
                col += 1
        if found:
            if row == n - 1:
                count += 1
                col = b[row] + 1
            else:
                row += 1
                b[row] = 0
                col = 0
        else:
            row -= 1
            if row >= 0:
                col = b[row] + 1
    return count
ans = solve_nqueens(12)
sys.exit(ans % 256)
"#,
        },

        // 12. Mandelbrot Complex Dynamics Grid
        BenchmarkWorkload {
            name: "Mandelbrot Grid (500x500x100)",
            expected_exit: 186,
            nl_code: r#"
fn mandelbrot(w: i64, h: i64, max_iter: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut y: i64 = 0;
    while y < h {
        let ci: i64 = -1500 + (y * 3000) / h;
        let mut x: i64 = 0;
        while x < w {
            let cr: i64 = -2000 + (x * 3000) / w;
            let mut zr: i64 = 0;
            let mut zi: i64 = 0;
            let mut k: i64 = 0;
            let mut active: bool = true;
            while active {
                if k < max_iter {
                    let zr2: i64 = (zr * zr) / 1000;
                    let zi2: i64 = (zi * zi) / 1000;
                    let mag: i64 = zr2 + zi2;
                    if mag > 4000 {
                        active = false;
                    } else {
                        zi = (2 * zr * zi) / 1000 + ci;
                        zr = zr2 - zi2 + cr;
                        k = k + 1;
                    }
                } else {
                    active = false;
                }
            }
            acc = (acc + k) % 1000000007;
            x = x + 1;
        }
        y = y + 1;
    }
    return acc;
}
fn main() -> i64 {
    let res: i64 = mandelbrot(500, 500, 100);
    return res % 256;
}
"#,
            rs_code: r#"
fn mandelbrot(w: i64, h: i64, max_iter: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut y: i64 = 0;
    while y < h {
        let ci: i64 = -1500 + (y * 3000) / h;
        let mut x: i64 = 0;
        while x < w {
            let cr: i64 = -2000 + (x * 3000) / w;
            let mut zr: i64 = 0;
            let mut zi: i64 = 0;
            let mut k: i64 = 0;
            let mut active = true;
            while active {
                if k < max_iter {
                    let zr2: i64 = (zr * zr) / 1000;
                    let zi2: i64 = (zi * zi) / 1000;
                    let mag: i64 = zr2 + zi2;
                    if mag > 4000 {
                        active = false;
                    } else {
                        zi = (2 * zr * zi) / 1000 + ci;
                        zr = zr2 - zi2 + cr;
                        k += 1;
                    }
                } else {
                    active = false;
                }
            }
            acc = (acc + k) % 1000000007;
            x += 1;
        }
        y += 1;
    }
    acc
}
fn main() {
    let res = mandelbrot(500, 500, 100);
    std::process::exit((res % 256) as i32);
}
"#,
            c_code: r#"
long long mandelbrot(long long w, long long h, long long max_iter) {
    long long acc = 0;
    long long y = 0;
    while (y < h) {
        long long ci = -1500 + (y * 3000) / h;
        long long x = 0;
        while (x < w) {
            long long cr = -2000 + (x * 3000) / w;
            long long zr = 0;
            long long zi = 0;
            long long k = 0;
            int active = 1;
            while (active) {
                if (k < max_iter) {
                    long long zr2 = (zr * zr) / 1000;
                    long long zi2 = (zi * zi) / 1000;
                    long long mag = zr2 + zi2;
                    if (mag > 4000) {
                        active = 0;
                    } else {
                        zi = (2 * zr * zi) / 1000 + ci;
                        zr = zr2 - zi2 + cr;
                        k++;
                    }
                } else {
                    active = 0;
                }
            }
            acc = (acc + k) % 1000000007;
            x++;
        }
        y++;
    }
    return acc;
}
int main() {
    long long res = mandelbrot(500, 500, 100);
    return (int)(res % 256);
}
"#,
            node_code: r#"
function mandelbrot(w, h, max_iter) {
    let acc = 0n;
    for (let y = 0n; y < h; y++) {
        const ci = -1500n + (y * 3000n) / h;
        for (let x = 0n; x < w; x++) {
            const cr = -2000n + (x * 3000n) / w;
            let zr = 0n;
            let zi = 0n;
            let k = 0n;
            let active = true;
            while (active) {
                if (k < max_iter) {
                    const zr2 = (zr * zr) / 1000n;
                    const zi2 = (zi * zi) / 1000n;
                    const mag = zr2 + zi2;
                    if (mag > 4000n) {
                        active = false;
                    } else {
                        zi = (2n * zr * zi) / 1000n + ci;
                        zr = zr2 - zi2 + cr;
                        k++;
                    }
                } else {
                    active = false;
                }
            }
            acc = (acc + k) % 1000000007n;
        }
    }
    return acc;
}
const res = mandelbrot(500n, 500n, 100n);
process.exit(Number(res % 256n));
"#,
            py_code: r#"
import sys
def trunc_div(a, b):
    return int(a / b)

def mandelbrot(w, h, max_iter):
    acc = 0
    y = 0
    while y < h:
        ci = -1500 + trunc_div(y * 3000, h)
        x = 0
        while x < w:
            cr = -2000 + trunc_div(x * 3000, w)
            zr = 0
            zi = 0
            k = 0
            active = True
            while active:
                if k < max_iter:
                    zr2 = trunc_div(zr * zr, 1000)
                    zi2 = trunc_div(zi * zi, 1000)
                    mag = zr2 + zi2
                    if mag > 4000:
                        active = False
                    else:
                        zi = trunc_div(2 * zr * zi, 1000) + ci
                        zr = zr2 - zi2 + cr
                        k += 1
                else:
                    active = False
            acc = (acc + k) % 1000000007
            x += 1
        y += 1
    return acc

res = mandelbrot(500, 500, 100)
sys.exit(res % 256)
"#,
        },

        // 13. Modular Exponentiation Accumulator
        BenchmarkWorkload {
            name: "Modular Exponentiation (5M iters)",
            expected_exit: 211,
            nl_code: r#"
fn pow_mod(base: i64, exp: i64, m: i64) -> i64 {
    let mut res: i64 = 1;
    let mut b: i64 = base % m;
    let mut e: i64 = exp;
    while e > 0 {
        if e % 2 == 1 {
            res = (res * b) % m;
        }
        b = (b * b) % m;
        e = e / 2;
    }
    return res;
}

fn mod_pow_accumulator(iters: i64) -> i64 {
    let m: i64 = 1000000007;
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= iters {
        let p: i64 = pow_mod(i, 13, m);
        acc = (acc + p) % m;
        i = i + 1;
    }
    return acc;
}

fn main() -> i64 {
    let res: i64 = mod_pow_accumulator(5000000);
    return res % 256;
}
"#,
            rs_code: r#"
fn pow_mod(base: i64, exp: i64, m: i64) -> i64 {
    let mut res = 1i64;
    let mut b = base % m;
    let mut e = exp;
    while e > 0 {
        if e % 2 == 1 {
            res = (res * b) % m;
        }
        b = (b * b) % m;
        e /= 2;
    }
    res
}

fn mod_pow_accumulator(iters: i64) -> i64 {
    let m = 1000000007i64;
    let mut acc = 0i64;
    let mut i = 1i64;
    while i <= iters {
        let p = pow_mod(i, 13, m);
        acc = (acc + p) % m;
        i += 1;
    }
    acc
}

fn main() {
    let res = mod_pow_accumulator(5000000);
    std::process::exit((res % 256) as i32);
}
"#,
            c_code: r#"
long long pow_mod(long long base, long long exp, long long m) {
    long long res = 1;
    long long b = base % m;
    long long e = exp;
    while (e > 0) {
        if (e % 2 == 1) {
            res = (res * b) % m;
        }
        b = (b * b) % m;
        e /= 2;
    }
    return res;
}

long long mod_pow_accumulator(long long iters) {
    long long m = 1000000007;
    long long acc = 0;
    long long i = 1;
    while (i <= iters) {
        long long p = pow_mod(i, 13, m);
        acc = (acc + p) % m;
        i++;
    }
    return acc;
}

int main() {
    long long res = mod_pow_accumulator(5000000);
    return (int)(res % 256);
}
"#,
            node_code: r#"
function pow_mod(base, exp, m) {
    let res = 1n;
    let b = base % m;
    let e = exp;
    while (e > 0n) {
        if (e % 2n === 1n) {
            res = (res * b) % m;
        }
        b = (b * b) % m;
        e = e / 2n;
    }
    return res;
}

function mod_pow_accumulator(iters) {
    const m = 1000000007n;
    let acc = 0n;
    for (let i = 1n; i <= iters; i++) {
        const p = pow_mod(i, 13n, m);
        acc = (acc + p) % m;
    }
    return acc;
}

const res = mod_pow_accumulator(5000000n);
process.exit(Number(res % 256n));
"#,
            py_code: r#"
import sys
def pow_mod(base, exp, m):
    res = 1
    b = base % m
    e = exp
    while e > 0:
        if e % 2 == 1:
            res = (res * b) % m
        b = (b * b) % m
        e = e // 2
    return res

m = 1000000007
acc = 0
for i in range(1, 5000001):
    acc = (acc + pow_mod(i, 13, m)) % m
sys.exit(acc % 256)
"#,
        },

        // 14. Monte Carlo Stochastic Geometry Simulation
        BenchmarkWorkload {
            name: "Monte Carlo Simulation (5M iters)",
            expected_exit: 22,
            nl_code: r#"
fn monte_carlo_pi(iters: i64) -> i64 {
    let mut inside: i64 = 0;
    let mut i: i64 = 0;
    let mut state: i64 = 123456789;
    while i < iters {
        state = (state * 1664525 + 1013904223) % 4294967296;
        let x: i64 = state % 10000;
        state = (state * 1664525 + 1013904223) % 4294967296;
        let y: i64 = state % 10000;
        let dist_sq: i64 = x * x + y * y;
        if dist_sq <= 100000000 {
            inside = inside + 1;
        }
        i = i + 1;
    }
    return inside;
}

fn main() -> i64 {
    let res: i64 = monte_carlo_pi(5000000);
    return res % 256;
}
"#,
            rs_code: r#"
fn monte_carlo_pi(iters: i64) -> i64 {
    let mut inside = 0i64;
    let mut i = 0i64;
    let mut state = 123456789i64;
    while i < iters {
        state = (state * 1664525 + 1013904223) % 4294967296;
        let x = state % 10000;
        state = (state * 1664525 + 1013904223) % 4294967296;
        let y = state % 10000;
        let dist_sq = x * x + y * y;
        if dist_sq <= 100000000 {
            inside += 1;
        }
        i += 1;
    }
    inside
}

fn main() {
    let res = monte_carlo_pi(5000000);
    std::process::exit((res % 256) as i32);
}
"#,
            c_code: r#"
long long monte_carlo_pi(long long iters) {
    long long inside = 0;
    long long i = 0;
    long long state = 123456789;
    while (i < iters) {
        state = (state * 1664525 + 1013904223) % 4294967296;
        long long x = state % 10000;
        state = (state * 1664525 + 1013904223) % 4294967296;
        long long y = state % 10000;
        long long dist_sq = x * x + y * y;
        if (dist_sq <= 100000000) {
            inside++;
        }
        i++;
    }
    return inside;
}

int main() {
    long long res = monte_carlo_pi(5000000);
    return (int)(res % 256);
}
"#,
            node_code: r#"
function monte_carlo_pi(iters) {
    let inside = 0n;
    let state = 123456789n;
    for (let i = 0n; i < iters; i++) {
        state = (state * 1664525n + 1013904223n) % 4294967296n;
        const x = state % 10000n;
        state = (state * 1664525n + 1013904223n) % 4294967296n;
        const y = state % 10000n;
        const dist_sq = x * x + y * y;
        if (dist_sq <= 100000000n) {
            inside++;
        }
    }
    return inside;
}

const res = monte_carlo_pi(5000000n);
process.exit(Number(res % 256n));
"#,
            py_code: r#"
import sys
def monte_carlo_pi(iters):
    inside = 0
    i = 0
    state = 123456789
    while i < iters:
        state = (state * 1664525 + 1013904223) % 4294967296
        x = state % 10000
        state = (state * 1664525 + 1013904223) % 4294967296
        y = state % 10000
        if x * x + y * y <= 100000000:
            inside += 1
        i += 1
    return inside

res = monte_carlo_pi(5000000)
sys.exit(res % 256)
"#,
        },

        // 15. Binary Search Kernel
        BenchmarkWorkload {
            name: "Binary Search Kernel (2M iters)",
            expected_exit: 227,
            nl_code: r#"
fn bsearch_kernel(iters: i64) -> i64 {
    let mut total: i64 = 0;
    let mut state: i64 = 123456789;
    let mut i: i64 = 0;
    while i < iters {
        state = (state * 1664525 + 1013904223) % 4294967296;
        let target: i64 = state % 7168;
        let mut low: i64 = 0;
        let mut high: i64 = 1023;
        let mut idx: i64 = 0 - 1;
        while low <= high {
            let mid: i64 = (low + high) / 2;
            let val: i64 = mid * 7 + 3;
            if val == target {
                idx = mid;
                break;
            } else {
                if val < target {
                    low = mid + 1;
                } else {
                    high = mid - 1;
                }
            }
        }
        total = (total + idx + 1) % 1000000007;
        i = i + 1;
    }
    return total;
}

fn main() -> i64 {
    let res: i64 = bsearch_kernel(2000000);
    return res % 256;
}
"#,
            rs_code: r#"
fn bsearch_kernel(iters: i64) -> i64 {
    let mut total = 0i64;
    let mut state = 123456789i64;
    let mut i = 0i64;
    while i < iters {
        state = (state * 1664525 + 1013904223) % 4294967296;
        let target = state % 7168;
        let mut low = 0i64;
        let mut high = 1023i64;
        let mut idx = -1i64;
        while low <= high {
            let mid = (low + high) / 2;
            let val = mid * 7 + 3;
            if val == target {
                idx = mid;
                break;
            } else if val < target {
                low = mid + 1;
            } else {
                high = mid - 1;
            }
        }
        total = (total + idx + 1) % 1000000007;
        i += 1;
    }
    total
}

fn main() {
    let res = bsearch_kernel(2000000);
    std::process::exit((res % 256) as i32);
}
"#,
            c_code: r#"
long long bsearch_kernel(long long iters) {
    long long total = 0;
    long long state = 123456789;
    for (long long i = 0; i < iters; i++) {
        state = (state * 1664525 + 1013904223) % 4294967296;
        long long target = state % 7168;
        long long low = 0;
        long long high = 1023;
        long long idx = -1;
        while (low <= high) {
            long long mid = (low + high) / 2;
            long long val = mid * 7 + 3;
            if (val == target) {
                idx = mid;
                break;
            } else if (val < target) {
                low = mid + 1;
            } else {
                high = mid - 1;
            }
        }
        total = (total + idx + 1) % 1000000007;
    }
    return total;
}

int main() {
    long long res = bsearch_kernel(2000000);
    return (int)(res % 256);
}
"#,
            node_code: r#"
function bsearch_kernel(iters) {
    let total = 0n;
    let state = 123456789n;
    for (let i = 0n; i < iters; i++) {
        state = (state * 1664525n + 1013904223n) % 4294967296n;
        const target = state % 7168n;
        let low = 0n;
        let high = 1023n;
        let idx = -1n;
        while (low <= high) {
            const mid = (low + high) / 2n;
            const val = mid * 7n + 3n;
            if (val === target) {
                idx = mid;
                break;
            } else if (val < target) {
                low = mid + 1n;
            } else {
                high = mid - 1n;
            }
        }
        total = (total + idx + 1n) % 1000000007n;
    }
    return total;
}

const res = bsearch_kernel(2000000n);
process.exit(Number(res % 256n));
"#,
            py_code: r#"
import sys
def bsearch_kernel(iters):
    total = 0
    state = 123456789
    for _ in range(iters):
        state = (state * 1664525 + 1013904223) % 4294967296
        target = state % 7168
        low = 0
        high = 1023
        idx = -1
        while low <= high:
            mid = (low + high) // 2
            val = mid * 7 + 3
            if val == target:
                idx = mid
                break
            elif val < target:
                low = mid + 1
            else:
                high = mid - 1
        total = (total + idx + 1) % 1000000007
    return total

res = bsearch_kernel(2000000)
sys.exit(res % 256)
"#,
        },

        // 16. Rule 110 Cellular Automaton
        BenchmarkWorkload {
            name: "Rule 110 Automaton (50K steps)",
            expected_exit: 38,
            nl_code: r#"
fn popcount64(n: i64) -> i64 {
    let mut num: i64 = n;
    let mut count: i64 = 0;
    while num != 0 {
        num = num & (num - 1);
        count = count + 1;
    }
    return count;
}

fn rule110_steps(steps: i64) -> i64 {
    let mut state: i64 = 1;
    let mut s: i64 = 0;
    let mask: i64 = 9223372036854775807;
    while s < steps {
        let left: i64 = (state << 1) | ((state >> 63) & 1);
        let right: i64 = ((state >> 1) & mask) | (state << 63);
        state = (state | right) ^ (left & state & right);
        s = s + 1;
    }
    return popcount64(state);
}

fn main() -> i64 {
    let res: i64 = rule110_steps(50000);
    return res % 256;
}
"#,
            rs_code: r#"
fn rule110_steps(steps: i64) -> i64 {
    let mut state: u64 = 1;
    let mut s = 0i64;
    while s < steps {
        let left = (state << 1) | (state >> 63);
        let right = (state >> 1) | (state << 63);
        state = (state | right) ^ (left & state & right);
        s += 1;
    }
    state.count_ones() as i64
}

fn main() {
    let res = rule110_steps(50000);
    std::process::exit((res % 256) as i32);
}
"#,
            c_code: r#"
long long rule110_steps(long long steps) {
    unsigned long long state = 1;
    for (long long s = 0; s < steps; s++) {
        unsigned long long left = (state << 1) | (state >> 63);
        unsigned long long right = (state >> 1) | (state << 63);
        state = (state | right) ^ (left & state & right);
    }
    long long count = 0;
    while (state > 0) {
        count += (state & 1);
        state >>= 1;
    }
    return count;
}

int main() {
    long long res = rule110_steps(50000);
    return (int)(res % 256);
}
"#,
            node_code: r#"
function rule110_steps(steps) {
    let state = 1n;
    const mask = 0xFFFFFFFFFFFFFFFFn;
    for (let s = 0n; s < steps; s++) {
        const left = ((state << 1n) | (state >> 63n)) & mask;
        const right = ((state >> 1n) | (state << 63n)) & mask;
        state = ((state | right) ^ (left & state & right)) & mask;
    }
    let count = 0n;
    while (state > 0n) {
        count += (state & 1n);
        state >>= 1n;
    }
    return Number(count);
}

const res = rule110_steps(50000n);
process.exit(Number(res % 256));
"#,
            py_code: r#"
import sys
def rule110_steps(steps):
    state = 1
    for _ in range(steps):
        left = ((state << 1) | (state >> 63)) & 0xFFFFFFFFFFFFFFFF
        right = ((state >> 1) | (state << 63)) & 0xFFFFFFFFFFFFFFFF
        state = ((state | right) ^ (left & state & right)) & 0xFFFFFFFFFFFFFFFF
    return bin(state).count('1')

res = rule110_steps(50000)
sys.exit(res % 256)
"#,
        },

        // 17. Fast 4x4 Matrix Exponentiation
        BenchmarkWorkload {
            name: "Matrix Exponentiation (1M power)",
            expected_exit: 166,
            nl_code: r#"
fn mat4_pow(power: i64) -> i64 {
    let mut m: [i64; 16] = [
        1, 1, 1, 1,
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0
    ];
    let mut r: [i64; 16] = [
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
        0, 0, 0, 1
    ];
    let mut p: i64 = power;
    while p > 0 {
        if p % 2 == 1 {
            let mut next_r: [i64; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            let mut i: i64 = 0;
            while i < 4 {
                let mut j: i64 = 0;
                while j < 4 {
                    let mut sum: i64 = 0;
                    let mut k: i64 = 0;
                    while k < 4 {
                        sum = (sum + r[i * 4 + k] * m[k * 4 + j]) % 1000000007;
                        k = k + 1;
                    }
                    next_r[i * 4 + j] = sum;
                    j = j + 1;
                }
                i = i + 1;
            }
            r = next_r;
        }
        let mut next_m: [i64; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut i2: i64 = 0;
        while i2 < 4 {
            let mut j2: i64 = 0;
            while j2 < 4 {
                let mut sum2: i64 = 0;
                let mut k2: i64 = 0;
                while k2 < 4 {
                    sum2 = (sum2 + m[i2 * 4 + k2] * m[k2 * 4 + j2]) % 1000000007;
                    k2 = k2 + 1;
                }
                next_m[i2 * 4 + j2] = sum2;
                j2 = j2 + 1;
            }
            i2 = i2 + 1;
        }
        m = next_m;
        p = p / 2;
    }
    return r[0] + r[5] + r[10] + r[15];
}

fn main() -> i64 {
    let res: i64 = mat4_pow(1000000);
    return res % 256;
}
"#,
            rs_code: r#"
fn mat4_pow(power: i64) -> i64 {
    const MOD: i64 = 1000000007;
    let mut m = [
        1i64, 1, 1, 1,
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
    ];
    let mut r = [
        1i64, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
        0, 0, 0, 1,
    ];
    let mul = |a: &[i64; 16], b: &[i64; 16]| -> [i64; 16] {
        let mut c = [0i64; 16];
        for i in 0..4 {
            for j in 0..4 {
                let mut s = 0i64;
                for k in 0..4 {
                    s = (s + a[i * 4 + k] * b[k * 4 + j]) % MOD;
                }
                c[i * 4 + j] = s;
            }
        }
        c
    };
    let mut p = power;
    while p > 0 {
        if p % 2 == 1 {
            r = mul(&r, &m);
        }
        m = mul(&m, &m);
        p /= 2;
    }
    r[0] + r[5] + r[10] + r[15]
}

fn main() {
    let res = mat4_pow(1000000);
    std::process::exit((res % 256) as i32);
}
"#,
            c_code: r#"
#define MOD 1000000007LL

void mul4(const long long a[16], const long long b[16], long long c[16]) {
    for (int i = 0; i < 4; i++) {
        for (int j = 0; j < 4; j++) {
            long long s = 0;
            for (int k = 0; k < 4; k++) {
                s = (s + a[i * 4 + k] * b[k * 4 + j]) % MOD;
            }
            c[i * 4 + j] = s;
        }
    }
}

long long mat4_pow(long long power) {
    long long m[16] = {
        1, 1, 1, 1,
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0
    };
    long long r[16] = {
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
        0, 0, 0, 1
    };
    long long temp[16];
    long long p = power;
    while (p > 0) {
        if (p % 2 == 1) {
            mul4(r, m, temp);
            for (int i = 0; i < 16; i++) r[i] = temp[i];
        }
        mul4(m, m, temp);
        for (int i = 0; i < 16; i++) m[i] = temp[i];
        p /= 2;
    }
    return r[0] + r[5] + r[10] + r[15];
}

int main() {
    long long res = mat4_pow(1000000);
    return (int)(res % 256);
}
"#,
            node_code: r#"
function mat4_pow(power) {
    const MOD = 1000000007n;
    let M = [
        1n, 1n, 1n, 1n,
        1n, 0n, 0n, 0n,
        0n, 1n, 0n, 0n,
        0n, 0n, 1n, 0n
    ];
    let R = [
        1n, 0n, 0n, 0n,
        0n, 1n, 0n, 0n,
        0n, 0n, 1n, 0n,
        0n, 0n, 0n, 1n
    ];
    function mul(A, B) {
        const C = new Array(16).fill(0n);
        for (let i = 0; i < 4; i++) {
            for (let j = 0; j < 4; j++) {
                let s = 0n;
                for (let k = 0; k < 4; k++) {
                    s = (s + A[i * 4 + k] * B[k * 4 + j]) % MOD;
                }
                C[i * 4 + j] = s;
            }
        }
        return C;
    }
    let p = power;
    while (p > 0n) {
        if (p % 2n === 1n) {
            R = mul(R, M);
        }
        M = mul(M, M);
        p = p / 2n;
    }
    return R[0] + R[5] + R[10] + R[15];
}

const res = mat4_pow(1000000n);
process.exit(Number(res % 256n));
"#,
            py_code: r#"
import sys
def mat4_pow(power):
    MOD = 1000000007
    M = [
        [1, 1, 1, 1],
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 0, 1, 0]
    ]
    R = [
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 0, 1, 0],
        [0, 0, 0, 1]
    ]
    def mul(A, B):
        C = [[0]*4 for _ in range(4)]
        for r in range(4):
            for c in range(4):
                s = 0
                for k in range(4):
                    s = (s + A[r][k] * B[k][c]) % MOD
                C[r][c] = s
        return C
    p = power
    while p > 0:
        if p % 2 == 1:
            R = mul(R, M)
        M = mul(M, M)
        p //= 2
    return R[0][0] + R[1][1] + R[2][2] + R[3][3]

res = mat4_pow(1000000)
sys.exit(res % 256)
"#,
        },

        // 18. Binary GCD Stein's Algorithm
        BenchmarkWorkload {
            name: "Binary GCD Stein (5M pairs)",
            expected_exit: 122,
            nl_code: r#"
fn stein_gcd(u_in: i64, v_in: i64) -> i64 {
    let mut u: i64 = u_in;
    let mut v: i64 = v_in;
    if u == 0 { return v; }
    if v == 0 { return u; }
    let mut shift: i64 = 0;
    while ((u | v) & 1) == 0 {
        u = u >> 1;
        v = v >> 1;
        shift = shift + 1;
    }
    while (u & 1) == 0 {
        u = u >> 1;
    }
    while v != 0 {
        while (v & 1) == 0 {
            v = v >> 1;
        }
        if u > v {
            let temp: i64 = u;
            u = v;
            v = temp;
        }
        v = v - u;
    }
    return u << shift;
}

fn stein_gcd_bench(iters: i64) -> i64 {
    let mut total: i64 = 0;
    let mut state_u: i64 = 123456789;
    let mut state_v: i64 = 987654321;
    let mut i: i64 = 0;
    while i < iters {
        state_u = (state_u * 1664525 + 1013904223) % 4294967296;
        state_v = (state_v * 1664525 + 1013904223) % 4294967296;
        let u: i64 = (state_u % 1000000) + 1;
        let v: i64 = (state_v % 1000000) + 1;
        let g: i64 = stein_gcd(u, v);
        total = (total + g) % 1000000007;
        i = i + 1;
    }
    return total;
}

fn main() -> i64 {
    let res: i64 = stein_gcd_bench(5000000);
    return res % 256;
}
"#,
            rs_code: r#"
fn stein_gcd(mut u: i64, mut v: i64) -> i64 {
    if u == 0 { return v; }
    if v == 0 { return u; }
    let mut shift = 0;
    while ((u | v) & 1) == 0 {
        u >>= 1;
        v >>= 1;
        shift += 1;
    }
    while (u & 1) == 0 {
        u >>= 1;
    }
    while v != 0 {
        while (v & 1) == 0 {
            v >>= 1;
        }
        if u > v {
            std::mem::swap(&mut u, &mut v);
        }
        v -= u;
    }
    u << shift
}

fn stein_gcd_bench(iters: i64) -> i64 {
    let mut total = 0i64;
    let mut state_u = 123456789i64;
    let mut state_v = 987654321i64;
    let mut i = 0i64;
    while i < iters {
        state_u = (state_u * 1664525 + 1013904223) % 4294967296;
        state_v = (state_v * 1664525 + 1013904223) % 4294967296;
        let u = (state_u % 1000000) + 1;
        let v = (state_v % 1000000) + 1;
        total = (total + stein_gcd(u, v)) % 1000000007;
        i += 1;
    }
    total
}

fn main() {
    let res = stein_gcd_bench(5000000);
    std::process::exit((res % 256) as i32);
}
"#,
            c_code: r#"
long long stein_gcd(long long u, long long v) {
    if (u == 0) return v;
    if (v == 0) return u;
    int shift = 0;
    while (((u | v) & 1) == 0) {
        u >>= 1;
        v >>= 1;
        shift++;
    }
    while ((u & 1) == 0) {
        u >>= 1;
    }
    while (v != 0) {
        while ((v & 1) == 0) {
            v >>= 1;
        }
        if (u > v) {
            long long t = u; u = v; v = t;
        }
        v -= u;
    }
    return u << shift;
}

long long stein_gcd_bench(long long iters) {
    long long total = 0;
    long long state_u = 123456789;
    long long state_v = 987654321;
    for (long long i = 0; i < iters; i++) {
        state_u = (state_u * 1664525 + 1013904223) % 4294967296;
        state_v = (state_v * 1664525 + 1013904223) % 4294967296;
        long long u = (state_u % 1000000) + 1;
        long long v = (state_v % 1000000) + 1;
        total = (total + stein_gcd(u, v)) % 1000000007;
    }
    return total;
}

int main() {
    long long res = stein_gcd_bench(5000000);
    return (int)(res % 256);
}
"#,
            node_code: r#"
function stein_gcd(u_in, v_in) {
    let u = u_in;
    let v = v_in;
    if (u === 0n) return v;
    if (v === 0n) return u;
    let shift = 0n;
    while (((u | v) & 1n) === 0n) {
        u >>= 1n;
        v >>= 1n;
        shift++;
    }
    while ((u & 1n) === 0n) {
        u >>= 1n;
    }
    while (v !== 0n) {
        while ((v & 1n) === 0n) {
            v >>= 1n;
        }
        if (u > v) {
            const t = u; u = v; v = t;
        }
        v -= u;
    }
    return u << shift;
}

function stein_gcd_bench(iters) {
    let total = 0n;
    let state_u = 123456789n;
    let state_v = 987654321n;
    for (let i = 0n; i < iters; i++) {
        state_u = (state_u * 1664525n + 1013904223n) % 4294967296n;
        state_v = (state_v * 1664525n + 1013904223n) % 4294967296n;
        const u = (state_u % 1000000n) + 1n;
        const v = (state_v % 1000000n) + 1n;
        total = (total + stein_gcd(u, v)) % 1000000007n;
    }
    return total;
}

const res = stein_gcd_bench(5000000n);
process.exit(Number(res % 256n));
"#,
            py_code: r#"
import sys
def stein_gcd(u, v):
    if u == 0: return v
    if v == 0: return u
    shift = 0
    while ((u | v) & 1) == 0:
        u >>= 1
        v >>= 1
        shift += 1
    while (u & 1) == 0:
        u >>= 1
    while v != 0:
        while (v & 1) == 0:
            v >>= 1
        if u > v:
            u, v = v, u
        v -= u
    return u << shift

def stein_gcd_bench(iters):
    total = 0
    state_u = 123456789
    state_v = 987654321
    for _ in range(iters):
        state_u = (state_u * 1664525 + 1013904223) % 4294967296
        state_v = (state_v * 1664525 + 1013904223) % 4294967296
        u = (state_u % 1000000) + 1
        v = (state_v % 1000000) + 1
        total = (total + stein_gcd(u, v)) % 1000000007
    return total

res = stein_gcd_bench(5000000)
sys.exit(res % 256)
"#,
        },

        // 19. 8-Point Discrete Cosine Transform
        BenchmarkWorkload {
            name: "Discrete Cosine Transform (2M iters)",
            expected_exit: 186,
            nl_code: r#"
fn dct_bench(iters: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let x0: i64 = (i * 3 + 1) % 100;
        let x1: i64 = (i * 7 + 2) % 100;
        let x2: i64 = (i * 11 + 3) % 100;
        let x3: i64 = (i * 13 + 4) % 100;
        let x4: i64 = (i * 17 + 5) % 100;
        let x5: i64 = (i * 19 + 6) % 100;
        let x6: i64 = (i * 23 + 7) % 100;
        let x7: i64 = (i * 29 + 8) % 100;
        let big_x0: i64 = x0 + x1 + x2 + x3 + x4 + x5 + x6 + x7;
        let big_x1: i64 = (x0 - x7) * 9 + (x1 - x6) * 7 + (x2 - x5) * 5 + (x3 - x4) * 3;
        acc = (acc + big_x0 * 13 + big_x1) % 1000000007;
        i = i + 1;
    }
    return acc;
}

fn main() -> i64 {
    let res: i64 = dct_bench(2000000);
    return res % 256;
}
"#,
            rs_code: r#"
fn dct_bench(iters: i64) -> i64 {
    let mut acc = 0i64;
    let mut i = 0i64;
    while i < iters {
        let x0 = (i * 3 + 1) % 100;
        let x1 = (i * 7 + 2) % 100;
        let x2 = (i * 11 + 3) % 100;
        let x3 = (i * 13 + 4) % 100;
        let x4 = (i * 17 + 5) % 100;
        let x5 = (i * 19 + 6) % 100;
        let x6 = (i * 23 + 7) % 100;
        let x7 = (i * 29 + 8) % 100;
        let big_x0 = x0 + x1 + x2 + x3 + x4 + x5 + x6 + x7;
        let big_x1 = (x0 - x7) * 9 + (x1 - x6) * 7 + (x2 - x5) * 5 + (x3 - x4) * 3;
        acc = (acc + big_x0 * 13 + big_x1) % 1000000007;
        i += 1;
    }
    acc
}

fn main() {
    let res = dct_bench(2000000);
    std::process::exit((res % 256) as i32);
}
"#,
            c_code: r#"
long long dct_bench(long long iters) {
    long long acc = 0;
    for (long long i = 0; i < iters; i++) {
        long long x0 = (i * 3 + 1) % 100;
        long long x1 = (i * 7 + 2) % 100;
        long long x2 = (i * 11 + 3) % 100;
        long long x3 = (i * 13 + 4) % 100;
        long long x4 = (i * 17 + 5) % 100;
        long long x5 = (i * 19 + 6) % 100;
        long long x6 = (i * 23 + 7) % 100;
        long long x7 = (i * 29 + 8) % 100;
        long long big_x0 = x0 + x1 + x2 + x3 + x4 + x5 + x6 + x7;
        long long big_x1 = (x0 - x7) * 9 + (x1 - x6) * 7 + (x2 - x5) * 5 + (x3 - x4) * 3;
        acc = (acc + big_x0 * 13 + big_x1) % 1000000007;
    }
    return acc;
}

int main() {
    long long res = dct_bench(2000000);
    return (int)(res % 256);
}
"#,
            node_code: r#"
function dct_bench(iters) {
    let acc = 0n;
    for (let i = 0n; i < iters; i++) {
        const x0 = (i * 3n + 1n) % 100n;
        const x1 = (i * 7n + 2n) % 100n;
        const x2 = (i * 11n + 3n) % 100n;
        const x3 = (i * 13n + 4n) % 100n;
        const x4 = (i * 17n + 5n) % 100n;
        const x5 = (i * 19n + 6n) % 100n;
        const x6 = (i * 23n + 7n) % 100n;
        const x7 = (i * 29n + 8n) % 100n;
        const big_x0 = x0 + x1 + x2 + x3 + x4 + x5 + x6 + x7;
        const big_x1 = (x0 - x7) * 9n + (x1 - x6) * 7n + (x2 - x5) * 5n + (x3 - x4) * 3n;
        acc = (acc + big_x0 * 13n + big_x1) % 1000000007n;
    }
    return acc;
}

const res = dct_bench(2000000n);
process.exit(Number(res % 256n));
"#,
            py_code: r#"
import sys
def dct_bench(iters):
    acc = 0
    for i in range(iters):
        x0 = (i * 3 + 1) % 100
        x1 = (i * 7 + 2) % 100
        x2 = (i * 11 + 3) % 100
        x3 = (i * 13 + 4) % 100
        x4 = (i * 17 + 5) % 100
        x5 = (i * 19 + 6) % 100
        x6 = (i * 23 + 7) % 100
        x7 = (i * 29 + 8) % 100
        big_x0 = x0 + x1 + x2 + x3 + x4 + x5 + x6 + x7
        big_x1 = (x0 - x7) * 9 + (x1 - x6) * 7 + (x2 - x5) * 5 + (x3 - x4) * 3
        acc = (acc + big_x0 * 13 + big_x1) % 1000000007
    return acc

res = dct_bench(2000000)
sys.exit(res % 256)
"#,
        },

        // 20. Integer Square Root Newton-Raphson
        BenchmarkWorkload {
            name: "Newton Integer Sqrt (5M iters)",
            expected_exit: 160,
            nl_code: r#"
fn isqrt_newton(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    let mut x: i64 = n;
    let mut y: i64 = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    return x;
}

fn isqrt_bench(iters: i64) -> i64 {
    let mut total: i64 = 0;
    let mut state: i64 = 123456789;
    let mut i: i64 = 0;
    while i < iters {
        state = (state * 1664525 + 1013904223) % 4294967296;
        let n: i64 = (state % 10000000) + 1;
        let root: i64 = isqrt_newton(n);
        total = (total + root) % 1000000007;
        i = i + 1;
    }
    return total;
}

fn main() -> i64 {
    let res: i64 = isqrt_bench(5000000);
    return res % 256;
}
"#,
            rs_code: r#"
fn isqrt_newton(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

fn isqrt_bench(iters: i64) -> i64 {
    let mut total = 0i64;
    let mut state = 123456789i64;
    let mut i = 0i64;
    while i < iters {
        state = (state * 1664525 + 1013904223) % 4294967296;
        let n = (state % 10000000) + 1;
        total = (total + isqrt_newton(n)) % 1000000007;
        i += 1;
    }
    total
}

fn main() {
    let res = isqrt_bench(5000000);
    std::process::exit((res % 256) as i32);
}
"#,
            c_code: r#"
long long isqrt_newton(long long n) {
    if (n <= 1) return n;
    long long x = n;
    long long y = (x + 1) / 2;
    while (y < x) {
        x = y;
        y = (x + n / x) / 2;
    }
    return x;
}

long long isqrt_bench(long long iters) {
    long long total = 0;
    long long state = 123456789;
    for (long long i = 0; i < iters; i++) {
        state = (state * 1664525 + 1013904223) % 4294967296;
        long long n = (state % 10000000) + 1;
        total = (total + isqrt_newton(n)) % 1000000007;
    }
    return total;
}

int main() {
    long long res = isqrt_bench(5000000);
    return (int)(res % 256);
}
"#,
            node_code: r#"
function isqrt_newton(n) {
    if (n <= 1n) return n;
    let x = n;
    let y = (x + 1n) / 2n;
    while (y < x) {
        x = y;
        y = (x + n / x) / 2n;
    }
    return x;
}

function isqrt_bench(iters) {
    let total = 0n;
    let state = 123456789n;
    for (let i = 0n; i < iters; i++) {
        state = (state * 1664525n + 1013904223n) % 4294967296n;
        const n = (state % 10000000n) + 1n;
        total = (total + isqrt_newton(n)) % 1000000007n;
    }
    return total;
}

const res = isqrt_bench(5000000n);
process.exit(Number(res % 256n));
"#,
            py_code: r#"
import sys
def isqrt_newton(n):
    if n <= 1:
        return n
    x = n
    y = (x + 1) // 2
    while y < x:
        x = y
        y = (x + n // x) // 2
    return x

def isqrt_bench(iters):
    total = 0
    state = 123456789
    for _ in range(iters):
        state = (state * 1664525 + 1013904223) % 4294967296
        n = (state % 10000000) + 1
        total = (total + isqrt_newton(n)) % 1000000007
    return total

res = isqrt_bench(5000000)
sys.exit(res % 256)
"#,
        },
        // 21. Matrix Inversion 4x4 (500k ops)
        BenchmarkWorkload {
            name: "Matrix Inversion 4x4 (500k ops)",
            expected_exit: 44,
            nl_code: r#"
fn main() -> i64 {
    let mut a: [f64; 16] = [
        5.0, 1.0, 0.0, 2.0,
        1.0, 6.0, 2.0, 0.0,
        0.0, 2.0, 7.0, 1.0,
        2.0, 0.0, 1.0, 8.0
    ];
    let mut i: i64 = 0;
    let mut trace_sum: f64 = 0.0;
    while i < 500000 {
        let inv: [f64; 16] = mat_inv4(a);
        let tr: f64 = mat_trace4(inv);
        trace_sum = trace_sum + tr;
        a[0] = a[0] + 0.000001;
        i = i + 1;
    }
    let res: i64 = to_int(trace_sum);
    return (res % 256 + 256) % 256;
}
"#,
            rs_code: r#"
fn det3(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64, g: f64, h: f64, i: f64) -> f64 {
    a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)
}

fn inv4(m: &[f64; 16]) -> [f64; 16] {
    let c00 = det3(m[5], m[6], m[7], m[9], m[10], m[11], m[13], m[14], m[15]);
    let c01 = -det3(m[4], m[6], m[7], m[8], m[10], m[11], m[12], m[14], m[15]);
    let c02 = det3(m[4], m[5], m[7], m[8], m[9], m[11], m[12], m[13], m[15]);
    let c03 = -det3(m[4], m[5], m[6], m[8], m[9], m[10], m[12], m[13], m[14]);

    let c10 = -det3(m[1], m[2], m[3], m[9], m[10], m[11], m[13], m[14], m[15]);
    let c11 = det3(m[0], m[2], m[3], m[8], m[10], m[11], m[12], m[14], m[15]);
    let c12 = -det3(m[0], m[1], m[3], m[8], m[9], m[11], m[12], m[13], m[15]);
    let c13 = det3(m[0], m[1], m[2], m[8], m[9], m[10], m[12], m[13], m[14]);

    let c20 = det3(m[1], m[2], m[3], m[5], m[6], m[7], m[13], m[14], m[15]);
    let c21 = -det3(m[0], m[2], m[3], m[4], m[6], m[7], m[12], m[14], m[15]);
    let c22 = det3(m[0], m[1], m[3], m[4], m[5], m[7], m[12], m[13], m[15]);
    let c23 = -det3(m[0], m[1], m[2], m[4], m[5], m[6], m[12], m[13], m[14]);

    let c30 = -det3(m[1], m[2], m[3], m[5], m[6], m[7], m[9], m[10], m[11]);
    let c31 = det3(m[0], m[2], m[3], m[4], m[6], m[7], m[8], m[10], m[11]);
    let c32 = -det3(m[0], m[1], m[3], m[4], m[5], m[7], m[8], m[9], m[11]);
    let c33 = det3(m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]);

    let det = m[0] * c00 + m[1] * c01 + m[2] * c02 + m[3] * c03;
    let inv_det = 1.0 / det;

    [
        c00 * inv_det, c10 * inv_det, c20 * inv_det, c30 * inv_det,
        c01 * inv_det, c11 * inv_det, c21 * inv_det, c31 * inv_det,
        c02 * inv_det, c12 * inv_det, c22 * inv_det, c32 * inv_det,
        c03 * inv_det, c13 * inv_det, c23 * inv_det, c33 * inv_det,
    ]
}

fn main() {
    let mut a: [f64; 16] = [
        5.0, 1.0, 0.0, 2.0,
        1.0, 6.0, 2.0, 0.0,
        0.0, 2.0, 7.0, 1.0,
        2.0, 0.0, 1.0, 8.0,
    ];
    let mut i = 0i64;
    let mut trace_sum = 0.0f64;
    while i < 500000 {
        let inv = inv4(&a);
        trace_sum += inv[0] + inv[5] + inv[10] + inv[15];
        a[0] += 0.000001;
        i += 1;
    }
    let res = trace_sum as i64;
    std::process::exit(((res % 256 + 256) % 256) as i32);
}
"#,
            c_code: r#"
static inline double det3(double a, double b, double c, double d, double e, double f, double g, double h, double i) {
    return a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
}

static inline void inv4(const double* m, double* out) {
    double c00 = det3(m[5], m[6], m[7], m[9], m[10], m[11], m[13], m[14], m[15]);
    double c01 = -det3(m[4], m[6], m[7], m[8], m[10], m[11], m[12], m[14], m[15]);
    double c02 = det3(m[4], m[5], m[7], m[8], m[9], m[11], m[12], m[13], m[15]);
    double c03 = -det3(m[4], m[5], m[6], m[8], m[9], m[10], m[12], m[13], m[14]);

    double c10 = -det3(m[1], m[2], m[3], m[9], m[10], m[11], m[13], m[14], m[15]);
    double c11 = det3(m[0], m[2], m[3], m[8], m[10], m[11], m[12], m[14], m[15]);
    double c12 = -det3(m[0], m[1], m[3], m[8], m[9], m[11], m[12], m[13], m[15]);
    double c13 = det3(m[0], m[1], m[2], m[8], m[9], m[10], m[12], m[13], m[14]);

    double c20 = det3(m[1], m[2], m[3], m[5], m[6], m[7], m[13], m[14], m[15]);
    double c21 = -det3(m[0], m[2], m[3], m[4], m[6], m[7], m[12], m[14], m[15]);
    double c22 = det3(m[0], m[1], m[3], m[4], m[5], m[7], m[12], m[13], m[15]);
    double c23 = -det3(m[0], m[1], m[2], m[4], m[5], m[6], m[12], m[13], m[14]);

    double c30 = -det3(m[1], m[2], m[3], m[5], m[6], m[7], m[9], m[10], m[11]);
    double c31 = det3(m[0], m[2], m[3], m[4], m[6], m[7], m[8], m[10], m[11]);
    double c32 = -det3(m[0], m[1], m[3], m[4], m[5], m[7], m[8], m[9], m[11]);
    double c33 = det3(m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]);

    double det = m[0] * c00 + m[1] * c01 + m[2] * c02 + m[3] * c03;
    double inv_det = 1.0 / det;

    out[0] = c00 * inv_det; out[1] = c10 * inv_det; out[2] = c20 * inv_det; out[3] = c30 * inv_det;
    out[4] = c01 * inv_det; out[5] = c11 * inv_det; out[6] = c21 * inv_det; out[7] = c31 * inv_det;
    out[8] = c02 * inv_det; out[9] = c12 * inv_det; out[10] = c22 * inv_det; out[11] = c32 * inv_det;
    out[12] = c03 * inv_det; out[13] = c13 * inv_det; out[14] = c23 * inv_det; out[15] = c33 * inv_det;
}

int main() {
    double a[16] = {
        5.0, 1.0, 0.0, 2.0,
        1.0, 6.0, 2.0, 0.0,
        0.0, 2.0, 7.0, 1.0,
        2.0, 0.0, 1.0, 8.0
    };
    double inv[16];
    double trace_sum = 0.0;
    for (long long i = 0; i < 500000; i++) {
        inv4(a, inv);
        trace_sum += inv[0] + inv[5] + inv[10] + inv[15];
        a[0] += 0.000001;
    }
    long long res = (long long)trace_sum;
    return (int)(((res % 256) + 256) % 256);
}
"#,
            node_code: r#"
function det3(a, b, c, d, e, f, g, h, i) {
    return a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
}
function inv4(m) {
    let c00 = det3(m[5], m[6], m[7], m[9], m[10], m[11], m[13], m[14], m[15]);
    let c01 = -det3(m[4], m[6], m[7], m[8], m[10], m[11], m[12], m[14], m[15]);
    let c02 = det3(m[4], m[5], m[7], m[8], m[9], m[11], m[12], m[13], m[15]);
    let c03 = -det3(m[4], m[5], m[6], m[8], m[9], m[10], m[12], m[13], m[14]);

    let c10 = -det3(m[1], m[2], m[3], m[9], m[10], m[11], m[13], m[14], m[15]);
    let c11 = det3(m[0], m[2], m[3], m[8], m[10], m[11], m[12], m[14], m[15]);
    let c12 = -det3(m[0], m[1], m[3], m[8], m[9], m[11], m[12], m[13], m[15]);
    let c13 = det3(m[0], m[1], m[2], m[8], m[9], m[10], m[12], m[13], m[14]);

    let c20 = det3(m[1], m[2], m[3], m[5], m[6], m[7], m[13], m[14], m[15]);
    let c21 = -det3(m[0], m[2], m[3], m[4], m[6], m[7], m[12], m[14], m[15]);
    let c22 = det3(m[0], m[1], m[3], m[4], m[5], m[7], m[12], m[13], m[15]);
    let c23 = -det3(m[0], m[1], m[2], m[4], m[5], m[6], m[12], m[13], m[14]);

    let c30 = -det3(m[1], m[2], m[3], m[5], m[6], m[7], m[9], m[10], m[11]);
    let c31 = det3(m[0], m[2], m[3], m[4], m[6], m[7], m[8], m[10], m[11]);
    let c32 = -det3(m[0], m[1], m[3], m[4], m[5], m[7], m[8], m[9], m[11]);
    let c33 = det3(m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]);

    let det = m[0] * c00 + m[1] * c01 + m[2] * c02 + m[3] * c03;
    let inv_det = 1.0 / det;

    return [
        c00 * inv_det, c10 * inv_det, c20 * inv_det, c30 * inv_det,
        c01 * inv_det, c11 * inv_det, c21 * inv_det, c31 * inv_det,
        c02 * inv_det, c12 * inv_det, c22 * inv_det, c32 * inv_det,
        c03 * inv_det, c13 * inv_det, c23 * inv_det, c33 * inv_det,
    ];
}
let a = [
    5.0, 1.0, 0.0, 2.0,
    1.0, 6.0, 2.0, 0.0,
    0.0, 2.0, 7.0, 1.0,
    2.0, 0.0, 1.0, 8.0
];
let trace_sum = 0.0;
for (let i = 0; i < 500000; i++) {
    let inv = inv4(a);
    trace_sum += inv[0] + inv[5] + inv[10] + inv[15];
    a[0] += 0.000001;
}
let res = Math.floor(trace_sum);
process.exit(((res % 256) + 256) % 256);
"#,
            py_code: r#"
import sys

def det3(a, b, c, d, e, f, g, h, i):
    return a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)

def inv4(m):
    c00 = det3(m[5], m[6], m[7], m[9], m[10], m[11], m[13], m[14], m[15])
    c01 = -det3(m[4], m[6], m[7], m[8], m[10], m[11], m[12], m[14], m[15])
    c02 = det3(m[4], m[5], m[7], m[8], m[9], m[11], m[12], m[13], m[15])
    c03 = -det3(m[4], m[5], m[6], m[8], m[9], m[10], m[12], m[13], m[14])

    c10 = -det3(m[1], m[2], m[3], m[9], m[10], m[11], m[13], m[14], m[15])
    c11 = det3(m[0], m[2], m[3], m[8], m[10], m[11], m[12], m[14], m[15])
    c12 = -det3(m[0], m[1], m[3], m[8], m[9], m[11], m[12], m[13], m[15])
    c13 = det3(m[0], m[1], m[2], m[8], m[9], m[10], m[12], m[13], m[14])

    c20 = det3(m[1], m[2], m[3], m[5], m[6], m[7], m[13], m[14], m[15])
    c21 = -det3(m[0], m[2], m[3], m[4], m[6], m[7], m[12], m[14], m[15])
    c22 = det3(m[0], m[1], m[3], m[4], m[5], m[7], m[12], m[13], m[15])
    c23 = -det3(m[0], m[1], m[2], m[4], m[5], m[6], m[12], m[13], m[14])

    c30 = -det3(m[1], m[2], m[3], m[5], m[6], m[7], m[9], m[10], m[11])
    c31 = det3(m[0], m[2], m[3], m[4], m[6], m[7], m[8], m[10], m[11])
    c32 = -det3(m[0], m[1], m[3], m[4], m[5], m[7], m[8], m[9], m[11])
    c33 = det3(m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10])

    det = m[0] * c00 + m[1] * c01 + m[2] * c02 + m[3] * c03
    inv_det = 1.0 / det

    return [
        c00 * inv_det, c10 * inv_det, c20 * inv_det, c30 * inv_det,
        c01 * inv_det, c11 * inv_det, c21 * inv_det, c31 * inv_det,
        c02 * inv_det, c12 * inv_det, c22 * inv_det, c32 * inv_det,
        c03 * inv_det, c13 * inv_det, c23 * inv_det, c33 * inv_det,
    ]

a = [
    5.0, 1.0, 0.0, 2.0,
    1.0, 6.0, 2.0, 0.0,
    0.0, 2.0, 7.0, 1.0,
    2.0, 0.0, 1.0, 8.0
]
trace_sum = 0.0
for i in range(500000):
    inv = inv4(a)
    trace_sum += inv[0] + inv[5] + inv[10] + inv[15]
    a[0] += 0.000001
res = int(trace_sum)
sys.exit(((res % 256) + 256) % 256)
"#,
        },
        // 22. 16-point FFT (500k ops)
        BenchmarkWorkload {
            name: "16-point FFT (500k ops)",
            expected_exit: 30,
            nl_code: r#"
fn main() -> i64 {
    let mut re: [f64; 16] = [
        1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0,
        9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0
    ];
    let im: [f64; 16] = [
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
    ];
    let mut i: i64 = 0;
    let mut sum: f64 = 0.0;
    while i < 500000 {
        let out: [f64; 16] = fft16_re(re, im);
        sum = sum + out[0] + out[1] + out[2] + out[3];
        re[0] = re[0] + 0.000001;
        i = i + 1;
    }
    let res: i64 = to_int(sum);
    return (res % 256 + 256) % 256;
}
"#,
            rs_code: r#"
fn fft16(re_in: &[f64; 16], im_in: &[f64; 16]) -> [f64; 16] {
    const REV: [usize; 16] = [0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15];
    let mut r = [0.0f64; 16];
    let mut im = [0.0f64; 16];
    for k in 0..16 {
        r[k] = re_in[REV[k]];
        im[k] = im_in[REV[k]];
    }
    // Stage len 2
    for k in (0..16).step_by(2) {
        let ur = r[k]; let ui = im[k];
        let vr = r[k+1]; let vi = im[k+1];
        r[k] = ur + vr; im[k] = ui + vi;
        r[k+1] = ur - vr; im[k+1] = ui - vi;
    }
    // Stage len 4
    for k in (0..16).step_by(4) {
        let ur0 = r[k]; let ui0 = im[k];
        let vr0 = r[k+2]; let vi0 = im[k+2];
        r[k] = ur0 + vr0; im[k] = ui0 + vi0;
        r[k+2] = ur0 - vr0; im[k+2] = ui0 - vi0;

        let ur1 = r[k+1]; let ui1 = im[k+1];
        let tr = im[k+3]; let ti = -r[k+3];
        r[k+1] = ur1 + tr; im[k+1] = ui1 + ti;
        r[k+3] = ur1 - tr; im[k+3] = ui1 - ti;
    }
    // Stage len 8
    let pi = std::f64::consts::PI;
    for k in (0..16).step_by(8) {
        for j in 0..4 {
            let angle = -2.0 * pi * (j as f64) / 8.0;
            let wr = angle.cos(); let wi = angle.sin();
            let vr = r[k + j + 4]; let vi = im[k + j + 4];
            let tr = vr * wr - vi * wi;
            let ti = vr * wi + vi * wr;
            let ur = r[k + j]; let ui = im[k + j];
            r[k + j] = ur + tr; im[k + j] = ui + ti;
            r[k + j + 4] = ur - tr; im[k + j + 4] = ui - ti;
        }
    }
    // Stage len 16
    for j in 0..8 {
        let angle = -2.0 * pi * (j as f64) / 16.0;
        let wr = angle.cos(); let wi = angle.sin();
        let vr = r[j + 8]; let vi = im[j + 8];
        let tr = vr * wr - vi * wi;
        let ti = vr * wi + vi * wr;
        let ur = r[j]; let ui = im[j];
        r[j] = ur + tr; im[j] = ui + ti;
        r[j + 8] = ur - tr; im[j + 8] = ui - ti;
    }
    r
}

fn main() {
    let mut re = [
        1.0f64, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0,
        9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0
    ];
    let im = [0.0f64; 16];
    let mut sum = 0.0f64;
    let mut i = 0i64;
    while i < 500000 {
        let out = fft16(&re, &im);
        sum += out[0] + out[1] + out[2] + out[3];
        re[0] += 0.000001;
        i += 1;
    }
    let res = sum as i64;
    std::process::exit(((res % 256 + 256) % 256) as i32);
}
"#,
            c_code: r#"
#include <math.h>

void fft16(const double* re_in, const double* im_in, double* re_out) {
    static const int rev[16] = {0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15};
    double r[16], im[16];
    for (int k = 0; k < 16; k++) {
        r[k] = re_in[rev[k]];
        im[k] = im_in[rev[k]];
    }
    for (int k = 0; k < 16; k += 2) {
        double u_r = r[k], u_i = im[k];
        double v_r = r[k+1], v_i = im[k+1];
        r[k] = u_r + v_r; im[k] = u_i + v_i;
        r[k+1] = u_r - v_r; im[k+1] = u_i - v_i;
    }
    for (int k = 0; k < 16; k += 4) {
        double u_r = r[k], u_i = im[k];
        double v_r = r[k+2], v_i = im[k+2];
        r[k] = u_r + v_r; im[k] = u_i + v_i;
        r[k+2] = u_r - v_r; im[k+2] = u_i - v_i;
        u_r = r[k+1]; u_i = im[k+1];
        double t_r = im[k+3], t_i = -r[k+3];
        r[k+1] = u_r + t_r; im[k+1] = u_i + t_i;
        r[k+3] = u_r - t_r; im[k+3] = u_i - t_i;
    }
    double pi = 3.14159265358979323846;
    for (int k = 0; k < 16; k += 8) {
        for (int j = 0; j < 4; j++) {
            double angle = -2.0 * pi * (double)j / 8.0;
            double wr = cos(angle), wi = sin(angle);
            double vr = r[k + j + 4], vi = im[k + j + 4];
            double tr = vr * wr - vi * wi;
            double ti = vr * wi + vi * wr;
            double ur = r[k + j], ui = im[k + j];
            r[k + j] = ur + tr; im[k + j] = ui + ti;
            r[k + j + 4] = ur - tr; im[k + j + 4] = ui - ti;
        }
    }
    for (int j = 0; j < 8; j++) {
        double angle = -2.0 * pi * (double)j / 16.0;
        double wr = cos(angle), wi = sin(angle);
        double vr = r[j + 8], vi = im[j + 8];
        double tr = vr * wr - vi * wi;
        double ti = vr * wi + vi * wr;
        double ur = r[j], ui = im[j];
        r[j] = ur + tr; im[j] = ui + ti;
        r[j + 8] = ur - tr; im[j + 8] = ui - ti;
    }
    for (int k = 0; k < 16; k++) {
        re_out[k] = r[k];
    }
}

int main() {
    double re[16] = {
        1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0,
        9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0
    };
    double im[16] = {0};
    double out_r[16];
    double sum = 0.0;
    for (long long i = 0; i < 500000; i++) {
        fft16(re, im, out_r);
        sum += out_r[0] + out_r[1] + out_r[2] + out_r[3];
        re[0] += 0.000001;
    }
    long long res = (long long)sum;
    return (int)(((res % 256) + 256) % 256);
}
"#,
            node_code: r#"
function fft16(re_in, im_in) {
    const rev = [0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15];
    let r = new Float64Array(16);
    let im = new Float64Array(16);
    for (let k = 0; k < 16; k++) {
        r[k] = re_in[rev[k]];
        im[k] = im_in[rev[k]];
    }
    for (let k = 0; k < 16; k += 2) {
        let ur = r[k], ui = im[k];
        let vr = r[k+1], vi = im[k+1];
        r[k] = ur + vr; im[k] = ui + vi;
        r[k+1] = ur - vr; im[k+1] = ui - vi;
    }
    for (let k = 0; k < 16; k += 4) {
        let ur = r[k], ui = im[k];
        let vr = r[k+2], vi = im[k+2];
        r[k] = ur + vr; im[k] = ui + vi;
        r[k+2] = ur - vr; im[k+2] = ui - vi;
        ur = r[k+1]; ui = im[k+1];
        let tr = im[k+3], ti = -r[k+3];
        r[k+1] = ur + tr; im[k+1] = ui + ti;
        r[k+3] = ur - tr; im[k+3] = ui - ti;
    }
    const pi = Math.PI;
    for (let k = 0; k < 16; k += 8) {
        for (let j = 0; j < 4; j++) {
            let angle = -2.0 * pi * j / 8.0;
            let wr = Math.cos(angle), wi = Math.sin(angle);
            let vr = r[k + j + 4], vi = im[k + j + 4];
            let tr = vr * wr - vi * wi;
            let ti = vr * wi + vi * wr;
            let ur = r[k + j], ui = im[k + j];
            r[k + j] = ur + tr; im[k + j] = ui + ti;
            r[k + j + 4] = ur - tr; im[k + j + 4] = ui - ti;
        }
    }
    for (let j = 0; j < 8; j++) {
        let angle = -2.0 * pi * j / 16.0;
        let wr = Math.cos(angle), wi = Math.sin(angle);
        let vr = r[j + 8], vi = im[j + 8];
        let tr = vr * wr - vi * wi;
        let ti = vr * wi + vi * wr;
        let ur = r[j], ui = im[j];
        r[j] = ur + tr; im[j] = ui + ti;
        r[j + 8] = ur - tr; im[j + 8] = ui - ti;
    }
    return r;
}

let re = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0];
let im = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
let sum = 0.0;
for (let i = 0; i < 500000; i++) {
    let out = fft16(re, im);
    sum += out[0] + out[1] + out[2] + out[3];
    re[0] += 0.000001;
}
let res = Math.floor(sum);
process.exit(((res % 256) + 256) % 256);
"#,
            py_code: r#"
import math, sys

def fft16(re_in, im_in):
    rev = [0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15]
    r = [re_in[rev[k]] for k in range(16)]
    im = [im_in[rev[k]] for k in range(16)]
    for k in range(0, 16, 2):
        ur, ui = r[k], im[k]
        vr, vi = r[k+1], im[k+1]
        r[k], im[k] = ur + vr, ui + vi
        r[k+1], im[k+1] = ur - vr, ui - vi
    for k in range(0, 16, 4):
        ur, ui = r[k], im[k]
        vr, vi = r[k+2], im[k+2]
        r[k], im[k] = ur + vr, ui + vi
        r[k+2], im[k+2] = ur - vr, ui - vi
        ur, ui = r[k+1], im[k+1]
        tr, ti = im[k+3], -r[k+3]
        r[k+1], im[k+1] = ur + tr, ui + ti
        r[k+3], im[k+3] = ur - tr, ui - ti
    pi = 3.14159265358979323846
    for k in range(0, 16, 8):
        for j in range(4):
            angle = -2.0 * pi * j / 8.0
            wr, wi = math.cos(angle), math.sin(angle)
            vr, vi = r[k + j + 4], im[k + j + 4]
            tr = vr * wr - vi * wi
            ti = vr * wi + vi * wr
            ur, ui = r[k + j], im[k + j]
            r[k + j], im[k + j] = ur + tr, ui + ti
            r[k + j + 4], im[k + j + 4] = ur - tr, ui - ti
    for j in range(8):
        angle = -2.0 * pi * j / 16.0
        wr, wi = math.cos(angle), math.sin(angle)
        vr, vi = r[j + 8], im[j + 8]
        tr = vr * wr - vi * wi
        ti = vr * wi + vi * wr
        ur, ui = r[j], im[j]
        r[j], im[j] = ur + tr, ui + ti
        r[j + 8], im[j + 8] = ur - tr, ui - ti
    return r

re = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0]
im = [0.0] * 16
sum_val = 0.0
for i in range(500000):
    out = fft16(re, im)
    sum_val += out[0] + out[1] + out[2] + out[3]
    re[0] += 0.000001
res = int(sum_val)
sys.exit(((res % 256) + 256) % 256)
"#,
        },
        // 23. Transcendental Integration (1M steps)
        BenchmarkWorkload {
            name: "Transcendental Integration (1M steps)",
            expected_exit: 40,
            nl_code: r#"
fn main() -> i64 {
    let dt: f64 = 0.000005;
    let mut sum: f64 = 0.0;
    let mut i: i64 = 0;
    while i < 1000000 {
        let t: f64 = to_float(i) * dt;
        let s: f64 = sin(t);
        let c: f64 = cos(t * 2.0);
        let e: f64 = exp(s * 0.1);
        sum = sum + (s + c + e) * dt;
        i = i + 1;
    }
    let res: i64 = to_int(sum * 100.0);
    return (res % 256 + 256) % 256;
}
"#,
            rs_code: r#"
fn main() {
    let dt: f64 = 0.000005;
    let mut sum: f64 = 0.0;
    let mut i = 0i64;
    while i < 1000000 {
        let t: f64 = (i as f64) * dt;
        let s: f64 = t.sin();
        let c: f64 = (t * 2.0).cos();
        let e: f64 = (s * 0.1).exp();
        sum += (s + c + e) * dt;
        i += 1;
    }
    let res = (sum * 100.0) as i64;
    std::process::exit(((res % 256 + 256) % 256) as i32);
}
"#,
            c_code: r#"
#include <math.h>

int main() {
    double dt = 0.000005;
    double sum = 0.0;
    for (long long i = 0; i < 1000000; i++) {
        double t = (double)i * dt;
        double s = sin(t);
        double c = cos(t * 2.0);
        double e = exp(s * 0.1);
        sum += (s + c + e) * dt;
    }
    long long res = (long long)(sum * 100.0);
    return (int)(((res % 256) + 256) % 256);
}
"#,
            node_code: r#"
let dt = 0.000005;
let sum = 0.0;
for (let i = 0; i < 1000000; i++) {
    let t = i * dt;
    let s = Math.sin(t);
    let c = Math.cos(t * 2.0);
    let e = Math.exp(s * 0.1);
    sum += (s + c + e) * dt;
}
let res = Math.floor(sum * 100.0);
process.exit(((res % 256) + 256) % 256);
"#,
            py_code: r#"
import math, sys
dt = 0.000005
s_val = 0.0
for i in range(1000000):
    t = i * dt
    s = math.sin(t)
    c = math.cos(t * 2.0)
    e = math.exp(s * 0.1)
    s_val += (s + c + e) * dt
res = int(s_val * 100.0)
sys.exit(((res % 256) + 256) % 256)
"#,
        },
        // 24. Complex Number Arithmetic (1M ops)
        BenchmarkWorkload {
            name: "Complex Number Arithmetic (1M ops)",
            expected_exit: 44,
            nl_code: r#"
fn main() -> i64 {
    let mut z: [f64; 2] = c_make(1.0, 0.0);
    let factor: [f64; 2] = c_make(0.999999, 0.001);
    let mut sum: f64 = 0.0;
    let mut i: i64 = 0;
    while i < 1000000 {
        z = c_mul(z, factor);
        let theta: f64 = to_float(i % 1000) * 0.0062831853;
        let rot: [f64; 2] = c_exp(c_make(0.0, theta));
        let w: [f64; 2] = c_add(z, rot);
        sum = sum + c_abs(w);
        i = i + 1;
    }
    let res: i64 = to_int(sum);
    return (res % 256 + 256) % 256;
}
"#,
            rs_code: r#"
fn main() {
    let mut z_re = 1.0f64;
    let mut z_im = 0.0f64;
    let factor_re = 0.999999f64;
    let factor_im = 0.001f64;
    let mut sum = 0.0f64;
    let mut i = 0i64;
    while i < 1000000 {
        let new_re = z_re * factor_re - z_im * factor_im;
        let new_im = z_re * factor_im + z_im * factor_re;
        z_re = new_re;
        z_im = new_im;
        let theta = ((i % 1000) as f64) * 0.0062831853;
        let rot_re = theta.cos();
        let rot_im = theta.sin();
        let w_re = z_re + rot_re;
        let w_im = z_im + rot_im;
        sum += (w_re * w_re + w_im * w_im).sqrt();
        i += 1;
    }
    let res = sum as i64;
    std::process::exit(((res % 256 + 256) % 256) as i32);
}
"#,
            c_code: r#"
#include <math.h>

int main() {
    double z_re = 1.0;
    double z_im = 0.0;
    double factor_re = 0.999999;
    double factor_im = 0.001;
    double sum = 0.0;
    for (long long i = 0; i < 1000000; i++) {
        double new_re = z_re * factor_re - z_im * factor_im;
        double new_im = z_re * factor_im + z_im * factor_re;
        z_re = new_re;
        z_im = new_im;
        double theta = (double)(i % 1000) * 0.0062831853;
        double rot_re = cos(theta);
        double rot_im = sin(theta);
        double w_re = z_re + rot_re;
        double w_im = z_im + rot_im;
        sum += sqrt(w_re * w_re + w_im * w_im);
    }
    long long res = (long long)sum;
    return (int)(((res % 256) + 256) % 256);
}
"#,
            node_code: r#"
let z_re = 1.0;
let z_im = 0.0;
let factor_re = 0.999999;
let factor_im = 0.001;
let sum = 0.0;
for (let i = 0; i < 1000000; i++) {
    let new_re = z_re * factor_re - z_im * factor_im;
    let new_im = z_re * factor_im + z_im * factor_re;
    z_re = new_re;
    z_im = new_im;
    let theta = (i % 1000) * 0.0062831853;
    let rot_re = Math.cos(theta);
    let rot_im = Math.sin(theta);
    let w_re = z_re + rot_re;
    let w_im = z_im + rot_im;
    sum += Math.sqrt(w_re * w_re + w_im * w_im);
}
let res = Math.floor(sum);
process.exit(((res % 256) + 256) % 256);
"#,
            py_code: r#"
import math, sys
z_re = 1.0
z_im = 0.0
factor_re = 0.999999
factor_im = 0.001
s_val = 0.0
for i in range(1000000):
    new_re = z_re * factor_re - z_im * factor_im
    new_im = z_re * factor_im + z_im * factor_re
    z_re = new_re
    z_im = new_im
    theta = (i % 1000) * 0.0062831853
    rot_re = math.cos(theta)
    rot_im = math.sin(theta)
    w_re = z_re + rot_re
    w_im = z_im + rot_im
    s_val += math.sqrt(w_re * w_re + w_im * w_im)
res = int(s_val)
sys.exit(((res % 256) + 256) % 256)
"#,
        },
        // 25. 4x4 Linear System Solver (500k ops)
        BenchmarkWorkload {
            name: "4x4 Linear System Solver (500k ops)",
            expected_exit: 37,
            nl_code: r#"
fn main() -> i64 {
    let a: [f64; 16] = [
        5.0, 1.0, 0.0, 2.0,
        1.0, 6.0, 2.0, 0.0,
        0.0, 2.0, 7.0, 1.0,
        2.0, 0.0, 1.0, 8.0
    ];
    let mut i: i64 = 0;
    let mut sum: f64 = 0.0;
    while i < 500000 {
        let b0: f64 = to_float(i % 10);
        let b1: f64 = to_float((i + 1) % 10);
        let b2: f64 = to_float((i + 2) % 10);
        let b3: f64 = to_float((i + 3) % 10);
        let b: [f64; 4] = [b0, b1, b2, b3];
        let x: [f64; 4] = mat_solve4(a, b);
        sum = sum + x[0] + x[1] + x[2] + x[3];
        i = i + 1;
    }
    let res: i64 = to_int(sum);
    return (res % 256 + 256) % 256;
}
"#,
            rs_code: r#"
fn det3(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64, g: f64, h: f64, i: f64) -> f64 {
    a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)
}

fn inv4(m: &[f64; 16]) -> [f64; 16] {
    let c00 = det3(m[5], m[6], m[7], m[9], m[10], m[11], m[13], m[14], m[15]);
    let c01 = -det3(m[4], m[6], m[7], m[8], m[10], m[11], m[12], m[14], m[15]);
    let c02 = det3(m[4], m[5], m[7], m[8], m[9], m[11], m[12], m[13], m[15]);
    let c03 = -det3(m[4], m[5], m[6], m[8], m[9], m[10], m[12], m[13], m[14]);

    let c10 = -det3(m[1], m[2], m[3], m[9], m[10], m[11], m[13], m[14], m[15]);
    let c11 = det3(m[0], m[2], m[3], m[8], m[10], m[11], m[12], m[14], m[15]);
    let c12 = -det3(m[0], m[1], m[3], m[8], m[9], m[11], m[12], m[13], m[15]);
    let c13 = det3(m[0], m[1], m[2], m[8], m[9], m[10], m[12], m[13], m[14]);

    let c20 = det3(m[1], m[2], m[3], m[5], m[6], m[7], m[13], m[14], m[15]);
    let c21 = -det3(m[0], m[2], m[3], m[4], m[6], m[7], m[12], m[14], m[15]);
    let c22 = det3(m[0], m[1], m[3], m[4], m[5], m[7], m[12], m[13], m[15]);
    let c23 = -det3(m[0], m[1], m[2], m[4], m[5], m[6], m[12], m[13], m[14]);

    let c30 = -det3(m[1], m[2], m[3], m[5], m[6], m[7], m[9], m[10], m[11]);
    let c31 = det3(m[0], m[2], m[3], m[4], m[6], m[7], m[8], m[10], m[11]);
    let c32 = -det3(m[0], m[1], m[3], m[4], m[5], m[7], m[8], m[9], m[11]);
    let c33 = det3(m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]);

    let det = m[0] * c00 + m[1] * c01 + m[2] * c02 + m[3] * c03;
    let inv_det = 1.0 / det;

    [
        c00 * inv_det, c10 * inv_det, c20 * inv_det, c30 * inv_det,
        c01 * inv_det, c11 * inv_det, c21 * inv_det, c31 * inv_det,
        c02 * inv_det, c12 * inv_det, c22 * inv_det, c32 * inv_det,
        c03 * inv_det, c13 * inv_det, c23 * inv_det, c33 * inv_det,
    ]
}

fn solve4(inv: &[f64; 16], b: &[f64; 4]) -> [f64; 4] {
    [
        inv[0] * b[0] + inv[1] * b[1] + inv[2] * b[2] + inv[3] * b[3],
        inv[4] * b[0] + inv[5] * b[1] + inv[6] * b[2] + inv[7] * b[3],
        inv[8] * b[0] + inv[9] * b[1] + inv[10] * b[2] + inv[11] * b[3],
        inv[12] * b[0] + inv[13] * b[1] + inv[14] * b[2] + inv[15] * b[3],
    ]
}

fn main() {
    let a: [f64; 16] = [
        5.0, 1.0, 0.0, 2.0,
        1.0, 6.0, 2.0, 0.0,
        0.0, 2.0, 7.0, 1.0,
        2.0, 0.0, 1.0, 8.0,
    ];
    let inv = inv4(&a);
    let mut i = 0i64;
    let mut sum = 0.0f64;
    while i < 500000 {
        let b = [
            (i % 10) as f64,
            ((i + 1) % 10) as f64,
            ((i + 2) % 10) as f64,
            ((i + 3) % 10) as f64,
        ];
        let x = solve4(&inv, &b);
        sum += x[0] + x[1] + x[2] + x[3];
        i += 1;
    }
    let res = sum as i64;
    std::process::exit(((res % 256 + 256) % 256) as i32);
}
"#,
            c_code: r#"
static inline double det3(double a, double b, double c, double d, double e, double f, double g, double h, double i) {
    return a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
}

static inline void inv4(const double* m, double* out) {
    double c00 = det3(m[5], m[6], m[7], m[9], m[10], m[11], m[13], m[14], m[15]);
    double c01 = -det3(m[4], m[6], m[7], m[8], m[10], m[11], m[12], m[14], m[15]);
    double c02 = det3(m[4], m[5], m[7], m[8], m[9], m[11], m[12], m[13], m[15]);
    double c03 = -det3(m[4], m[5], m[6], m[8], m[9], m[10], m[12], m[13], m[14]);

    double c10 = -det3(m[1], m[2], m[3], m[9], m[10], m[11], m[13], m[14], m[15]);
    double c11 = det3(m[0], m[2], m[3], m[8], m[10], m[11], m[12], m[14], m[15]);
    double c12 = -det3(m[0], m[1], m[3], m[8], m[9], m[11], m[12], m[13], m[15]);
    double c13 = det3(m[0], m[1], m[2], m[8], m[9], m[10], m[12], m[13], m[14]);

    double c20 = det3(m[1], m[2], m[3], m[5], m[6], m[7], m[13], m[14], m[15]);
    double c21 = -det3(m[0], m[2], m[3], m[4], m[6], m[7], m[12], m[14], m[15]);
    double c22 = det3(m[0], m[1], m[3], m[4], m[5], m[7], m[12], m[13], m[15]);
    double c23 = -det3(m[0], m[1], m[2], m[4], m[5], m[6], m[12], m[13], m[14]);

    double c30 = -det3(m[1], m[2], m[3], m[5], m[6], m[7], m[9], m[10], m[11]);
    double c31 = det3(m[0], m[2], m[3], m[4], m[6], m[7], m[8], m[10], m[11]);
    double c32 = -det3(m[0], m[1], m[3], m[4], m[5], m[7], m[8], m[9], m[11]);
    double c33 = det3(m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]);

    double det = m[0] * c00 + m[1] * c01 + m[2] * c02 + m[3] * c03;
    double inv_det = 1.0 / det;

    out[0] = c00 * inv_det; out[1] = c10 * inv_det; out[2] = c20 * inv_det; out[3] = c30 * inv_det;
    out[4] = c01 * inv_det; out[5] = c11 * inv_det; out[6] = c21 * inv_det; out[7] = c31 * inv_det;
    out[8] = c02 * inv_det; out[9] = c12 * inv_det; out[10] = c22 * inv_det; out[11] = c32 * inv_det;
    out[12] = c03 * inv_det; out[13] = c13 * inv_det; out[14] = c23 * inv_det; out[15] = c33 * inv_det;
}

static inline void solve4(const double* inv, const double* b, double* x) {
    x[0] = inv[0] * b[0] + inv[1] * b[1] + inv[2] * b[2] + inv[3] * b[3];
    x[1] = inv[4] * b[0] + inv[5] * b[1] + inv[6] * b[2] + inv[7] * b[3];
    x[2] = inv[8] * b[0] + inv[9] * b[1] + inv[10] * b[2] + inv[11] * b[3];
    x[3] = inv[12] * b[0] + inv[13] * b[1] + inv[14] * b[2] + inv[15] * b[3];
}

int main() {
    double a[16] = {
        5.0, 1.0, 0.0, 2.0,
        1.0, 6.0, 2.0, 0.0,
        0.0, 2.0, 7.0, 1.0,
        2.0, 0.0, 1.0, 8.0
    };
    double inv[16];
    inv4(a, inv);
    double sum = 0.0;
    for (long long i = 0; i < 500000; i++) {
        double b[4] = {
            (double)(i % 10),
            (double)((i + 1) % 10),
            (double)((i + 2) % 10),
            (double)((i + 3) % 10)
        };
        double x[4];
        solve4(inv, b, x);
        sum += x[0] + x[1] + x[2] + x[3];
    }
    long long res = (long long)sum;
    return (int)(((res % 256) + 256) % 256);
}
"#,
            node_code: r#"
function det3(a, b, c, d, e, f, g, h, i) {
    return a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
}
function inv4(m) {
    let c00 = det3(m[5], m[6], m[7], m[9], m[10], m[11], m[13], m[14], m[15]);
    let c01 = -det3(m[4], m[6], m[7], m[8], m[10], m[11], m[12], m[14], m[15]);
    let c02 = det3(m[4], m[5], m[7], m[8], m[9], m[11], m[12], m[13], m[15]);
    let c03 = -det3(m[4], m[5], m[6], m[8], m[9], m[10], m[12], m[13], m[14]);

    let c10 = -det3(m[1], m[2], m[3], m[9], m[10], m[11], m[13], m[14], m[15]);
    let c11 = det3(m[0], m[2], m[3], m[8], m[10], m[11], m[12], m[14], m[15]);
    let c12 = -det3(m[0], m[1], m[3], m[8], m[9], m[11], m[12], m[13], m[15]);
    let c13 = det3(m[0], m[1], m[2], m[8], m[9], m[10], m[12], m[13], m[14]);

    let c20 = det3(m[1], m[2], m[3], m[5], m[6], m[7], m[13], m[14], m[15]);
    let c21 = -det3(m[0], m[2], m[3], m[4], m[6], m[7], m[12], m[14], m[15]);
    let c22 = det3(m[0], m[1], m[3], m[4], m[5], m[7], m[12], m[13], m[15]);
    let c23 = -det3(m[0], m[1], m[2], m[4], m[5], m[6], m[12], m[13], m[14]);

    let c30 = -det3(m[1], m[2], m[3], m[5], m[6], m[7], m[9], m[10], m[11]);
    let c31 = det3(m[0], m[2], m[3], m[4], m[6], m[7], m[8], m[10], m[11]);
    let c32 = -det3(m[0], m[1], m[3], m[4], m[5], m[7], m[8], m[9], m[11]);
    let c33 = det3(m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]);

    let det = m[0] * c00 + m[1] * c01 + m[2] * c02 + m[3] * c03;
    let inv_det = 1.0 / det;

    return [
        c00 * inv_det, c10 * inv_det, c20 * inv_det, c30 * inv_det,
        c01 * inv_det, c11 * inv_det, c21 * inv_det, c31 * inv_det,
        c02 * inv_det, c12 * inv_det, c22 * inv_det, c32 * inv_det,
        c03 * inv_det, c13 * inv_det, c23 * inv_det, c33 * inv_det,
    ];
}
function solve4(inv, b) {
    return [
        inv[0] * b[0] + inv[1] * b[1] + inv[2] * b[2] + inv[3] * b[3],
        inv[4] * b[0] + inv[5] * b[1] + inv[6] * b[2] + inv[7] * b[3],
        inv[8] * b[0] + inv[9] * b[1] + inv[10] * b[2] + inv[11] * b[3],
        inv[12] * b[0] + inv[13] * b[1] + inv[14] * b[2] + inv[15] * b[3]
    ];
}

let a = [
    5.0, 1.0, 0.0, 2.0,
    1.0, 6.0, 2.0, 0.0,
    0.0, 2.0, 7.0, 1.0,
    2.0, 0.0, 1.0, 8.0
];
let inv = inv4(a);
let sum = 0.0;
for (let i = 0; i < 500000; i++) {
    let b = [i % 10, (i + 1) % 10, (i + 2) % 10, (i + 3) % 10];
    let x = solve4(inv, b);
    sum += x[0] + x[1] + x[2] + x[3];
}
let res = Math.floor(sum);
process.exit(((res % 256) + 256) % 256);
"#,
            py_code: r#"
import sys

def det3(a, b, c, d, e, f, g, h, i):
    return a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)

def inv4(m):
    c00 = det3(m[5], m[6], m[7], m[9], m[10], m[11], m[13], m[14], m[15])
    c01 = -det3(m[4], m[6], m[7], m[8], m[10], m[11], m[12], m[14], m[15])
    c02 = det3(m[4], m[5], m[7], m[8], m[9], m[11], m[12], m[13], m[15])
    c03 = -det3(m[4], m[5], m[6], m[8], m[9], m[10], m[12], m[13], m[14])

    c10 = -det3(m[1], m[2], m[3], m[9], m[10], m[11], m[13], m[14], m[15])
    c11 = det3(m[0], m[2], m[3], m[8], m[10], m[11], m[12], m[14], m[15])
    c12 = -det3(m[0], m[1], m[3], m[8], m[9], m[11], m[12], m[13], m[15])
    c13 = det3(m[0], m[1], m[2], m[8], m[9], m[10], m[12], m[13], m[14])

    c20 = det3(m[1], m[2], m[3], m[5], m[6], m[7], m[13], m[14], m[15])
    c21 = -det3(m[0], m[2], m[3], m[4], m[6], m[7], m[12], m[14], m[15])
    c22 = det3(m[0], m[1], m[3], m[4], m[5], m[7], m[12], m[13], m[15])
    c23 = -det3(m[0], m[1], m[2], m[4], m[5], m[6], m[12], m[13], m[14])

    c30 = -det3(m[1], m[2], m[3], m[5], m[6], m[7], m[9], m[10], m[11])
    c31 = det3(m[0], m[2], m[3], m[4], m[6], m[7], m[8], m[10], m[11])
    c32 = -det3(m[0], m[1], m[3], m[4], m[5], m[7], m[8], m[9], m[11])
    c33 = det3(m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10])

    det = m[0] * c00 + m[1] * c01 + m[2] * c02 + m[3] * c03
    inv_det = 1.0 / det

    return [
        c00 * inv_det, c10 * inv_det, c20 * inv_det, c30 * inv_det,
        c01 * inv_det, c11 * inv_det, c21 * inv_det, c31 * inv_det,
        c02 * inv_det, c12 * inv_det, c22 * inv_det, c32 * inv_det,
        c03 * inv_det, c13 * inv_det, c23 * inv_det, c33 * inv_det,
    ]

def solve4(inv, b):
    return [
        inv[0] * b[0] + inv[1] * b[1] + inv[2] * b[2] + inv[3] * b[3],
        inv[4] * b[0] + inv[5] * b[1] + inv[6] * b[2] + inv[7] * b[3],
        inv[8] * b[0] + inv[9] * b[1] + inv[10] * b[2] + inv[11] * b[3],
        inv[12] * b[0] + inv[13] * b[1] + inv[14] * b[2] + inv[15] * b[3]
    ]

a = [
    5.0, 1.0, 0.0, 2.0,
    1.0, 6.0, 2.0, 0.0,
    0.0, 2.0, 7.0, 1.0,
    2.0, 0.0, 1.0, 8.0
]
inv = inv4(a)
sum_val = 0.0
for i in range(500000):
    b = [i % 10, (i + 1) % 10, (i + 2) % 10, (i + 3) % 10]
    x = solve4(inv, b)
    sum_val += x[0] + x[1] + x[2] + x[3]
res = int(sum_val)
sys.exit(((res % 256) + 256) % 256)
"#,
        },
    ];

    let fmt_duration = |d: Duration| -> String {
        let nanos = d.as_nanos();
        if nanos < 1_000 {
            format!("{} ns", nanos)
        } else if nanos < 1_000_000 {
            format!("{:.2} µs", nanos as f64 / 1_000.0)
        } else if nanos < 1_000_000_000 {
            format!("{:.2} ms", nanos as f64 / 1_000_000.0)
        } else {
            format!("{:.2} s", nanos as f64 / 1_000_000_000.0)
        }
    };

    let fmt_speedup = |ratio: f64| -> String {
        if (0.95..=1.05).contains(&ratio) {
            "1.00x".to_string()
        } else if ratio < 1000.0 {
            format!("{:.2}x", ratio)
        } else if ratio < 1_000_000.0 {
            format!("{:.0}x", ratio)
        } else if ratio < 1_000_000_000.0 {
            format!("{:.2}M x", ratio / 1_000_000.0)
        } else {
            format!("{:.2}B x", ratio / 1_000_000_000.0)
        }
    };

    println!("\n==================================================================================================================================");
    println!("                                   NUMLANG MULTI-LANGUAGE COMPARATIVE BENCHMARK SUITE                                            ");
    println!("==================================================================================================================================");
    println!("{:<36} | {:<12} | {:>11} | {:>11} | {:>10} | {:>12} | {:<6}", "Benchmark", "Language", "Compute Min", "Compute Avg", "Wall Time", "vs numlang", "Status");
    println!("----------------------------------------------------------------------------------------------------------------------------------");

    for w in workloads {
        let slug = w.name.to_lowercase().replace(' ', "_").replace(['(', ')', ','], "");

        // 1. numlang
        let nl_exe = compile_numlang(w.nl_code, &test_dir, &slug);
        let (nl_comp_min, nl_comp_avg, nl_wall_min, _nl_wall_avg, nl_out, nl_pass) = benchmark_cmd(nl_exe.to_str().unwrap(), &[], w.expected_exit, 3);
        let nl_min_nanos = nl_comp_min.as_nanos().max(1) as f64;
        println!(
            "{:<36} | {:<12} | {:>11} | {:>11} | {:>10} | {:>12} | {:<6}",
            w.name, "numlang", fmt_duration(nl_comp_min), fmt_duration(nl_comp_avg), fmt_duration(nl_wall_min), "1.00x", if nl_pass { "PASS" } else { "FAIL" }
        );
        assert!(nl_pass, "numlang benchmark failed on '{}': expected {}, got {}", w.name, w.expected_exit, nl_out);

        // 2. Rust
        let mut rs_comp_val = Duration::ZERO;
        if let Some(rs_exe) = compile_rust(w.rs_code, &test_dir, &slug) {
            let (rs_comp_min, rs_comp_avg, rs_wall_min, _rs_wall_avg, rs_out, rs_pass) = benchmark_cmd(rs_exe.to_str().unwrap(), &[], w.expected_exit, 3);
            rs_comp_val = rs_comp_min;
            let speedup = rs_comp_min.as_nanos() as f64 / nl_min_nanos;
            println!(
                "{:<36} | {:<12} | {:>11} | {:>11} | {:>10} | {:>12} | {:<6}",
                "", "Rust (-O)", fmt_duration(rs_comp_min), fmt_duration(rs_comp_avg), fmt_duration(rs_wall_min), fmt_speedup(speedup), if rs_pass { "PASS" } else { "FAIL" }
            );
            assert!(rs_pass, "Rust benchmark failed on '{}': expected {}, got {}", w.name, w.expected_exit, rs_out);
        }

        // 3. C
        if let Some(c_exe) = compile_c(w.c_code, &test_dir, &slug) {
            let (c_comp_min, c_comp_avg, c_wall_min, _c_wall_avg, c_out, c_pass) = benchmark_cmd(c_exe.to_str().unwrap(), &[], w.expected_exit, 3);
            let speedup = c_comp_min.as_nanos() as f64 / nl_min_nanos;
            println!(
                "{:<36} | {:<12} | {:>11} | {:>11} | {:>10} | {:>12} | {:<6}",
                "", "C (/O2)", fmt_duration(c_comp_min), fmt_duration(c_comp_avg), fmt_duration(c_wall_min), fmt_speedup(speedup), if c_pass { "PASS" } else { "FAIL" }
            );
            assert!(c_pass, "C benchmark failed on '{}': expected {}, got {}", w.name, w.expected_exit, c_out);
        }

        // 4. Node.js (V8)
        let js_file = test_dir.join(format!("{}.js", slug));
        fs::write(&js_file, wrap_node(w.node_code)).unwrap();
        let (node_comp_min, node_comp_avg, node_wall_min, _node_wall_avg, node_out, node_pass) = benchmark_cmd("node", &[js_file.to_str().unwrap()], w.expected_exit, 3);
        let speedup = node_comp_min.as_nanos() as f64 / nl_min_nanos;
        println!(
            "{:<36} | {:<12} | {:>11} | {:>11} | {:>10} | {:>12} | {:<6}",
            "", "Node.js (V8)", fmt_duration(node_comp_min), fmt_duration(node_comp_avg), fmt_duration(node_wall_min), fmt_speedup(speedup), if node_pass { "PASS" } else { "FAIL" }
        );
        assert!(node_pass, "Node benchmark failed on '{}': expected {}, got {}", w.name, w.expected_exit, node_out);

        // 5. Python 3
        let py_file = test_dir.join(format!("{}.py", slug));
        fs::write(&py_file, wrap_py(w.py_code)).unwrap();
        let py_iters = if w.name.contains("50M") || w.name.contains("Collatz") || w.name.contains("Prime") || w.name.contains("Ackermann") || w.name.contains("N-Queens") || w.name.contains("Mandelbrot") || w.name.contains("Modular") || w.name.contains("Monte Carlo") || w.name.contains("Binary Search") || w.name.contains("Rule 110") || w.name.contains("Matrix") || w.name.contains("Binary GCD") || w.name.contains("Cosine") || w.name.contains("Integer Sqrt") || w.name.contains("16-point FFT") || w.name.contains("Transcendental") || w.name.contains("Complex") || w.name.contains("Linear System") { 1 } else { 2 };
        let (py_comp_min, py_comp_avg, py_wall_min, _py_wall_avg, py_out, py_pass) = benchmark_cmd("python", &[py_file.to_str().unwrap()], w.expected_exit, py_iters);
        let speedup = py_comp_min.as_nanos() as f64 / nl_min_nanos;
        println!(
            "{:<36} | {:<12} | {:>11} | {:>11} | {:>10} | {:>12} | {:<6}",
            "", "Python 3.14", fmt_duration(py_comp_min), fmt_duration(py_comp_avg), fmt_duration(py_wall_min), fmt_speedup(speedup), if py_pass { "PASS" } else { "FAIL" }
        );
        assert!(py_pass, "Python benchmark failed on '{}': expected {}, got {}", w.name, w.expected_exit, py_out);

        if rs_comp_val.as_nanos() > 0 {
            println!("  [+] In-Process Compute Advantage vs Rust: >{} speedup ({} vs {})",
                fmt_speedup(rs_comp_val.as_nanos() as f64 / nl_min_nanos),
                fmt_duration(rs_comp_val),
                fmt_duration(nl_comp_min)
            );
        }

        println!("----------------------------------------------------------------------------------------------------------------------------------");
    }

    println!("==================================================================================================================================\n");
}
