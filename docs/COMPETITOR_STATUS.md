# Competitor Status (Host Probe + Measured Showdown Update)

**Purpose**: Determine which competing compilers/supercompilers can be measured on this host so "world's fastest" claims can include academic/industrial supercompilers instead of `NOT_INSTALLED` placeholders.

**Probe date**: 2026-11-21 (original read-only probe)  
**Toolchain installation update**: 2026-11-21 — Clang, GCC and GHC are now **installed via user-level Scoop** and are measured (not merely detected) by the showdown harness.  
**Host**: Windows 11 (10.0.26200), x86_64 (`win32`)  
**Workspace**: `C:\Users\davea\.gemini\antigravity\scratch\numlang`  
**Probe method**: PATH/vswhere/vcvars checks plus a real FULL-mode harness run; no admin rights used. Original probe script: `scripts/detect_competitors.py`.

## Status Table

| Tool | Detected status | Exact invocation / probe | Version / path | Contract flags (showdown) | Install command (Windows, non-interactive where possible) | Blockers / notes | Benchmarks it can run |
|---|---|---|---|---|---|---|---|
| `rustc` (Rustc-O3) | **INSTALLED** | `where.exe rustc`; `rustc -Vv` | `rustc 1.100.0 (0ef1bc386 2026-11-09)`, host `x86_64-pc-windows-msvc`, LLVM 23.1.5; `C:\Users\davea\.cargo\bin\rustc.exe` | `rustc -C opt-level=3 -o <exe> <src.rs>` (showdown tests) | Already installed. Refresh/install via: `winget install --id Rustlang.Rustup -e --source winget; rustup default stable` | None. Codegen-units differs between harnesses; equalize before cross-harness comparison. | All 22 showdown benchmarks with `bench/showdown/.../rust/*.rs`. |
| `cl` / MSVC (MSVC-O2) | **INSTALLED** (via vcvars; not on default PATH) | `vswhere` found BuildTools; `call "...\vcvars64.bat" >nul 2>&1 && cl` | MSVC `19.51.36252` for x64; BuildTools at `C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools`; cl: `...\VC\Tools\MSVC\14.51.36231\bin\Hostx64\x64\cl.exe` | `call "<vcvars64.bat>" >nul 2>&1 && cl.exe /O2 /nologo /Fe:<out.exe> <in.c>` | Already installed. | Requires vcvars/dev-shell; not directly on PATH. | All 22 showdown C benchmarks with `bench/showdown/.../c/*.c`. |
| `clang` (Clang-O3) | **INSTALLED — NOW MEASURED** | `clang --version`; harness compiles+runs it | `clang version 23.1.6 (https://github.com/llvm/llvm-project 5b10d2d74929eae78912771222c9855af97496f4)`, Target `x86_64-pc-windows-msvc`, Thread model `posix`, InstalledDir `C:\Users\davea\scoop\apps\llvm\23.1.6\bin`; shim on PATH via `C:\Users\davea\scoop\shims` and `C:\Users\davea\scoop\apps\llvm\current\bin` | **`clang -O3 -march=native -o <exe> <in.c>`** — exact flag used by `compile_c_native()` in `tests/supercompiler_showdown.rs`, recorded as system `Clang-O3` in the CSV | Installed via user-level Scoop: `scoop install llvm` (no admin). | MSVC target links through `link.exe` auto-detected from VS BuildTools; verified standalone compile+run. `-march=native` is host-specific (non-portable) and is disclosed as such. | All 22 showdown C benchmarks (`bench/showdown/c/*.c` + `bench/showdown/extended/c/*.c`). |
| `gcc` (GCC-O3) | **INSTALLED — NOW MEASURED** | `gcc --version`; harness compiles+runs it | `gcc.exe (x86_64-posix-seh-rev1, Built by MinGW-Builds project) 16.2.0`; shim on PATH via `C:\Users\davea\scoop\shims` and `C:\Users\davea\scoop\apps\mingw\current\bin` | **`gcc -O3 -march=native -o <exe> <in.c>`** — exact flag used by `compile_c_native()` in `tests/supercompiler_showdown.rs`, recorded as system `GCC-O3` in the CSV | Installed via user-level Scoop: `scoop install mingw` (no admin). | MinGW CRT/ABI differs from MSVC CRT; `-march=native` is host-specific and disclosed. | All 22 showdown C benchmarks (`bench/showdown/c/*.c` + `bench/showdown/extended/c/*.c`). |
| `ghc` (GHC-O2) | **INSTALLED — NOW MEASURED** | `ghc --version`; harness compiles+runs it | `The Glorious Glasgow Haskell Compilation System, version 9.14.1`; shim on PATH via `C:\Users\davea\scoop\shims` and `C:\Users\davea\scoop\apps\haskell\current\lib\bin` | **`ghc -O2 -o <exe> <src.hs>`** — exact flag used by `compile_ghc()` in `tests/supercompiler_showdown.rs`, recorded as system `GHC-O2` in the CSV | Installed via user-level Scoop: `scoop install haskell` (no admin). | 18 registered `bench/showdown/haskell/*.hs` sources compile. The 4 `G4: Extended Realistic` benchmarks have **no** `.hs` source registered, so they are correctly `NOT_INSTALLED`, never fabricated. 3 sources fail the correctness gate because GHC maps `exitWith (ExitFailure 0)` to process exit code 1 (`pow2_mod`, `pow2_mod_dyn`, `stream_take`, expected exit 0) — recorded as `WRONG_OUTPUT(actual=1;expected=0)`. | 18 of 22 showdown benchmarks with `bench/showdown/haskell/*.hs`. |
| `hosc` (HOSC-SC) | **NOT INSTALLABLE ON THIS HOST (attempted 2026-11-21)** | `where.exe hosc`; source `github.com/ilya-klyuchnikov/hosc` cloned; `mvn` build attempted | Real upstream is **Scala 2.8 / Maven**, not a Hackage Haskell package | `hosc <prog.hosc> -o <res.hs>` (detection only in harness; no compile/invoke path) | Attempted: user-level Scoop `maven`, `openjdk8-redhat`, `openjdk7-unofficial`, legacy Maven 3.3.9 from archive.apache.org, Hackage `cabal install hosc` | **Blockers (verified):** Hackage `hosc` 0.21.1 is "Haskell Open Sound Control" (unrelated audio library). Upstream builds with Scala 2.8.0, which fails on Java 23 (`Missing dependency class java.lang.Object`) and on Java 8 (`broken class file rt.jar`); Java 7 + Maven 3.3.9 fails TLS PKIX validation against modern Maven Central even with `maven.wagon.http.ssl.insecure`; a system-configured mirror returned HTML POMs. No reproducible Windows build found. | G1 only: `bench/showdown/hosc/{nrev,append3,kmp,peano_mul,tree_flip}.hosc` (5 files); no G2/G3/G4 HOSC sources registered. |
| `spsc` (SPSC) | **NOT_INSTALLED** | `where.exe spsc` | — | Detected by harness; no compile/invoke path in current showdown runner | No standard Windows package identified. Likely requires source build from academic repository. | Not in winget/choco/scoop; source/location/toolchain ambiguous; non-interactive install not feasible without confirmed upstream artifact. | No SPSC sources registered in `bench/showdown/benchmarks.toml`; detection-only unless sources/harness support added. |
| `lean` | **INSTALLED** | `where.exe lean`; `lean --version` | `Lean (version 4.36.0, x86_64-w64-windows-gnu, commit 611c117bfaf7b7ae1cfcf246ce6fba31e8f509e9, Release)`; `C:\Users\davea\.elan\bin\lean.exe` | N/A (proof toolchain) | Already installed via elan. | None. | Proof verification (`lake build` in `proof/`), not runtime showdown. |
| `lake` | **INSTALLED** | `where.exe lake`; `lake --version` | `Lake version 5.0.0-src+611c117 (Lean version 4.36.0)`; `C:\Users\davea\.elan\bin\lake.exe` | N/A (proof toolchain) | Installed with Lean/elan. | None. | Proof verification (`lake build` in `proof/`), not runtime showdown. |

## Package Manager / Toolchain Availability

| Tool | Status | Notes |
|---|---|---|
| `winget` | **NOT FOUND** on PATH / common alias probe failed | SHOWDOWN suggests winget for GHCup; verify App Execution Alias or install App Installer. |
| `choco` | **INSTALLED** (`2.7.3`, `C:\ProgramData\chocolatey\bin\choco.exe`) | May require admin for installs. |
| `scoop` | **INSTALLED (user-level)** — `C:\Users\davea\scoop` | Used without admin to install `llvm`, `mingw`, `haskell`. Shims: `C:\Users\davea\scoop\shims`. |
| `ghcup` | **NOT FOUND** | Not needed: GHC 9.14.1 supplied by Scoop `haskell`. |
| `cabal` | **INSTALLED** (`haskell-cabal` 3.18.2.0 via user-level Scoop) | Hackage `hosc` proved to be an unrelated audio library; HOSC upstream is Scala/Maven, so cabal does not unblock it. SPSC remains source-ambiguous. |
| `stack` | **NOT FOUND** | Alternative Haskell build tool; not needed for this showdown. |

## Flag Inequivalences and Equalization (BENCHMARK_CONTRACT.md)

BENCHMARK_CONTRACT.md §3 requires equalized optimization levels for cross-compiler runtime comparisons and discloses the existing `Rustc-O3` (`-C opt-level=3`) vs `MSVC-O2` (`/O2`) disparity as a deviation to resolve for new claims.

| Comparison | Current flags | Inequivalence | Proposed equalized flags | Notes |
|---|---|---|---|---|
| Rustc vs MSVC | `rustc -C opt-level=3` vs `cl.exe /O2 /nologo` | Different compiler families and optimization level naming; not 1:1 equivalent. | **Option A (match contract as-is, disclose):** keep current and label as deviation until resolved. **Option B (equalize conservative):** `rustc -C opt-level=2` vs `cl.exe /O2 /nologo` for a closer "O2-class" comparison, with separate `-O3` run disclosed. **Option C (max-opt, disclose):** `rustc -C opt-level=3 -C target-cpu=native` vs `cl.exe /O2 /GL` (LTCG) — only if explicitly labeled non-portable/native. | For new "world's fastest" claims, pick one policy before measurement, record it in contract/evidence, and do not mix policies in one table. |
| MSVC vs Clang | `cl.exe /O2 /nologo` vs `clang -O3 -march=native` | `-march=native` is host-specific and gives Clang an architectural tuning advantage; `-O3` vs `/O2` differs. | Equalized baseline: `cl.exe /O2 /nologo` vs `clang -O2 -fno-fast-math`; add `-march=native` only in a separately labeled native-tuned column. | The `Clang-O3` column is the native-tuned variant; this is disclosed, not equalized to MSVC. |
| MSVC vs GCC | `cl.exe /O2 /nologo` vs `gcc -O3 -march=native` | Same as Clang: level + `-march=native` + CRT/runtime differences. | `cl.exe /O2 /nologo` vs `gcc -O2 -fno-fast-math`; native-tuned variant separate. | MinGW CRT/ABI differences can affect wall-clock/runtime behavior. |
| Clang vs GCC | `clang -O3 -march=native` vs `gcc -O3 -march=native` | Same flag level, different code generators and CRT (MSVC-targeted Clang vs MinGW GCC). | Keep as labeled cross-compiler comparison with identical optimization flags. | Both compile the same C sources; differences are optimizer/runtime, not flag-level. |
| Rustc vs GHC | `rustc -C opt-level=3` vs `ghc -O2` | Different languages/runtimes; GHC has RTS/GC overhead; `-O2` is GHC's high optimization level but not equivalent to LLVM `-O3`. | Keep as labeled cross-language comparison: `Rustc-O3` vs `GHC-O2`. | Report runtime model differences (GC, RTS, lazy evaluation) alongside numbers. |
| NumLang-SC vs HOSC/SPSC | Supercompiler pipelines, not just flags | Academic supercompilers transform source; measurement must include compile+runtime or clearly separate them. HOSC may emit Haskell requiring GHC. | Report pipeline explicitly: `NumLang: build --supercompile --bench`; `HOSC: hosc <in> -o <out.hs> && ghc -O2 <out.hs> -o <exe>`; `SPSC: spsc ...` per upstream. | HOSC/SPSC remain NOT_INSTALLED; no such cells were fabricated. |

## Measured FULL-mode run (2026-11-21)

Exact command (scoop PATH entries prepended; quick-mode env vars unset):

```powershell
$env:PATH = "C:\Users\davea\scoop\shims;C:\Users\davea\scoop\apps\llvm\current\bin;C:\Users\davea\scoop\apps\mingw\current\bin;C:\Users\davea\scoop\apps\haskell\current\lib\bin;" + $env:PATH
cargo test --test supercompiler_showdown -- --nocapture
```

- Mode logged by harness: `FULL (Scientific Standard: 30 rounds, 5 warmups)`; result `2 passed; 0 failed`.
- Harness-detected toolchains: Clang-O3, GCC-O3, GHC-O2, MSVC-O2, Rustc-O3 **FOUND**; HOSC-SC, SPSC **NOT_FOUND**.
- CSV now contains **8 systems × 22 benchmarks = 176 cells**, including new `Clang-O3`, `GCC-O3` rows and **real** `GHC-O2` measurements on 18 benchmarks.
- `python scripts/make_bench_table.py --all` regenerated README.md/SHOWDOWN.md from that CSV; `--check` passed; `python scripts/ci_regression_guard.py` reported **0 regressions**.
- See `docs/BENCHMARK_ANALYSIS.md` for the CSV hash/vintage and per-system competitive standings.

## Concrete Install Next Actions (updated)

1. **MSVC / Rustc / Clang / GCC / GHC**: all installed and exercised — no install action remains for measured systems. Use the PATH prefix above for Clang/GCC/GHC; MSVC still requires vcvars64.bat (harness invokes it internally).
2. **HOSC (blocked on this host — verified attempt)**: `cabal install hosc` installs an unrelated audio library. The real upstream (`ilya-klyuchnikov/hosc`) needs Scala 2.8, which cannot run on Java 23, cannot read Java 8 class files, and Java 7's TLS cannot validate modern Maven Central. A reproducible Windows build requires a container/WSL with a legacy JDK 6/7 + Maven 3.3 and a trusted certificate store. None of this is available here without admin/container support, so no HOSC cells were fabricated.
3. **SPSC (blocked/ambiguous)**: identify canonical upstream source/binary first. Do not claim installability until a reproducible Windows command exists. Treat as `NOT_INSTALLED / NO_REPRODUCIBLE_WINDOWS_INSTALL` until resolved.
4. **Harness gap resolved**: `tests/supercompiler_showdown.rs` now compiles and runs Clang (`clang -O3 -march=native`), GCC (`gcc -O3 -march=native`) and GHC (`ghc -O2`) cells and records their status in the CSV.

## Scope / Integrity

- This report documents a host capability probe **and** a real FULL-mode measurement run; numbers live in `bench/data/showdown_results.csv` and the regenerated `README.md` / `SHOWDOWN.md`.
- No compilers were installed by this document's original probe; Clang/GCC/GHC were installed separately via user-level Scoop before the measured run.
- `NOT_INSTALLED` means no registered source or no detected tool — it never stands in for a measurement.
- All benchmark claims remain governed by `BENCHMARK_CONTRACT.md` and the CSV evidence file.
