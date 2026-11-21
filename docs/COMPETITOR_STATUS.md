# Competitor Status (Host Probe)

**Purpose**: Determine which competing compilers/supercompilers can be measured on this host so "world's fastest" claims can include academic/industrial supercompilers instead of `NOT_INSTALLED` placeholders.

**Probe date**: 2026-11-21  
**Host**: Windows 11 (10.0.26200), x86_64 (`win32`)  
**Workspace**: `C:\Users\davea\.gemini\antigravity\scratch\numlang`  
**Probe method**: Read-only PATH/vswhere/vcvars checks; no installs performed. Script: `scripts/detect_competitors.py`.

## Status Table

| Tool | Detected status | Exact invocation / probe | Version / path | Contract flags (showdown) | Install command (Windows, non-interactive where possible) | Blockers / notes | Benchmarks it can run |
|---|---|---|---|---|---|---|---|
| `rustc` (Rustc-O3) | **INSTALLED** | `where.exe rustc`; `rustc -Vv` | `rustc 1.100.0 (0ef1bc386 2026-11-09)`, host `x86_64-pc-windows-msvc`, LLVM 23.1.5; `C:\Users\davea\.cargo\bin\rustc.exe` | `rustc -C opt-level=3 -C codegen-units=1 -o <exe> <src.rs>` (per `bench/harness/runner.py`) / `rustc -C opt-level=3 -o <exe> <src.rs>` (per showdown tests) | Already installed. Refresh/install via: `winget install --id Rustlang.Rustup -e --source winget; rustup default stable` | None. Codegen-units differs between harnesses; equalize before cross-harness comparison. | All 18 showdown benchmarks with `bench/showdown/rust/*.rs`. |
| `cl` / MSVC (MSVC-O2) | **INSTALLED** (via vcvars; not on default PATH) | `vswhere` found BuildTools; `call "...\vcvars64.bat" >nul 2>&1 && cl` | MSVC `19.51.36252` for x64; BuildTools at `C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools`; cl: `...\VC\Tools\MSVC\14.51.36231\bin\Hostx64\x64\cl.exe` | `call "<vcvars64.bat>" >nul 2>&1 && cl.exe /O2 /nologo /Fe:<out.exe> <in.c>` (contract/showdown) | Already installed. Install if missing: `winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive --norestart"` | May require admin/elevation; large download; license/EULA; requires vcvars/dev-shell; not directly on PATH. | All 18 showdown C benchmarks with `bench/showdown/c/*.c`. |
| `clang` (Clang-O3) | **NOT_INSTALLED** | `where.exe clang`; searched VS BuildTools/Program Files/LLVM/MSYS2 | — | Showdown contract/harness label: `clang -O3 -march=native -o <exe> <in.c>` (fallback in `bench/harness/runner.py` when no vcvars) | `winget install --id LLVM.LLVM -e` or `choco install llvm -y` | `winget` not found on PATH; `choco` 2.7.3 present but install may require admin; PATH refresh required; LLVM install can be large. | C showdown benchmarks (`bench/showdown/c/*.c`) if harness invokes clang; current Rust showdown harness detects only, does not compile clang cells. |
| `gcc` (GCC-O3) | **NOT_INSTALLED** | `where.exe gcc`; checked `C:\msys64\mingw64|ucrt64\bin\gcc.exe` | — | Showdown label: `gcc -O3 -march=native -o <exe> <in.c>` (not currently invoked by Rust showdown harness) | `winget install --id MSYS2.MSYS2 -e`; then in MSYS2: `pacman -S --needed --noconfirm mingw-w64-ucrt-x86_64-gcc`; add `C:\msys64\ucrt64\bin` to PATH | Multi-step, not cleanly non-interactive on Windows; MSYS2 install/admin/policy; PATH/manual shell setup; MinGW runtime differs from MSVC CRT. | C showdown benchmarks (`bench/showdown/c/*.c`) if harness adds gcc support. |
| `ghc` (GHC-O2) | **NOT_INSTALLED** | `where.exe ghc` | — | `ghc -O2 <src.hs> -o <exe>` (per contract/showdown) | `winget install --id GHCup.GHCup -e; ghcup install ghc latest --set; ghcup install cabal latest --set` (or install specific version, e.g. 9.8.1 as in SHOWDOWN) | `winget` not found on PATH; `ghcup/cabal/stack` absent; installer may require interactive/policy approval; network/download size; PATH refresh. | Haskell showdown benchmarks where present (`bench/showdown/haskell/*.hs`, 18 files present). |
| `hosc` (HOSC-SC) | **NOT_INSTALLED** | `where.exe hosc` | — | Research supercompiler; SHOWDOWN notes `hosc <prog.hosc> -o <res.hs>` (detection only in current harness) | Install GHC+cabal first, then `cabal update && cabal install hosc` | Academic Haskell package; Windows cabal build reliability uncertain; depends on GHC/cabal; not packaged in winget/choco/scoop; non-interactive feasibility low. | G1 only: `bench/showdown/hosc/{nrev,append3,kmp,peano_mul,tree_flip}.hosc` (5 files); no G2/G3 HOSC sources registered. |
| `spsc` (SPSC) | **NOT_INSTALLED** | `where.exe spsc` | — | Detected by harness; no compile/invoke path in current showdown runner | No standard Windows package identified. Likely requires source build from academic repository; toolchain depends on upstream distribution. | Not in winget/choco/scoop; source/location/toolchain ambiguous; non-interactive install not feasible without confirmed upstream artifact. | No SPSC sources registered in `bench/showdown/benchmarks.toml`; detection-only unless sources/harness support added. |
| `lean` | **INSTALLED** | `where.exe lean`; `lean --version` | `Lean (version 4.36.0, x86_64-w64-windows-gnu, commit 611c117bfaf7b7ae1cfcf246ce6fba31e8f509e9, Release)`; `C:\Users\davea\.elan\bin\lean.exe` | N/A (proof toolchain) | Already installed via elan. | None. | Proof verification (`lake build` in `proof/`), not runtime showdown. |
| `lake` | **INSTALLED** | `where.exe lake`; `lake --version` | `Lake version 5.0.0-src+611c117 (Lean version 4.36.0)`; `C:\Users\davea\.elan\bin\lake.exe` | N/A (proof toolchain) | Installed with Lean/elan. | None. | Proof verification (`lake build` in `proof/`), not runtime showdown. |

## Package Manager / Toolchain Availability

| Tool | Status | Notes |
|---|---|---|
| `winget` | **NOT FOUND** on PATH / common alias probe failed | SHOWDOWN suggests winget for GHCup; verify App Execution Alias or install App Installer. |
| `choco` | **INSTALLED** (`2.7.3`, `C:\ProgramData\chocolatey\bin\choco.exe`) | May require admin for installs. |
| `scoop` | **NOT FOUND** | — |
| `ghcup` | **NOT FOUND** | Needed for GHC/cabal/stack managed install. |
| `cabal` | **NOT FOUND** | Needed for HOSC/SPSC Haskell builds. |
| `stack` | **NOT FOUND** | Alternative Haskell build tool. |

## Flag Inequivalences and Equalization (BENCHMARK_CONTRACT.md)

BENCHMARK_CONTRACT.md §3 requires equalized optimization levels for cross-compiler runtime comparisons and discloses the existing `Rustc-O3` (`-C opt-level=3`) vs `MSVC-O2` (`/O2`) disparity as a deviation to resolve for new claims.

| Comparison | Current flags | Inequivalence | Proposed equalized flags | Notes |
|---|---|---|---|---|
| Rustc vs MSVC | `rustc -C opt-level=3` vs `cl.exe /O2 /nologo` | Different compiler families and optimization level naming; not 1:1 equivalent. | **Option A (match contract as-is, disclose):** keep current and label as deviation until resolved. **Option B (equalize conservative):** `rustc -C opt-level=2` vs `cl.exe /O2 /nologo` for a closer "O2-class" comparison, with separate `-O3` run disclosed. **Option C (max-opt, disclose):** `rustc -C opt-level=3 -C target-cpu=native` vs `cl.exe /O2 /GL` (LTCG) — only if explicitly labeled non-portable/native. | For new "world's fastest" claims, pick one policy before measurement, record it in contract/evidence, and do not mix policies in one table. |
| MSVC vs Clang | `cl.exe /O2 /nologo` vs `clang -O3 -march=native` | `-march=native` is host-specific and gives Clang an architectural tuning advantage; `-O3` vs `/O2` differs. | Equalized baseline: `cl.exe /O2 /nologo` vs `clang -O2 -fno-fast-math` (or `-O3` only with same level across all C compilers). Add `-march=native` only in a separately labeled native-tuned column. | `bench/harness/runner.py` currently falls back to `clang -O3 -march=native` when no vcvars — this is not equalized to MSVC `/O2`. |
| MSVC vs GCC | `cl.exe /O2 /nologo` vs `gcc -O3 -march=native` | Same as Clang: level + `-march=native` + CRT/runtime differences. | `cl.exe /O2 /nologo` vs `gcc -O2 -fno-fast-math`; native-tuned variant separate. | GCC on Windows usually means MinGW; CRT/ABI differences can affect wall-clock/runtime behavior. |
| Rustc vs GHC | `rustc -C opt-level=3` vs `ghc -O2` | Different languages/runtimes; GHC has RTS/GC overhead; `-O2` is GHC's high optimization level but not equivalent to LLVM `-O3`. | Keep as labeled cross-language comparison: `Rustc-O3` vs `GHC-O2`. Do not treat as same-language equalized compiler comparison. | Report runtime model differences (GC, RTS, lazy evaluation) alongside numbers. |
| NumLang-SC vs HOSC/SPSC | Supercompiler pipelines, not just flags | Academic supercompilers transform source; measurement must include compile+runtime or clearly separate them. HOSC may emit Haskell requiring GHC. | Report pipeline explicitly: `NumLang: build --supercompile --bench`; `HOSC: hosc <in> -o <out.hs> && ghc -O2 <out.hs> -o <exe>`; `SPSC: spsc ...` per upstream. | Compare only where equivalent benchmark sources exist; HOSC currently has 5 G1 sources. SPSC has no registered sources. |

## Concrete Install Next Actions

1. **MSVC (already present)**: Use vcvars before C benchmark runs: `call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat"`. No install needed.
2. **Clang (low friction if choco admin ok)**: `choco install llvm -y` (admin). Verify with `clang --version`. If non-admin required, use winget if App Installer becomes available, or portable LLVM zip + PATH.
3. **GCC (moderate friction)**: Install MSYS2, then `pacman -S --needed --noconfirm mingw-w64-ucrt-x86_64-gcc`, add `C:\msys64\ucrt64\bin` to PATH. Avoid unless MSVC/Clang already provide C comparison coverage.
4. **GHC (moderate friction)**: Install GHCup, then `ghcup install ghc 9.8.1 --set && ghcup install cabal latest --set`. Confirm with `ghc --version`. Needed before HOSC.
5. **HOSC (high friction, low confidence on Windows)**: After GHC+cabal, try `cabal update && cabal install hosc`. If Windows build fails, use WSL/Linux container or document as unavailable-on-Windows with exact blocker.
6. **SPSC (blocked/ambiguous)**: Identify canonical upstream source/binary first. Do not claim installability until a reproducible Windows command exists. Treat as `NOT_INSTALLED / NO_REPRODUCIBLE_WINDOWS_INSTALL` until resolved.
7. **Harness gap**: `tests/supercompiler_showdown.rs` detects Clang/GCC but does not compile/run them; `bench/harness/runner.py` only falls back to clang when vcvars absent. To include clang/gcc in measured showdown, add explicit compile/run cells and record flags in results.

## Scope / Integrity

- This report is a host capability probe, not a benchmark result.
- No compilers were installed; no benchmark workloads were executed.
- `NOT_INSTALLED` means not detected by this probe on current PATH/common locations; it does not prove absence on all disks.
- Existing benchmark claims remain governed by `BENCHMARK_CONTRACT.md` and evidence files.