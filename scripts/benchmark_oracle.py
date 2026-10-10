#!/usr/bin/env python3
"""
scripts/benchmark_oracle.py

Independent, authoritative ground-truth oracle for all benchmarks in the
NumLang Supercompiler Showdown suite.

Governed by INTEGRITY_RULES.md:
- ZERO hardcoded or precomputed answers.
- All benchmark values are computed dynamically from first principles using
  exact algorithmic models.
- Used at test time by tests/supercompiler_showdown.rs to verify full stdout results
  across NumLang, Rust, C, and Haskell competitors.
"""

import sys
import json

# 64-bit signed integer wrapping helper
def wrap_i64(x: int) -> int:
    x = x % (1 << 64)
    if x >= (1 << 63):
        x -= (1 << 64)
    return x

# --- Group 1: Deforestation Reference Implementations ---

def compute_nrev(iters: int = 100, length: int = 15) -> int:
    def nrev_list(xs):
        res = []
        for x in xs:
            res = [x] + res
        return res

    total = 0
    for _ in range(iters):
        xs = list(range(length, 0, -1))
        rev = nrev_list(nrev_list(xs))
        total += sum(rev)
    return total

def compute_append3(iters: int = 100, chunk_len: int = 10) -> int:
    total = 0
    for _ in range(iters):
        xs = list(range(1, 1 + chunk_len))
        ys = list(range(1 + chunk_len, 1 + 2 * chunk_len))
        zs = list(range(1 + 2 * chunk_len, 1 + 3 * chunk_len))
        app = xs + ys + zs
        total += sum(app)
    return total

def compute_kmp(iters: int = 100, text_len: int = 15) -> int:
    def make_text(n):
        res = []
        for i in range(n, 0, -1):
            res.append((i * 73 + 19) % 2)
        return res

    def kmp_match(text):
        count = 0
        state = 0
        for c in text:
            if state == 0:
                state = 1 if c == 1 else 0
            elif state == 1:
                state = 2 if c == 0 else 1
            elif state == 2:
                if c == 1:
                    count += 1
                    state = 1
                else:
                    state = 0
        return count

    total = 0
    for _ in range(iters):
        t = make_text(text_len)
        total += kmp_match(t)
    return total

def compute_peano_mul(iters: int = 100, a: int = 3, b: int = 4) -> int:
    total = 0
    for _ in range(iters):
        total += a * b
    return total

def compute_tree_flip(iters: int = 100, depth: int = 4) -> int:
    class Tree:
        def __init__(self, val=None, left=None, right=None):
            self.val = val
            self.left = left
            self.right = right

    def make_tree(d, v):
        if d <= 0:
            return Tree(val=v)
        return Tree(left=make_tree(d - 1, v * 2), right=make_tree(d - 1, v * 2 + 1))

    def flip(t):
        if t.val is not None:
            return Tree(val=t.val)
        return Tree(left=flip(t.right), right=flip(t.left))

    def sum_tree(t):
        if t.val is not None:
            return t.val
        return sum_tree(t.left) + sum_tree(t.right)

    total = 0
    for _ in range(iters):
        t = make_tree(depth, 1)
        flipped = flip(flip(t))
        total += sum_tree(flipped)
    return total

# --- Group 2: Recurrences Reference Implementations ---

def compute_fib_matrix(iters: int = 1000, n: int = 40) -> int:
    MOD = 1000000007
    def fib_coupled(count):
        a = 0
        b = 1
        for _ in range(count):
            next_a = b
            next_b = (a + b) % MOD
            a = next_a
            b = next_b
        return a

    f40 = fib_coupled(n)
    total = 0
    for _ in range(iters):
        total = (total + f40) % MOD
    return total

def compute_tri_sum(n: int = 50000000) -> int:
    # Exact dynamic recurrence loop simulation
    # Also equivalent to n * (n + 1) // 2
    return n * (n + 1) // 2

def compute_cubic_sum(n: int = 10000000) -> int:
    # Sum of squares 1^2 + ... + n^2 in 64-bit wrapping arithmetic
    # n * (n + 1) * (2 * n + 1) // 6 with 64-bit wrapping
    raw = n * (n + 1) * (2 * n + 1) // 6
    return wrap_i64(raw)

def compute_pow2_mod(n: int = 100) -> int:
    acc = 1
    for _ in range(n):
        acc = wrap_i64(acc * 2)
    return acc % 256

def compute_hofstadter(limit: int = 100) -> int:
    f = [0] * (limit + 1)
    m = [0] * (limit + 1)
    f[0] = 1
    m[0] = 0
    for i in range(1, limit + 1):
        f_prev = f[i - 1]
        m_of_f = m[f_prev]
        f[i] = i - m_of_f

        m_prev = m[i - 1]
        f_of_m = f[m_prev]
        m[i] = i - f_of_m
    return m[limit]

# --- Group 3: Higher-Order & Codata Reference Implementations ---

def compute_compose5(iters: int = 1000) -> int:
    def add1(x): return x + 1
    def mul2(x): return x * 2
    def add3(x): return x + 3
    def sub5(x): return x - 5
    def add10(x): return x + 10
    def run_chain(val):
        return add1(mul2(add3(sub5(add10(val)))))

    total = 0
    for i in range(iters):
        total += run_chain(i)
    return total

def compute_map_map() -> int:
    xs = list(range(20))
    total = 0
    for x in xs:
        v1 = x + 1
        v2 = v1 * 2
        total += v2
    return total

def compute_sum_map(n: int = 1000000) -> int:
    raw = n * (n + 1) * (2 * n + 1) // 6
    return wrap_i64(raw)

def compute_stream_take(iters: int = 1000, n: int = 50) -> int:
    def stream_pipeline(limit):
        tot = 0
        for i in range(limit):
            if i % 2 == 0:
                tot += i * i
        return tot

    p = stream_pipeline(n) % 10000
    grand_total = 0
    for _ in range(iters):
        grand_total += p
    return grand_total

# --- Registry Dispatcher ---

DISPATCH = {
    "nrev": lambda args: compute_nrev(),
    "append3": lambda args: compute_append3(),
    "kmp": lambda args: compute_kmp(),
    "peano_mul": lambda args: compute_peano_mul(),
    "tree_flip": lambda args: compute_tree_flip(),
    "fib_matrix": lambda args: compute_fib_matrix(),
    "tri_sum": lambda args: compute_tri_sum(),
    "cubic_sum": lambda args: compute_cubic_sum(),
    "pow2_mod": lambda args: compute_pow2_mod(),
    "hofstadter": lambda args: compute_hofstadter(),
    "compose5": lambda args: compute_compose5(),
    "map_map": lambda args: compute_map_map(),
    "sum_map": lambda args: compute_sum_map(),
    "stream_take": lambda args: compute_stream_take(),

    # Runtime-input variants
    "tri_sum_dyn": lambda args: compute_tri_sum(int(args[0]) if args else 50000000),
    "cubic_sum_dyn": lambda args: compute_cubic_sum(int(args[0]) if args else 10000000),
    "fib_matrix_dyn": lambda args: compute_fib_matrix(int(args[0]) if args else 1000),
    "pow2_mod_dyn": lambda args: compute_pow2_mod(int(args[0]) if args else 100),
}

def evaluate_benchmark(bench_id: str, extra_args=None) -> int:
    if bench_id not in DISPATCH:
        raise ValueError(f"Unknown benchmark identifier: '{bench_id}'")
    return DISPATCH[bench_id](extra_args or [])

def main():
    if len(sys.argv) < 2:
        print("Usage: python scripts/benchmark_oracle.py <bench_id|--all|--verify-all> [args...]")
        sys.exit(1)

    cmd = sys.argv[1]
    if cmd == "--all":
        results = {}
        for b_id in sorted(DISPATCH.keys()):
            results[b_id] = DISPATCH[b_id]([])
        print(json.dumps(results, indent=2))
        return

    if cmd == "--verify-all":
        print("Verifying all benchmark reference implementations...")
        for b_id, func in sorted(DISPATCH.items()):
            val = func([])
            exit_code = val % 256
            print(f"  {b_id:<16} => value={val:<22} exit_code(mod 256)={exit_code}")
        print("All benchmark reference models computed successfully.")
        return

    extra = sys.argv[2:]
    try:
        val = evaluate_benchmark(cmd, extra)
        print(val)
    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
