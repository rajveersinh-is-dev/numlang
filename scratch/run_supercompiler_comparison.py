import subprocess
import os
import sys
import time

vcvars = r"C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
nl_bin = r"C:\Users\davea\.gemini\antigravity\scratch\numlang\target\release\numlang.exe"
tmp_dir = r"C:\Users\davea\.gemini\antigravity\scratch\supercompiler_bench"

os.makedirs(tmp_dir, exist_ok=True)

benchmarks = [
    {
        "id": 1,
        "name": "Nonlinear Arithmetic Progression (25M iters)",
        "category": "Algebraic Supercompilation (Degree-2 Closed Form)",
        "canonical_ref": "Turchin (1972) / LLVM SCEV",
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

long long llabs_c(long long x) { return x < 0 ? -x : x; }

int main() {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
    long long sum = 0;
    for (long long i = 0; i < 25000000; i++) {
        long long diff = 7 * i - 31;
        long long val = llabs_c(diff);
        sum = (sum + val) % 1000000007;
    }
    QueryPerformanceCounter(&t1);
    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", ns);
    return (int)(sum % 256);
}
"""
    },
    {
        "id": 2,
        "name": "5-State Cyclic Feedforward Automaton (15M iters)",
        "category": "Coupled Recurrence (Matrix Exponentiation)",
        "canonical_ref": "Bolingbroke & Peyton Jones GHC Supercompiler",
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
    long long a = 3, b = 7, c = 11, d = 19, e = 23, acc = 0;
    for (long long i = 0; i < 15000000; i++) {
        long long t = (a ^ b) + (c & d) - e;
        acc = (acc + t) % 1000000007;
        long long next_a = b, next_b = c, next_c = d, next_d = e, next_e = a;
        a = next_a; b = next_b; c = next_c; d = next_d; e = next_e;
    }
    QueryPerformanceCounter(&t1);
    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", ns);
    return (int)((acc % 256 + 256) % 256);
}
"""
    },
    {
        "id": 3,
        "name": "Geometric Power of Three Scaling (28 iters)",
        "category": "Algebraic Homomorphism & Binary Exponentiation",
        "canonical_ref": "Romanenko SPSC / Binary Exponentiation",
        "nl": r"""
fn main() -> i64 {
    let mut p: i64 = 1;
    let mut i: i64 = 0;
    while i < 28 {
        p = (p * 3) % 1000000009;
        i = i + 1;
    }
    return p % 256;
}
""",
        "rs": r"""
fn main() {
    let t0 = std::time::Instant::now();
    let mut p: i64 = 1;
    let mut i: i64 = 0;
    while i < 28 {
        p = (p * 3) % 1000000009;
        i += 1;
    }
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit((p % 256) as i32);
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
    long long p = 1;
    for (long long i = 0; i < 28; i++) {
        p = (p * 3) % 1000000009;
    }
    QueryPerformanceCounter(&t1);
    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", ns);
    return (int)(p % 256);
}
"""
    },
    {
        "id": 4,
        "name": "Discrete Step Leapfrogging (40M iters)",
        "category": "Piecewise Linear Supercompilation & Unrolling",
        "canonical_ref": "BCE & Piecewise Induction",
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
    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", ns);
    return (int)(sum % 256);
}
"""
    },
    {
        "id": 5,
        "name": "Collatz Peak Altitude Search (20k seeds)",
        "category": "Symbolic Partial Evaluation & Space Pruning",
        "canonical_ref": "Dynamic Termination & Search Space Pruning",
        "nl": r"""
fn collatz_max(seed: i64) -> i64 {
    let mut n: i64 = seed;
    let mut peak: i64 = seed;
    let mut steps: i64 = 0;
    while n > 1 {
        if steps > 1000 { return peak; }
        if n % 2 == 0 {
            n = n / 2;
        } else {
            n = 3 * n + 1;
        }
        if n > peak {
            peak = n;
        }
        steps = steps + 1;
    }
    return peak;
}
fn main() -> i64 {
    let mut global_max: i64 = 0;
    let mut s: i64 = 1;
    while s <= 20000 {
        let p: i64 = collatz_max(s);
        if p > global_max {
            global_max = p;
        }
        s = s + 1;
    }
    return global_max % 256;
}
""",
        "rs": r"""
fn collatz_max(seed: i64) -> i64 {
    let mut n = seed;
    let mut peak = seed;
    let mut steps = 0;
    while n > 1 {
        if steps > 1000 { return peak; }
        if n % 2 == 0 {
            n /= 2;
        } else {
            n = 3 * n + 1;
        }
        if n > peak {
            peak = n;
        }
        steps += 1;
    }
    peak
}
fn main() {
    let t0 = std::time::Instant::now();
    let mut global_max = 0;
    let mut s = 1;
    while s <= 20000 {
        let p = collatz_max(s);
        if p > global_max {
            global_max = p;
        }
        s += 1;
    }
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit((global_max % 256) as i32);
}
""",
        "c": r"""
#include <stdio.h>
#include <stdlib.h>
#include <windows.h>

long long collatz_max(long long seed) {
    long long n = seed, peak = seed, steps = 0;
    while (n > 1) {
        if (steps > 1000) return peak;
        if (n % 2 == 0) n /= 2;
        else n = 3 * n + 1;
        if (n > peak) peak = n;
        steps++;
    }
    return peak;
}

int main() {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
    long long global_max = 0;
    for (long long s = 1; s <= 20000; s++) {
        long long p = collatz_max(s);
        if (p > global_max) global_max = p;
    }
    QueryPerformanceCounter(&t1);
    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", ns);
    return (int)(global_max % 256);
}
"""
    },
    {
        "id": 6,
        "name": "Coupled 3-State Tribonacci (100M iters)",
        "category": "Multi-Variable Linear Recurrence Supercompilation",
        "canonical_ref": "Klyuchnikov MRSC / Multi-Result Recurrence",
        "nl": r"""
fn main() -> i64 {
    let mut a: i64 = 1;
    let mut b: i64 = 2;
    let mut c: i64 = 4;
    let mut i: i64 = 0;
    while i < 100000000 {
        let next_c: i64 = (a + 2 * b + 3 * c) % 1000000007;
        a = b;
        b = c;
        c = next_c;
        i = i + 1;
    }
    return c % 256;
}
""",
        "rs": r"""
fn main() {
    let t0 = std::time::Instant::now();
    let mut a: i64 = 1;
    let mut b: i64 = 2;
    let mut c: i64 = 4;
    let mut i: i64 = 0;
    while i < 100000000 {
        let next_c: i64 = (a + 2 * b + 3 * c) % 1000000007;
        a = b;
        b = c;
        c = next_c;
        i += 1;
    }
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit((c % 256) as i32);
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
    long long a = 1, b = 2, c = 4;
    for (long long i = 0; i < 100000000; i++) {
        long long next_c = (a + 2 * b + 3 * c) % 1000000007;
        a = b; b = c; c = next_c;
    }
    QueryPerformanceCounter(&t1);
    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", ns);
    return (int)(c % 256);
}
"""
    },
    {
        "id": 7,
        "name": "Takeuchi Recursion tak(21, 14, 7)",
        "category": "Recursive Unfolding & Tail Optimization",
        "canonical_ref": "NoFib Imaginary Suite / Mitchell Supero",
        "nl": r"""
fn tak(x: i64, y: i64, z: i64) -> i64 {
    if y < x {
        return tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y));
    } else {
        return z;
    }
}
fn main() -> i64 {
    return tak(21, 14, 7) % 256;
}
""",
        "rs": r"""
fn tak(x: i64, y: i64, z: i64) -> i64 {
    if y < x {
        tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y))
    } else {
        z
    }
}
fn main() {
    let t0 = std::time::Instant::now();
    let res = tak(21, 14, 7);
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit((res % 256) as i32);
}
""",
        "c": r"""
#include <stdio.h>
#include <stdlib.h>
#include <windows.h>

long long tak(long long x, long long y, long long z) {
    if (y < x) {
        return tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y));
    } else {
        return z;
    }
}

int main() {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
    long long res = tak(21, 14, 7);
    QueryPerformanceCounter(&t1);
    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", ns);
    return (int)(res % 256);
}
"""
    },
    {
        "id": 8,
        "name": "Dynamic Trial Division Prime Sieve (N=50k)",
        "category": "Loop Peeling & Branch Pruning Supercompilation",
        "canonical_ref": "NoFib Imaginary Primes / Sieve",
        "nl": r"""
fn is_prime(n: i64) -> i64 {
    if n <= 1 { return 0; }
    if n <= 3 { return 1; }
    if n % 2 == 0 { return 0; }
    let mut d: i64 = 3;
    while d * d <= n {
        if n % d == 0 { return 0; }
        d = d + 2;
    }
    return 1;
}
fn main() -> i64 {
    let mut count: i64 = 0;
    let mut num: i64 = 2;
    while num < 50000 {
        if is_prime(num) == 1 {
            count = count + 1;
        }
        num = num + 1;
    }
    return count % 256;
}
""",
        "rs": r"""
fn is_prime(n: i64) -> bool {
    if n <= 1 { return false; }
    if n <= 3 { return true; }
    if n % 2 == 0 { return false; }
    let mut d = 3;
    while d * d <= n {
        if n % d == 0 { return false; }
        d += 2;
    }
    true
}
fn main() {
    let t0 = std::time::Instant::now();
    let mut count = 0;
    let mut num = 2;
    while num < 50000 {
        if is_prime(num) {
            count += 1;
        }
        num += 1;
    }
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit((count % 256) as i32);
}
""",
        "c": r"""
#include <stdio.h>
#include <stdlib.h>
#include <windows.h>

int is_prime(long long n) {
    if (n <= 1) return 0;
    if (n <= 3) return 1;
    if (n % 2 == 0) return 0;
    long long d = 3;
    while (d * d <= n) {
        if (n % d == 0) return 0;
        d += 2;
    }
    return 1;
}

int main() {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
    long long count = 0;
    for (long long num = 2; num < 50000; num++) {
        if (is_prime(num)) count++;
    }
    QueryPerformanceCounter(&t1);
    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", ns);
    return (int)(count % 256);
}
"""
    },
    {
        "id": 9,
        "name": "Rule 110 Elementary Cellular Automaton (50k)",
        "category": "Bit-Parallel Cellular Supercompilation",
        "canonical_ref": "Turing-Complete Ring Simulation",
        "nl": r"""
fn main() -> i64 {
    let mut state: i64 = 1;
    let mut step: i64 = 0;
    while step < 50000 {
        let left: i64 = (state * 2) + (state / 2147483648);
        let right: i64 = (state / 2) + ((state % 2) * 2147483648);
        let center: i64 = state;
        let next_state: i64 = (center ^ right) | ((0 - 1 - left) & right);
        state = next_state % 4294967296;
        step = step + 1;
    }
    return (state % 256 + 256) % 256;
}
""",
        "rs": r"""
fn main() {
    let t0 = std::time::Instant::now();
    let mut state: i64 = 1;
    let mut step: i64 = 0;
    while step < 50000 {
        let left: i64 = (state * 2) + (state / 2147483648);
        let right: i64 = (state / 2) + ((state % 2) * 2147483648);
        let center: i64 = state;
        let next_state: i64 = (center ^ right) | ((0 - 1 - left) & right);
        state = next_state % 4294967296;
        step += 1;
    }
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit(((state % 256 + 256) % 256) as i32);
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
    long long state = 1;
    for (long long step = 0; step < 50000; step++) {
        long long left = (state * 2) + (state / 2147483648LL);
        long long right = (state / 2) + ((state % 2) * 2147483648LL);
        long long center = state;
        long long next_state = (center ^ right) | ((0 - 1 - left) & right);
        state = next_state % 4294967296LL;
    }
    QueryPerformanceCounter(&t1);
    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", ns);
    return (int)((state % 256 + 256) % 256);
}
"""
    },
    {
        "id": 10,
        "name": "Cubic Polynomial Modular Series (2M iters)",
        "category": "Forward-Difference Degree-3 Polynomial Fitting",
        "canonical_ref": "Arbitrary-Degree Difference Tables",
        "nl": r"""
fn main() -> i64 {
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < 2000000 {
        let term: i64 = (i * i * i + 3 * i * i + 5 * i + 7) % 1000000007;
        sum = (sum + term) % 1000000007;
        i = i + 1;
    }
    return sum % 256;
}
""",
        "rs": r"""
fn main() {
    let t0 = std::time::Instant::now();
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < 2000000 {
        let term: i64 = (i * i * i + 3 * i * i + 5 * i + 7) % 1000000007;
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
    long long sum = 0;
    for (long long i = 0; i < 2000000; i++) {
        long long term = (i * i * i + 3 * i * i + 5 * i + 7) % 1000000007;
        sum = (sum + term) % 1000000007;
    }
    QueryPerformanceCounter(&t1);
    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", ns);
    return (int)(sum % 256);
}
"""
    }
]

def format_ns(ns):
    if ns <= 100:
        return "< 100 ns"
    elif ns < 1000:
        return f"{ns:.0f} ns"
    elif ns < 1_000_000:
        return f"{ns / 1000.0:.2f} us"
    elif ns < 1_000_000_000:
        return f"{ns / 1_000_000.0:.2f} ms"
    else:
        return f"{ns / 1_000_000_000.0:.2f} s"

def run_cmd(args, cwd=tmp_dir):
    try:
        p = subprocess.run(args, cwd=cwd, capture_output=True, text=True, timeout=60)
        compute_ns = 0
        for line in p.stdout.splitlines():
            if "COMPUTE_NS:" in line:
                try:
                    compute_ns = int(line.split(":")[1].strip())
                    break
                except:
                    pass
        return p.returncode, compute_ns
    except subprocess.TimeoutExpired:
        return -999, 60_000_000_000

def main():
    print("=" * 135)
    print("      NUMLANG PRODUCTION SUPERCOMPILER vs STATE-OF-THE-ART COMPILERS & RESEARCH PARADIGMS")
    print("=" * 135)
    print(f"{'#':<3} | {'Benchmark Name':<46} | {'NumLang Supercompiler':<21} | {'Rust (LLVM -O3)':<15} | {'C (MSVC /O2)':<14} | {'Exit Match'}")
    print("-" * 135)

    for b in benchmarks:
        slug = f"bench_{b['id']}"
        nl_src = os.path.join(tmp_dir, f"{slug}.nl")
        nl_exe = os.path.join(tmp_dir, f"{slug}_nl.exe")
        rs_src = os.path.join(tmp_dir, f"{slug}.rs")
        rs_exe = os.path.join(tmp_dir, f"{slug}_rs.exe")
        c_src = os.path.join(tmp_dir, f"{slug}.c")
        c_exe = os.path.join(tmp_dir, f"{slug}_c.exe")

        # 1. Compile NumLang with --bench
        with open(nl_src, "w") as f:
            f.write(b["nl"])
        res_nl = subprocess.run([nl_bin, "build", nl_src, "--bench", "-o", nl_exe], capture_output=True, text=True)
        if res_nl.returncode != 0:
            print(f"NumLang build failed for {b['name']}:\n{res_nl.stderr}")
            continue

        # 2. Compile Rust (LLVM -O3 native)
        with open(rs_src, "w") as f:
            f.write(b["rs"])
        res_rs = subprocess.run(["rustc", "-C", "opt-level=3", "-C", "target-cpu=native", rs_src, "-o", rs_exe], capture_output=True, text=True)
        if res_rs.returncode != 0:
            print(f"Rust build failed for {b['name']}:\n{res_rs.stderr}")
            continue

        # 3. Compile MSVC C (/O2)
        with open(c_src, "w") as f:
            f.write(b["c"])
        bat_path = os.path.join(tmp_dir, f"build_{slug}.bat")
        with open(bat_path, "w") as f:
            f.write(f'@call "{vcvars}" >nul 2>&1\n@cl /O2 /Oi /Ot /nologo /Fe:"{c_exe}" "{c_src}" >nul 2>&1\n')
        res_c = subprocess.run(["cmd.exe", "/c", bat_path], capture_output=True, text=True)
        if not os.path.exists(c_exe):
            print(f"MSVC C build failed for {b['name']}")
            continue

        # Benchmark runs (3 iterations, best of 3)
        nl_times = []
        nl_code = None
        for _ in range(3):
            code, ns = run_cmd([nl_exe])
            nl_code = code
            nl_times.append(ns)
        nl_min = min(nl_times) if nl_times else 0

        rs_times = []
        rs_code = None
        for _ in range(3):
            code, ns = run_cmd([rs_exe])
            rs_code = code
            rs_times.append(ns)
        rs_min = min(rs_times) if rs_times else 0

        c_times = []
        c_code = None
        for _ in range(3):
            code, ns = run_cmd([c_exe])
            c_code = code
            c_times.append(ns)
        c_min = min(c_times) if c_times else 0

        match = (nl_code == rs_code == c_code)
        match_str = f"YES ({nl_code})" if match else f"FAIL ({nl_code}/{rs_code}/{c_code})"

        print(f"{b['id']:<3} | {b['name']:<46} | {format_ns(nl_min):<21} | {format_ns(rs_min):<15} | {format_ns(c_min):<14} | {match_str}")

    print("=" * 135)

if __name__ == "__main__":
    main()
