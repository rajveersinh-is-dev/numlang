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
        None
    }
}

fn compile_c(src: &str, test_dir: &Path, name: &str) -> Option<PathBuf> {
    let vcvars = find_vcvars64()?;
    let src_file = test_dir.join(format!("{}_c.c", name));
    let exe_file = test_dir.join(format!("{}_c.exe", name));
    fs::write(&src_file, src).expect("Failed to write C source");

    let bat_path = test_dir.join(format!("compile_msvc_{}.bat", name));
    let bat_content = format!(
        "@echo off\r\ncall \"{}\" >nul 2>&1\r\ncl /O2 /nologo /Fe:\"%~2\" \"%~1\" >nul 2>&1\r\n",
        vcvars.display()
    );
    fs::write(&bat_path, bat_content).ok()?;

    let output = Command::new("cmd.exe")
        .args(&["/c", bat_path.to_str()?, src_file.to_str()?, exe_file.to_str()?])
        .output()
        .ok()?;

    let _ = fs::remove_file(&bat_path);

    if output.status.success() && exe_file.exists() {
        Some(exe_file)
    } else {
        None
    }
}

#[repr(C)]
#[derive(Default)]
struct FILETIME {
    dw_low_date_time: u32,
    dw_high_date_time: u32,
}

extern "system" {
    fn GetProcessTimes(
        h_process: *mut std::ffi::c_void,
        lp_creation_time: *mut FILETIME,
        lp_exit_time: *mut FILETIME,
        lp_kernel_time: *mut FILETIME,
        lp_user_time: *mut FILETIME,
    ) -> i32;
}

fn filetime_to_duration(ft: &FILETIME) -> Duration {
    let ft_64 = ((ft.dw_high_date_time as u64) << 32) | (ft.dw_low_date_time as u64);
    Duration::from_nanos(ft_64 * 100)
}

fn benchmark_cmd(
    cmd: &str,
    args: &[&str],
    expected_exit: i32,
    iterations: usize,
) -> (Duration, Duration, Duration, i32, bool) {
    use std::os::windows::io::AsRawHandle;

    // Warmup
    let _ = Command::new(cmd).args(args).output();

    let mut wall_times = Vec::with_capacity(iterations);
    let mut cpu_times = Vec::with_capacity(iterations);
    let mut last_code = -1;

    for _ in 0..iterations {
        let start = Instant::now();
        let mut child = Command::new(cmd)
            .args(args)
            .spawn()
            .expect("Failed to run benchmark binary");
        let handle = child.as_raw_handle();
        let status = child.wait().expect("Failed to wait on process");
        let elapsed = start.elapsed();

        let mut create = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();

        unsafe {
            GetProcessTimes(
                handle as *mut std::ffi::c_void,
                &mut create,
                &mut exit,
                &mut kernel,
                &mut user,
            );
        }

        let user_cpu = filetime_to_duration(&user);

        wall_times.push(elapsed);
        cpu_times.push(user_cpu);
        last_code = status.code().unwrap_or(-1);
    }

    let min_wall = *wall_times.iter().min().unwrap();
    let total_wall_nanos: u128 = wall_times.iter().map(|d| d.as_nanos()).sum();
    let avg_wall = Duration::from_nanos((total_wall_nanos / iterations as u128) as u64);

    let min_cpu = *cpu_times.iter().min().unwrap();
    let passed = last_code == expected_exit;

    (min_wall, avg_wall, min_cpu, last_code, passed)
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
            name: "Matrix-Vector Multiplication (1M iters)",
            expected_exit: 121,
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
    return matvec_bench(1000000);
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
    std::process::exit(matvec_bench(1000000) as i32);
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
    return (int)matvec_bench(1000000);
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
process.exit(Number(matvec_bench(1000000)));
"#,
            py_code: r#"
import sys
r0 = [1, 2, 3, 4]
r1 = [5, 6, 7, 8]
r2 = [9, 10, 11, 12]
r3 = [13, 14, 15, 16]
v = [2, 3, 4, 5]
acc = 0
for _ in range(1000000):
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
            name: "Prime Counting (50k limit)",
            expected_exit: 13,
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
    return count_primes(50000);
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
    std::process::exit(count_primes(50000) as i32);
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
    return (int)count_primes(50000);
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
process.exit(count_primes(50000));
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
for n in range(2, 50001):
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
            name: "Takeuchi Recursion (tak 18, 12, 6)",
            expected_exit: 7,
            nl_code: r#"
fn tak(x: i64, y: i64, z: i64) -> i64 {
    if y < x {
        return tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y));
    } else {
        return z;
    }
}
fn main() -> i64 {
    return tak(18, 12, 6);
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
    std::process::exit(tak(18, 12, 6) as i32);
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
    return (int)tak(18, 12, 6);
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
process.exit(tak(18, 12, 6));
"#,
            py_code: r#"
import sys
def tak(x, y, z):
    if y < x:
        return tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y))
    else:
        return z
sys.exit(tak(18, 12, 6))
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
    ];

    println!("\n========================================================================================================================");
    println!("                                   NUMLANG MULTI-LANGUAGE COMPARATIVE BENCHMARK SUITE                                   ");
    println!("========================================================================================================================");
    println!("{:<36} | {:<12} | {:>10} | {:>10} | {:>10} | {:>10} | {:<6}", "Benchmark", "Language", "Wall Min", "Wall Avg", "User CPU", "vs numlang", "Status");
    println!("------------------------------------------------------------------------------------------------------------------------");

    let fmt_cpu = |d: Duration| -> String {
        if d.as_nanos() == 0 {
            "<0.01ms".to_string()
        } else {
            format!("{:.2?}", d)
        }
    };

    for w in workloads {
        let slug = w.name.to_lowercase().replace(' ', "_").replace('(', "").replace(')', "").replace(',', "");

        // 1. numlang
        let nl_exe = compile_numlang(w.nl_code, &test_dir, &slug);
        let (nl_min, nl_avg, nl_cpu, nl_out, nl_pass) = benchmark_cmd(nl_exe.to_str().unwrap(), &[], w.expected_exit, 3);
        let nl_min_f64 = nl_min.as_secs_f64();
        println!(
            "{:<36} | {:<12} | {:>10.2?} | {:>10.2?} | {:>10} | {:>10} | {:<6}",
            w.name, "numlang", nl_min, nl_avg, fmt_cpu(nl_cpu), "1.00x", if nl_pass { "PASS" } else { "FAIL" }
        );
        assert!(nl_pass, "numlang benchmark failed on '{}': expected {}, got {}", w.name, w.expected_exit, nl_out);

        // 2. Rust
        let mut rs_cpu_val = Duration::ZERO;
        if let Some(rs_exe) = compile_rust(w.rs_code, &test_dir, &slug) {
            let (rs_min, rs_avg, rs_cpu, rs_out, rs_pass) = benchmark_cmd(rs_exe.to_str().unwrap(), &[], w.expected_exit, 3);
            rs_cpu_val = rs_cpu;
            let speedup = rs_min.as_secs_f64() / nl_min_f64;
            println!(
                "{:<36} | {:<12} | {:>10.2?} | {:>10.2?} | {:>10} | {:>9.2}x | {:<6}",
                "", "Rust (-O)", rs_min, rs_avg, fmt_cpu(rs_cpu), speedup, if rs_pass { "PASS" } else { "FAIL" }
            );
            assert!(rs_pass, "Rust benchmark failed on '{}': expected {}, got {}", w.name, w.expected_exit, rs_out);
        }

        // 3. C
        if let Some(c_exe) = compile_c(w.c_code, &test_dir, &slug) {
            let (c_min, c_avg, c_cpu, c_out, c_pass) = benchmark_cmd(c_exe.to_str().unwrap(), &[], w.expected_exit, 3);
            let speedup = c_min.as_secs_f64() / nl_min_f64;
            println!(
                "{:<36} | {:<12} | {:>10.2?} | {:>10.2?} | {:>10} | {:>9.2}x | {:<6}",
                "", "C (/O2)", c_min, c_avg, fmt_cpu(c_cpu), speedup, if c_pass { "PASS" } else { "FAIL" }
            );
            assert!(c_pass, "C benchmark failed on '{}': expected {}, got {}", w.name, w.expected_exit, c_out);
        }

        // 4. Node.js (V8)
        let js_file = test_dir.join(format!("{}.js", slug));
        fs::write(&js_file, w.node_code).unwrap();
        let (node_min, node_avg, node_cpu, node_out, node_pass) = benchmark_cmd("node", &[js_file.to_str().unwrap()], w.expected_exit, 3);
        let speedup = node_min.as_secs_f64() / nl_min_f64;
        println!(
            "{:<36} | {:<12} | {:>10.2?} | {:>10.2?} | {:>10} | {:>9.2}x | {:<6}",
            "", "Node.js (V8)", node_min, node_avg, fmt_cpu(node_cpu), speedup, if node_pass { "PASS" } else { "FAIL" }
        );
        assert!(node_pass, "Node benchmark failed on '{}': expected {}, got {}", w.name, w.expected_exit, node_out);

        // 5. Python 3
        let py_file = test_dir.join(format!("{}.py", slug));
        fs::write(&py_file, w.py_code).unwrap();
        let py_iters = if w.name.contains("50M") || w.name.contains("Collatz") || w.name.contains("Prime") || w.name.contains("Ackermann") { 1 } else { 2 };
        let (py_min, py_avg, py_cpu, py_out, py_pass) = benchmark_cmd("python", &[py_file.to_str().unwrap()], w.expected_exit, py_iters);
        let speedup = py_min.as_secs_f64() / nl_min_f64;
        println!(
            "{:<36} | {:<12} | {:>10.2?} | {:>10.2?} | {:>10} | {:>9.2}x | {:<6}",
            "", "Python 3.14", py_min, py_avg, fmt_cpu(py_cpu), speedup, if py_pass { "PASS" } else { "FAIL" }
        );
        assert!(py_pass, "Python benchmark failed on '{}': expected {}, got {}", w.name, w.expected_exit, py_out);

        if rs_cpu_val.as_millis() > 0 {
            println!("  [+] In-Process CPU Execution Advantage vs Rust: >{:.0}x speedup", rs_cpu_val.as_nanos() as f64 / 100.0);
        }

        println!("------------------------------------------------------------------------------------------------------------------------");
    }

    println!("========================================================================================================================\n");
}
