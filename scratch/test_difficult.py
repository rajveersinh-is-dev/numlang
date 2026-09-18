import subprocess
import os
import sys
import time

vcvars = r"C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
nl_bin = r"C:\Users\davea\.gemini\antigravity\scratch\numlang\target\release\numlang.exe"
cycle_bench = r"C:\Users\davea\.gemini\antigravity\brain\d21c1850-b996-4958-951d-ea34362faef0\scratch\cycle_bench.exe"
tmp_dir = r"C:\Users\davea\.gemini\antigravity\scratch"

difficult_benchmarks = [
    {
        "id": 1,
        "name": "Coupled 3-State Tribonacci Recurrence (100M iters)",
        "desc": "Exceeds 60M step budget, multi-variable recurrence under modulo 10^9+7",
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
        a = b;
        b = c;
        c = next_c;
    }

    QueryPerformanceCounter(&t1);
    double ns = (double)(t1.QuadPart - t0.QuadPart) * 1000000000.0 / (double)freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(c % 256);
}
"""
    },
    {
        "id": 2,
        "name": "Nested Prime Sieve via Dynamic Trial Division (N=50,000)",
        "desc": "Nested while loop with quadratic condition (d*d <= n) and dynamic break",
        "nl": r"""
fn main() -> i64 {
    let mut primes: i64 = 0;
    let mut n: i64 = 2;
    while n <= 50000 {
        let mut is_prime: i64 = 1;
        let mut d: i64 = 2;
        while d * d <= n {
            if n % d == 0 {
                is_prime = 0;
                d = n;
            }
            d = d + 1;
        }
        if is_prime == 1 {
            primes = primes + 1;
        }
        n = n + 1;
    }
    return primes % 256;
}
""",
        "rs": r"""
fn main() {
    let t0 = std::time::Instant::now();
    let mut primes: i64 = 0;
    let mut n: i64 = 2;
    while n <= 50000 {
        let mut is_prime: i64 = 1;
        let mut d: i64 = 2;
        while d * d <= n {
            if n % d == 0 {
                is_prime = 0;
                d = n;
            }
            d += 1;
        }
        if is_prime == 1 {
            primes += 1;
        }
        n += 1;
    }
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit((primes % 256) as i32);
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

    long long primes = 0;
    for (long long n = 2; n <= 50000; n++) {
        long long is_prime = 1;
        for (long long d = 2; d * d <= n; d++) {
            if (n % d == 0) {
                is_prime = 0;
                break;
            }
        }
        if (is_prime == 1) {
            primes++;
        }
    }

    QueryPerformanceCounter(&t1);
    double ns = (double)(t1.QuadPart - t0.QuadPart) * 1000000000.0 / (double)freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(primes % 256);
}
"""
    },
    {
        "id": 3,
        "name": "Rule 110 Elementary Cellular Automaton (50,000 steps)",
        "desc": "Turing-complete elementary cellular automaton simulation with bit shifts",
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
    double ns = (double)(t1.QuadPart - t0.QuadPart) * 1000000000.0 / (double)freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(((state % 256) + 256) % 256);
}
"""
    },
    {
        "id": 4,
        "name": "Takeuchi Deep Recursion Benchmark: tak(21, 14, 7)",
        "desc": "Deep combinatorial tree recursion stressing stack frames",
        "nl": r"""
fn tak(x: i64, y: i64, z: i64) -> i64 {
    if y < x {
        let a: i64 = tak(x - 1, y, z);
        let b: i64 = tak(y - 1, z, x);
        let c: i64 = tak(z - 1, x, y);
        return tak(a, b, c);
    } else {
        return z;
    }
}

fn main() -> i64 {
    let res: i64 = tak(21, 14, 7);
    return res % 256;
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
#include <intrin.h>

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
    _mm_mfence();
    QueryPerformanceCounter(&t0);
    _mm_mfence();

    volatile long long res = tak(21, 14, 7);

    _mm_mfence();
    QueryPerformanceCounter(&t1);
    _mm_mfence();
    double ns = (double)(t1.QuadPart - t0.QuadPart) * 1000000000.0 / (double)freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(res % 256);
}
"""
    },
    {
        "id": 5,
        "name": "Cubic Polynomial Modular Series (2M iters)",
        "desc": "Degree-3 polynomial accumulator sum_{i=0}^{2M} (i^3 + 2i^2 + 3i + 5) mod 10^9+7 without i64 overflow",
        "nl": r"""
fn main() -> i64 {
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < 2000000 {
        let term: i64 = (i * i * i + 2 * i * i + 3 * i + 5) % 1000000007;
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
        let term: i64 = (i * i * i + 2 * i * i + 3 * i + 5) % 1000000007;
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
        long long term = (i * i * i + 2 * i * i + 3 * i + 5) % 1000000007;
        sum = (sum + term) % 1000000007;
    }

    QueryPerformanceCounter(&t1);
    double ns = (double)(t1.QuadPart - t0.QuadPart) * 1000000000.0 / (double)freq.QuadPart;
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}
"""
    }
]

print(f"Loaded {len(difficult_benchmarks)} difficult benchmarks.")

def format_ns(ns):
    if ns < 1000:
        return f"{ns} ns"
    elif ns < 1_000_000:
        return f"{ns/1000:.2f} us"
    else:
        return f"{ns/1_000_000:.2f} ms"

results = []

for b in difficult_benchmarks:
    b_id = b["id"]
    name = b["name"]
    print(f"\n[{b_id}/5] Testing: {name}...")
    nl_file = os.path.join(tmp_dir, f"diff_{b_id}.nl")
    obj_file = os.path.join(tmp_dir, f"diff_{b_id}.obj")
    rs_file = os.path.join(tmp_dir, f"diff_{b_id}.rs")
    c_file = os.path.join(tmp_dir, f"diff_{b_id}.c")
    exe_nl = os.path.join(tmp_dir, f"diff_{b_id}_nl.exe")
    exe_rs = os.path.join(tmp_dir, f"diff_{b_id}_rs.exe")
    exe_c = os.path.join(tmp_dir, f"diff_{b_id}_c.exe")

    with open(nl_file, "w") as f: f.write(b["nl"])
    with open(rs_file, "w") as f: f.write(b["rs"])
    with open(c_file, "w") as f: f.write(b["c"])

    # 1. Compile NumLang executable
    t_nl_c0 = time.perf_counter()
    env = os.environ.copy()
    env["NUMLANG_BENCH"] = "1"
    nl_res = subprocess.run([nl_bin, "build", nl_file, "--bench", "-o", exe_nl],
                            env=env, capture_output=True, text=True)
    t_nl_c1 = time.perf_counter()
    nl_compile_time_ms = (t_nl_c1 - t_nl_c0) * 1000.0

    if nl_res.returncode != 0:
        print(f"  NumLang build FAILED:\n{nl_res.stderr}")
        continue

    # Also emit obj for cycle_bench analysis
    subprocess.run([nl_bin, "build", nl_file, "--bench", "--emit-obj", obj_file],
                   env=env, capture_output=True, text=True)

    # 2. Compile Rust with rustc -O
    subprocess.run(["rustc", "-O", "-o", exe_rs, rs_file], check=True)

    # 3. Compile C with MSVC cl /O2
    c_cmd = f'cmd /c "call "{vcvars}" >nul 2>&1 && cl /O2 /nologo "{c_file}" /Fe:"{exe_c}" >nul 2>&1"'
    subprocess.run(c_cmd, shell=True, check=True)

    # 4. Run Rust
    p_rs = subprocess.run([exe_rs], capture_output=True, text=True)
    rs_exit = p_rs.returncode
    rs_ns = 0
    for line in p_rs.stdout.splitlines():
        if "COMPUTE_NS:" in line:
            rs_ns = int(line.split(":")[1].strip())

    # 5. Run C
    p_c = subprocess.run([exe_c], capture_output=True, text=True)
    c_exit = p_c.returncode
    c_ns = 0
    for line in p_c.stdout.splitlines():
        if "COMPUTE_NS:" in line:
            c_ns = int(line.split(":")[1].strip())

    # 6. Run NumLang executable
    p_nl = subprocess.run([exe_nl], capture_output=True, text=True)
    nl_exit = p_nl.returncode
    nl_qpc_ns = 0
    for line in p_nl.stdout.splitlines():
        if "COMPUTE_NS:" in line:
            nl_qpc_ns = int(line.split(":")[1].strip())

    # 7. Run cycle_bench if elevated to single obj or native
    p_cb = subprocess.run([cycle_bench, obj_file], capture_output=True, text=True)
    cb_out = p_cb.stdout
    nl_exact_info = ""
    nl_throughput = ""
    code_size = 0
    for line in cb_out.splitlines():
        if "Emitted Machine Code Size:" in line:
            code_size = int(line.split(":")[1].strip().split()[0])
        elif "Minimum Observed:" in line:
            nl_exact_info = line.split("-->")[1].strip()
        elif "Net Steady-State Rate:" in line:
            nl_throughput = line.split("-->")[1].strip()

    # Determine whether it was closed-form or native loop
    is_closed_form = (code_size > 0 and code_size <= 24 and p_cb.returncode == 0)

    results.append({
        "id": b_id,
        "name": name,
        "desc": b["desc"],
        "is_closed_form": is_closed_form,
        "code_size": code_size,
        "nl_compile_ms": nl_compile_time_ms,
        "nl_exit": nl_exit,
        "rs_exit": rs_exit,
        "c_exit": c_exit,
        "nl_exact": nl_throughput if is_closed_form else format_ns(nl_qpc_ns),
        "rs_time": format_ns(rs_ns),
        "c_time": format_ns(c_ns),
        "match": (nl_exit == rs_exit == c_exit)
    })

    print(f"  Done. Exit codes: NL={nl_exit}, RS={rs_exit}, C={c_exit} (Match: {nl_exit == rs_exit == c_exit})", flush=True)
    print(f"  Mode: {'Closed Form (Elevated)' if is_closed_form else 'Native Loop (Cranelift)'}, Code Size: {code_size} bytes", flush=True)
    print(f"  Times: NL={results[-1]['nl_exact']} | RS={results[-1]['rs_time']} | C={results[-1]['c_time']}", flush=True)

print("\n" + "=" * 135, flush=True)
print(f"{'#':<2} | {'Difficult Benchmark':<45} | {'Mode':<14} | {'NumLang Time':<18} | {'Rust (-O)':<12} | {'C (/O2)':<12} | {'Exit Match'}", flush=True)
print("=" * 135, flush=True)
for r in results:
    mode_str = "Closed Form" if r["is_closed_form"] else "Native Loop"
    match_str = f"YES ({r['nl_exit']})" if r["match"] else f"NO (NL:{r['nl_exit']} RS:{r['rs_exit']} C:{r['c_exit']})"
    print(f"{r['id']:<2} | {r['name']:<45} | {mode_str:<14} | {r['nl_exact']:<18} | {r['rs_time']:<12} | {r['c_time']:<12} | {match_str}", flush=True)
print("=" * 135, flush=True)

