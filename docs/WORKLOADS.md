# Extended Workloads and Compile-Time Benchmarks

This document records two benchmark-contract additions made by Agent C:

1. four **new realistic workloads** under `bench/showdown/extended/`, which are
   deliberately *not* variations of the three distillation templates or the 18
   canonical microkernels;
2. a compile-time benchmark harness, `tests/compile_time_benchmarks.rs`, which
   writes `bench/data/compile_times.csv` and closes the contract v1.0 §7 gap
   where compile-time was published as `N/A`.

No existing registry, harness, compiler source, table generator, or README was
modified. The exact follow-up edits needed for orchestrator integration are in
§4 below.

---

## 1. New realistic workloads

All four workloads were authored for this repository on 2026-11-21 by Agent C.
They are **original constructions**, not copies of an external benchmark
corpus. Their inputs are fixed so that every implementation is deterministic
and the oracle can recompute the answer from first principles. Each workload
has a NumLang source, a Rust reference, a C reference, and a documented
expected exit code. Rust and C emit the same `COMPUTE_NS` line as the existing
showdown references when built with optimization.

| ID | NumLang source | Rust reference | C reference | stdout | exit |
|:---|:---|:---|:---|---:|---:|
| `expr_eval` | `bench/showdown/extended/numlang/expr_eval.nl` | `bench/showdown/extended/rust/expr_eval.rs` | `bench/showdown/extended/c/expr_eval.c` | `14` | 14 |
| `window_stats` | `bench/showdown/extended/numlang/window_stats.nl` | `bench/showdown/extended/rust/window_stats.rs` | `bench/showdown/extended/c/window_stats.c` | `248` | 248 |
| `record_pipeline` | `bench/showdown/extended/numlang/record_pipeline.nl` | `bench/showdown/extended/rust/record_pipeline.rs` | `bench/showdown/extended/c/record_pipeline.c` | `7466` | 42 |
| `nested_stencil` | `bench/showdown/extended/numlang/nested_stencil.nl` | `bench/showdown/extended/rust/nested_stencil.rs` | `bench/showdown/extended/c/nested_stencil.c` | `62` | 62 |

Cross-check performed on 2026-11-21: NumLang `--use-mir`, NumLang
`--supercompile`, Rust `-C opt-level=3`, and C `/O2` all printed the stdout
value and returned the exit code listed above.

### 1.1 `expr_eval` — stack-machine expression evaluator

- **Provenance**: original bytecode-interpreter kernel authored for this repo.
  The 11-instruction program evaluates `((7+5)*3-11) % (2+9) = 3`; the
  observable result adds the executed instruction count, giving 14.
- **Structure**: 23-cell code array with a header-encoded program bound,
  parallel opcode/operand stream, 16-slot operand stack, five dispatch branches,
  and a data-dependent `pc`/`sp`.
- **Why it is not benchmark-shaped**: the existing group-1 workloads are
  producer/consumer tree or list pipelines (`nrev`, `append3`, `tree_flip`) and
  group-3 workloads are closure/stream pipelines. This is an interpreter with
  opcode dispatch and mutable stack state; the loop bound comes from a code
  header rather than a literal trip count, and the result depends on the dynamic
  dispatch count. It is neither a deforestation template nor a closed-form
  recurrence.

### 1.2 `window_stats` — sliding-window sensor statistics

- **Provenance**: original signal-processing-style kernel authored for this
  repo. Control record `[6, 24, 3]` supplies window size, stream length, and
  reporting stride; the 24 samples are fixed test telemetry.
- **Structure**: nested loops with a data-derived window bound, running sum,
  min/max reductions, modulo-based score, and stride-dependent branching.
- **Why it is not benchmark-shaped**: the 18 canonical recurrences collapse a
  single arithmetic accumulation to closed form. Here the inner reduction is
  branchy min/max over a data window, the outer bound is loaded from a control
  record, and the accumulation alternates between two update rules. No algebraic
  identity or producer/consumer fusion template applies.

### 1.3 `record_pipeline` — struct/array transform pipeline

- **Provenance**: original record-processing pipeline authored for this repo.
  Ten `Reading { id, raw, weight, bucket }` records are normalised, bucketed,
  and boosted by a position-dependent factor.
- **Structure**: fixed-size array of flat structs, three passes with separate
  scalar arrays, computed bucket indices, and a final weighted reduction
  (stdout 7466, exit 42).
- **Why it is not benchmark-shaped**: `map_map`/`sum_map` are single producer
  functions over scalar arrays; `tree_flip` is a double constructor inversion.
  This workload combines heterogeneous records, multi-pass arrays, and bucket
  scatter accumulation. It exercises SROA, struct layout, and aliasing rather
  than a named distillation law.

### 1.4 `nested_stencil` — multi-pass 2D stencil with clamped halo

- **Provenance**: original stencil kernel authored for this repo. Control
  record `[3, 6, 1, 2]` supplies pass count, dimension, and two kernel weights.
- **Structure**: 6×6 grid, 3 passes, 9-tap neighbourhood, clamped boundary
  indices, asymmetric parity-dependent coefficients, and a full copy-back
  between passes.
- **Why it is not benchmark-shaped**: `bench/numlang/jacobi_stencil.nl` is a
  4×4, 10-pass, 4-neighbour uniform average. This kernel has a 9-point
  asymmetric stencil, clamped halo, control-derived weights, and a different
  observable cell; it is a realistic grid computation rather than a one-line
  recurrence or a tree/list template.

---

## 2. Compile-time benchmark methodology

`tests/compile_time_benchmarks.rs` measures the **supercompiler specialization
stage** in-process:

- `NumLang-SC-specialize`: typed-AST distillation (`ho_distill + supercompile`),
  MIR lowering, then `supercompile_mir_program`.
- `NumLang-Base-lower`: same pipeline with distillation and supercompilation
  disabled, used as a control for the overhead attributable to specialization.

Timing uses `std::time::Instant` only: `QueryPerformanceCounter` on Windows and
`clock_gettime(CLOCK_MONOTONIC)` on POSIX. No wall-clock `SystemTime`, no lookup
tables, and no synthetic values are used. For each source/benchmark pair the
typed AST is cloned once per round outside the timed region, and each residual
`MirProgram` is passed through `std::hint::black_box` so the work cannot be
eliminated.

Protocols:

| Mode | Warmups (discarded) | Measured rounds | Selection |
|:---|---:|---:|:---|
| FULL | 5 | 30 | default `cargo test --test compile_time_benchmarks` |
| QUICK | 2 | 5 | `COMPILE_TIME_QUICK=1` or `QUICK_BENCHMARKS=1` |

QUICK is the contract's documented smoke mode; FULL is the default and was used
for the committed CSV. The full run takes about 4–5 s on the measurement host,
so CI can run it without a special exemption.

CSV schema (`bench/data/compile_times.csv`):

```
benchmark_id,system,warmup_ns,median_compile_ns,p95_compile_ns,rounds,status
```

- `warmup_ns` is the **summed** duration of the discarded warmup rounds.
- `median_compile_ns` and `p95_compile_ns` use nearest-rank percentiles over the
  measured rounds, consistent with `tests/supercompiler_showdown.rs`.
- `rounds` records the number actually used, so QUICK-mode files are
  self-describing.
- Missing or unreadable sources produce `SKIPPED_*` rows and never fail the
  test; the current file contains 36 `SUCCESS` rows (18 canonical + 4 extended,
  each with SC and Base systems).

Measurement host for the committed CSV:

- CPU: 11th Gen Intel(R) Core(TM) i5-11260H @ 2.60GHz
- OS: Microsoft Windows 11 Home Single Language (AMD64)
- Rust: `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- Clock: `QueryPerformanceCounter`
- Quantization floor: the showdown's published `<= 500 ns*` runtime floor does
  **not** apply here; all measured specialization medians are far above 500 ns.

Scope note: this is the supercompiler-specific compile-time component. It does
not include process startup, linking, or binary emission; a future end-to-end
`numlang build` wall-clock column should be added separately rather than
mislabelling these numbers as full build time.

---

## 3. Provenance integrity

- No expected result is hardcoded in the benchmark harness; the CSV contains
  only measured nanosecond counts and status strings.
- The four workloads' expected values are recomputed dynamically by the oracle
  functions in §4.3 and were independently checked against Rust and C
  references.
- The workloads introduce no new "fastest" claim. Their `expected_gain` field
  is explicitly `N/A`, because their purpose is realism and robustness
  coverage, not a predicted supercompiler win.

---

## 4. Exact orchestrator additions required (after Agent A finishes)

These edits are **not** applied here because the registry/harness/oracle files
are outside this agent's ownership. They should be appended by the orchestrator.

### 4.1 `bench/showdown/benchmarks.toml`

Append four entries (suggested group `G4: Extended Realistic`):

```toml
[[benchmark]]
id = "expr_eval"
group = "G4: Extended Realistic"
name = "Bytecode Expression Evaluator"
algorithm = "Stack-machine interpreter over 11-instruction bytecode with header-driven bound"
transformation = "N/A - control-flow/data-dependent interpreter probe"
expected_gain = "N/A (realism probe; no predicted supercompiler win)"
expected_exit = 14
numlang = "bench/showdown/extended/numlang/expr_eval.nl"
haskell = ""
hosc = ""
c = "bench/showdown/extended/c/expr_eval.c"
rust = "bench/showdown/extended/rust/expr_eval.rs"

[[benchmark]]
id = "window_stats"
group = "G4: Extended Realistic"
name = "Sliding-Window Sensor Statistics"
algorithm = "Data-derived window/min/max/sum over 24-sample telemetry stream"
transformation = "N/A - branchy data-dependent window reduction"
expected_gain = "N/A (realism probe; no predicted supercompiler win)"
expected_exit = 248
numlang = "bench/showdown/extended/numlang/window_stats.nl"
haskell = ""
hosc = ""
c = "bench/showdown/extended/c/window_stats.c"
rust = "bench/showdown/extended/rust/window_stats.rs"

[[benchmark]]
id = "record_pipeline"
group = "G4: Extended Realistic"
name = "Struct Record Transform Pipeline"
algorithm = "Normalise/bucket/boost 10 struct records into 4 weighted buckets"
transformation = "N/A - SROA/array-of-struct multi-pass pipeline"
expected_gain = "N/A (realism probe; no predicted supercompiler win)"
expected_exit = 42
numlang = "bench/showdown/extended/numlang/record_pipeline.nl"
haskell = ""
hosc = ""
c = "bench/showdown/extended/c/record_pipeline.c"
rust = "bench/showdown/extended/rust/record_pipeline.rs"

[[benchmark]]
id = "nested_stencil"
group = "G4: Extended Realistic"
name = "Multi-Pass Clamped 2D Stencil"
algorithm = "3-pass 6x6 nine-point stencil with clamped halo and control weights"
transformation = "N/A - memory-bound grid kernel, no collapse claimed"
expected_gain = "N/A (realism probe; no predicted supercompiler win)"
expected_exit = 62
numlang = "bench/showdown/extended/numlang/nested_stencil.nl"
haskell = ""
hosc = ""
c = "bench/showdown/extended/c/nested_stencil.c"
rust = "bench/showdown/extended/rust/nested_stencil.rs"
```

### 4.2 `tests/supercompiler_showdown.rs`

Append these four `BenchmarkSpec` values to `all_benchmarks` (after the existing
group-3 entries and after any dynamic variants Agent A adds):

```rust
BenchmarkSpec {
    id: "expr_eval",
    group: "G4: Extended Realistic",
    name: "Bytecode Expression Evaluator",
    algorithm: "Stack-machine interpreter over 11-instruction bytecode",
    expected_exit: 14,
    runtime_arg: None,
    nl_rel: "bench/showdown/extended/numlang/expr_eval.nl",
    rs_rel: "bench/showdown/extended/rust/expr_eval.rs",
    c_rel: "bench/showdown/extended/c/expr_eval.c",
    hs_rel: "",
},
BenchmarkSpec {
    id: "window_stats",
    group: "G4: Extended Realistic",
    name: "Sliding-Window Sensor Statistics",
    algorithm: "Data-derived window/min/max/sum over 24-sample stream",
    expected_exit: 248,
    runtime_arg: None,
    nl_rel: "bench/showdown/extended/numlang/window_stats.nl",
    rs_rel: "bench/showdown/extended/rust/window_stats.rs",
    c_rel: "bench/showdown/extended/c/window_stats.c",
    hs_rel: "",
},
BenchmarkSpec {
    id: "record_pipeline",
    group: "G4: Extended Realistic",
    name: "Struct Record Transform Pipeline",
    algorithm: "Normalise/bucket/boost 10 struct records into 4 buckets",
    expected_exit: 42,
    runtime_arg: None,
    nl_rel: "bench/showdown/extended/numlang/record_pipeline.nl",
    rs_rel: "bench/showdown/extended/rust/record_pipeline.rs",
    c_rel: "bench/showdown/extended/c/record_pipeline.c",
    hs_rel: "",
},
BenchmarkSpec {
    id: "nested_stencil",
    group: "G4: Extended Realistic",
    name: "Multi-Pass Clamped 2D Stencil",
    algorithm: "3-pass 6x6 nine-point stencil with clamped halo",
    expected_exit: 62,
    runtime_arg: None,
    nl_rel: "bench/showdown/extended/numlang/nested_stencil.nl",
    rs_rel: "bench/showdown/extended/rust/nested_stencil.rs",
    c_rel: "bench/showdown/extended/c/nested_stencil.c",
    hs_rel: "",
},
```

Haskell/HOSC paths are empty because no Haskell or HOSC reference was added;
the harness already treats empty paths as `NOT_INSTALLED`/skipped.

### 4.3 `scripts/benchmark_oracle.py`

Add four dynamic reference functions before the `# --- Registry Dispatcher ---`
marker:

```python
def compute_expr_eval() -> int:
    code = [22, 1, 7, 1, 5, 2, 0, 1, 3, 3, 0, 1, 11, 4, 0, 1, 2, 1, 9, 2, 0, 5, 0]
    stack = []
    pc = 1
    end = 1 + code[0]
    executed = 0
    while pc < end:
        op = code[pc]
        arg = code[pc + 1]
        if op == 1:
            stack.append(arg)
        elif op == 2:
            b = stack.pop(); a = stack.pop(); stack.append(wrap_i64(a + b))
        elif op == 3:
            b = stack.pop(); a = stack.pop(); stack.append(wrap_i64(a * b))
        elif op == 4:
            b = stack.pop(); a = stack.pop(); stack.append(wrap_i64(a - b))
        elif op == 5:
            b = stack.pop(); a = stack.pop()
            stack.append(0 if b == 0 else a % b)
        pc += 2
        executed += 1
    return wrap_i64(stack[-1] + executed)


def compute_window_stats() -> int:
    window, n, stride = [6, 24, 3]
    samples = [4, 9, 2, 17, 6, 11, 3, 20, 8, 14, 1, 7,
               19, 5, 12, 10, 16, 2, 13, 6, 18, 4, 9, 15]
    acc = 0
    start = 0
    while start + window <= n:
        chunk = samples[start:start + window]
        score = (max(chunk) - min(chunk)) + sum(chunk) % 7
        if start % stride == 0:
            acc = wrap_i64(acc + score)
        else:
            acc = wrap_i64(acc + score // 2)
        start += 1
    return acc


def compute_record_pipeline() -> int:
    records = [
        (0, 137, 3, 2), (1, 89, 5, 1), (2, 204, 2, 3), (3, 45, 7, 0),
        (4, 176, 4, 2), (5, 63, 6, 1), (6, 151, 3, 3), (7, 28, 8, 0),
        (8, 199, 2, 2), (9, 112, 5, 1),
    ]
    norm = [(raw % 101) * weight for (_id, raw, weight, _bucket) in records]
    buckets = [0] * 4
    for (id_, _raw, _weight, bucket), nv in zip(records, norm):
        b = id_ % 4
        buckets[b] = wrap_i64(buckets[b] + nv + bucket)
    total = 0
    for k in range(4):
        boost = k * 2
        if k == 0:
            boost = 3
        total = wrap_i64(total + buckets[k] * boost)
    return total


def compute_nested_stencil() -> int:
    passes, dim, w0, w1 = [3, 6, 1, 2]
    grid = [0, 7, 14, 21, 28, 6, 13, 20, 0, 8, 16, 19,
            26, 6, 15, 19, 28, 3, 10, 20, 25, 6, 11, 16,
            23, 5, 11, 17, 23, 0, 7, 14, 21, 28, 6, 13]
    out = [0] * (dim * dim)
    for _ in range(passes):
        for r in range(dim):
            for c in range(dim):
                acc = 0
                for k in range(9):
                    dr = k // 3 - 1
                    dc = k % 3 - 1
                    rr = min(max(r + dr, 0), dim - 1)
                    cc = min(max(c + dc, 0), dim - 1)
                    coeff = 4 if dr * dr + dc * dc == 0 else (
                        w0 if (dr + dc + 4) % 2 == 0 else w1
                    )
                    acc = wrap_i64(acc + grid[rr * dim + cc] * coeff)
                out[r * dim + c] = (acc + 8) // 10
        grid = out[:]
    return grid[2 * dim + 3]
```

Add these four keys to `DISPATCH`:

```python
"expr_eval": lambda args: compute_expr_eval(),
"window_stats": lambda args: compute_window_stats(),
"record_pipeline": lambda args: compute_record_pipeline(),
"nested_stencil": lambda args: compute_nested_stencil(),
```

These functions evaluate to `14`, `248`, `7466`, and `62` respectively; they
contain no precomputed lookup table.

### 4.4 No other edits required

- `Cargo.toml`: no `[[test]]` entry is needed; Cargo auto-discovers files in
  `tests/`, as it already does for the other integration tests.
- `scripts/make_bench_table.py`: untouched; it still reads
  `bench/data/showdown_results.csv`. The new compile-time CSV should be
  disclosed as a separate cited artifact until Agent A wires it into the table
  generator.
- `README.md`, `SHOWDOWN.md`, `BENCHMARK_CONTRACT.md`: untouched by Agent C.
