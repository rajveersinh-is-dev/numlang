import subprocess
import os
import sys
import time

vcvars = r"C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat"

benchmarks = [
    {
        "name": "Nonlinear Arithmetic Progression with Warmup (25M iters)",
        "nl": r"""
fn main() -> i64 {
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < 25000000 {
        let diff: i64 = 7 * i - 31;
        let val: i64 = abs(diff);
        sum = (sum + val) % 1000000007;
        i = i + 1;
    }
    return sum % 256;
}
""",
        "rs": r"""
fn abs(x: i64) -> i64 { x.abs() }
fn main() {
    let t0 = std::time::Instant::now();
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < 25000000 {
        let diff: i64 = 7 * i - 31;
        let val: i64 = abs(diff);
        sum = (sum + val) % 1000000007;
        i += 1;
    }
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit((sum % 256) as i32);
}
""",
        "c": r"""
#include <stdio.h>
#include <stdlib.h>
#include <windows.h>

long long llabs_custom(long long x) { return x < 0 ? -x : x; }

int main() {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    long long sum = 0;
    for (long long i = 0; i < 25000000; i++) {
        long long diff = 7 * i - 31;
        long long val = llabs_custom(diff);
        sum = (sum + val) % 1000000007;
    }

    QueryPerformanceCounter(&t1);
    double ns = (double)(t1.QuadPart - t0.QuadPart) * 1000000000.0 / (double)freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}
"""
    },
    {
        "name": "5-State Cyclic Feedforward Automaton (15M iters)",
        "nl": r"""
fn main() -> i64 {
    let mut a: i64 = 3;
    let mut b: i64 = 7;
    let mut c: i64 = 11;
    let mut d: i64 = 19;
    let mut e: i64 = 23;
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < 15000000 {
        let t: i64 = (a ^ b) + (c & d) - e;
        acc = (acc + t) % 1000000007;
        let next_a: i64 = b;
        let next_b: i64 = c;
        let next_c: i64 = d;
        let next_d: i64 = e;
        let next_e: i64 = a;
        a = next_a;
        b = next_b;
        c = next_c;
        d = next_d;
        e = next_e;
        i = i + 1;
    }
    return (acc % 256 + 256) % 256;
}
""",
        "rs": r"""
fn main() {
    let t0 = std::time::Instant::now();
    let mut a: i64 = 3;
    let mut b: i64 = 7;
    let mut c: i64 = 11;
    let mut d: i64 = 19;
    let mut e: i64 = 23;
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < 15000000 {
        let t: i64 = (a ^ b) + (c & d) - e;
        acc = (acc + t) % 1000000007;
        let next_a: i64 = b;
        let next_b: i64 = c;
        let next_c: i64 = d;
        let next_d: i64 = e;
        let next_e: i64 = a;
        a = next_a;
        b = next_b;
        c = next_c;
        d = next_d;
        e = next_e;
        i += 1;
    }
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit(((acc % 256 + 256) % 256) as i32);
}
""",
        "c": r"""
#include <stdio.h>
#include <stdlib.h>
#include <windows.h>

int main() {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    long long a = 3, b = 7, c = 11, d = 19, e = 23;
    long long acc = 0;
    for (long long i = 0; i < 15000000; i++) {
        long long t = (a ^ b) + (c & d) - e;
        acc = (acc + t) % 1000000007;
        long long next_a = b;
        long long next_b = c;
        long long next_c = d;
        long long next_d = e;
        long long next_e = a;
        a = next_a; b = next_b; c = next_c; d = next_d; e = next_e;
    }

    QueryPerformanceCounter(&t1);
    double ns = (double)(t1.QuadPart - t0.QuadPart) * 1000000000.0 / (double)freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(((acc % 256) + 256) % 256);
}
"""
    },
    {
        "name": "Geometric Power of Three Scaling (28 iterations)",
        "nl": r"""
fn main() -> i64 {
    let mut x: i64 = 7;
    let mut i: i64 = 0;
    while i < 28 {
        x = x * 3;
        i = i + 1;
    }
    return (x % 256 + 256) % 256;
}
""",
        "rs": r"""
fn main() {
    let t0 = std::time::Instant::now();
    let mut x: i64 = 7;
    let mut i: i64 = 0;
    while i < 28 {
        x = x.wrapping_mul(3);
        i += 1;
    }
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit(((x % 256 + 256) % 256) as i32);
}
""",
        "c": r"""
#include <stdio.h>
#include <stdlib.h>
#include <windows.h>

int main() {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    long long x = 7;
    for (long long i = 0; i < 28; i++) {
        x = x * 3;
    }

    QueryPerformanceCounter(&t1);
    double ns = (double)(t1.QuadPart - t0.QuadPart) * 1000000000.0 / (double)freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(((x % 256) + 256) % 256);
}
"""
    },
    {
        "name": "Discrete Step-Function Leapfrogging (40M iters)",
        "nl": r"""
fn main() -> i64 {
    let iters: i64 = 40000000;
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let q: i64 = (i * 500) / iters;
        let denom: i64 = 500000 + q * q;
        let term: i64 = 987654321000 / denom;
        sum = (sum + term) % 1000000007;
        i = i + 1;
    }
    return sum % 256;
}
""",
        "rs": r"""
fn main() {
    let t0 = std::time::Instant::now();
    let iters: i64 = 40000000;
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < iters {
        let q: i64 = (i * 500) / iters;
        let denom: i64 = 500000 + q * q;
        let term: i64 = 987654321000 / denom;
        sum = (sum + term) % 1000000007;
        i += 1;
    }
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit((sum % 256) as i32);
}
""",
        "c": r"""
#include <stdio.h>
#include <stdlib.h>
#include <windows.h>

int main() {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    long long iters = 40000000;
    long long sum = 0;
    for (long long i = 0; i < iters; i++) {
        long long q = (i * 500) / iters;
        long long denom = 500000 + q * q;
        long long term = 987654321000LL / denom;
        sum = (sum + term) % 1000000007;
    }

    QueryPerformanceCounter(&t1);
    double ns = (double)(t1.QuadPart - t0.QuadPart) * 1000000000.0 / (double)freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}
"""
    },
    {
        "name": "Collatz Peak Altitude Search (20,000 seeds)",
        "nl": r"""
fn main() -> i64 {
    let mut max_peak: i64 = 0;
    let mut seed: i64 = 1;
    while seed < 20000 {
        let mut n: i64 = seed;
        let mut peak: i64 = seed;
        while n > 1 {
            if n % 2 == 0 {
                n = n / 2;
            } else {
                n = 3 * n + 1;
            }
            if n > peak {
                peak = n;
            }
        }
        if peak > max_peak {
            max_peak = peak;
        }
        seed = seed + 1;
    }
    return max_peak % 256;
}
""",
        "rs": r"""
fn main() {
    let t0 = std::time::Instant::now();
    let mut max_peak: i64 = 0;
    let mut seed: i64 = 1;
    while seed < 20000 {
        let mut n: i64 = seed;
        let mut peak: i64 = seed;
        while n > 1 {
            if n % 2 == 0 {
                n = n / 2;
            } else {
                n = 3 * n + 1;
            }
            if n > peak {
                peak = n;
            }
        }
        if peak > max_peak {
            max_peak = peak;
        }
        seed += 1;
    }
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit((max_peak % 256) as i32);
}
""",
        "c": r"""
#include <stdio.h>
#include <stdlib.h>
#include <windows.h>

int main() {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    long long max_peak = 0;
    for (long long seed = 1; seed < 20000; seed++) {
        long long n = seed;
        long long peak = seed;
        while (n > 1) {
            if (n % 2 == 0) {
                n = n / 2;
            } else {
                n = 3 * n + 1;
            }
            if (n > peak) {
                peak = n;
            }
        }
        if (peak > max_peak) {
            max_peak = peak;
        }
    }

    QueryPerformanceCounter(&t1);
    double ns = (double)(t1.QuadPart - t0.QuadPart) * 1000000000.0 / (double)freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(max_peak % 256);
}
"""
    }
]

def fmt_time(ns):
    if ns <= 100:
        return "< 100 ns"
    elif ns < 1000:
        return f"{ns} ns"
    elif ns < 1_000_000:
        return f"{ns/1000:.2f} us"
    else:
        return f"{ns/1_000_000:.2f} ms"

print("=" * 115)
print(f"{'#':<3} | {'Novel Benchmark Name':<50} | {'NumLang':<10} | {'Rust (-O)':<10} | {'C (/O2)':<10} | {'Exit Code'}")
print("=" * 115)

for idx, b in enumerate(benchmarks, 1):
    name = b["name"]
    
    # 1. Compile and run NumLang
    with open("temp_novel.nl", "w", encoding="utf-8") as f:
        f.write(b["nl"])
    
    res_comp = subprocess.run(["target/release/numlang.exe", "build", "temp_novel.nl", "--bench", "-o", "temp_novel.exe"], capture_output=True, text=True)
    if res_comp.returncode != 0:
        print(f"NumLang build failed for {name}: {res_comp.stderr}")
        continue
    
    nl_times = []
    nl_code = None
    for _ in range(3):
        p = subprocess.run(["temp_novel.exe"], capture_output=True, text=True)
        nl_code = p.returncode
        for line in p.stdout.splitlines():
            if "COMPUTE_NS:" in line:
                try:
                    nl_times.append(int(line.split(":")[1].strip()))
                except Exception:
                    pass
    nl_ns = min(nl_times) if nl_times else 0

    # 2. Compile and run Rust
    with open("temp_novel.rs", "w", encoding="utf-8") as f:
        f.write(b["rs"])
    subprocess.run(["rustc", "-O", "temp_novel.rs", "-o", "temp_novel_rs.exe"], capture_output=True)
    
    rs_times = []
    rs_code = None
    for _ in range(3):
        p = subprocess.run(["temp_novel_rs.exe"], capture_output=True, text=True)
        rs_code = p.returncode
        for line in p.stdout.splitlines():
            if "COMPUTE_NS:" in line:
                try:
                    rs_times.append(int(line.split(":")[1].strip()))
                except Exception:
                    pass
    rs_ns = min(rs_times) if rs_times else 0

    # 3. Compile and run C
    with open("temp_novel.c", "w", encoding="utf-8") as f:
        f.write(b["c"])
    subprocess.run(f'cmd /c "call "{vcvars}" >nul 2>&1 && cl /O2 /nologo temp_novel.c /Fe:temp_novel_c.exe >nul 2>&1"', shell=True, capture_output=True)
    
    c_times = []
    c_code = None
    for _ in range(3):
        p = subprocess.run(["temp_novel_c.exe"], capture_output=True, text=True)
        c_code = p.returncode
        for line in p.stdout.splitlines():
            if "COMPUTE_NS:" in line:
                try:
                    c_times.append(int(line.split(":")[1].strip()))
                except Exception:
                    pass
    c_ns = min(c_times) if c_times else 0

    match_status = "MATCH" if (nl_code == rs_code == c_code) else f"MISMATCH(nl={nl_code}, rs={rs_code}, c={c_code})"
    print(f"{idx:<3} | {name:<50} | {fmt_time(nl_ns):<10} | {fmt_time(rs_ns):<10} | {fmt_time(c_ns):<10} | {nl_code} ({match_status})")

print("=" * 115)
