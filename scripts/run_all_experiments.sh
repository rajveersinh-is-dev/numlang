#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${ROOT_DIR}"

echo "======================================================================"
echo " NumLang Supercompiler: ACM SIGPLAN Empirical Reproduction Pipeline"
echo "======================================================================"
echo "Host OS: $(uname -s) $(uname -m)"
echo "Timestamp: $(date -u)"
echo "Commit: $(git rev-parse --short HEAD 2>/dev/null || echo 'release-artifact')"
echo ""

# Stage 1: Compiler Build & Test Suite Verification
echo "[STAGE 1/6] Running full test suite across all 46+ compiler suites..."
cargo test --tests --release
echo "[PASS] All tests passed with 0 failures."
echo ""

# Stage 2: Formal Verification in Lean 4
echo "[STAGE 2/6] Verifying formal Lean 4 soundness and termination proofs..."
if command -v lake &> /dev/null; then
    (cd proof && lake build)
    echo "[PASS] Lean 4 formal machine proofs verified."
else
    echo "[SKIP] 'lake' not found in PATH; skipping Lean 4 proof build on host."
fi
echo ""

# Stage 3: Criterion Microbenchmarks
echo "[STAGE 3/6] Running Criterion compiler throughput benchmark check..."
cargo bench --bench supercompiler_benchmarks -- --test
echo "[PASS] Criterion benchmarks validated."
echo ""

# Stage 4: Multi-Compiler Canonical Empirical Evaluation
echo "[STAGE 4/6] Executing canonical 10-benchmark literature evaluation..."
echo "             Configurations: NumLang (Base), NumLang (Supercompiled), Rust (-O3), C (Clang -O3)"
echo "             Parameters: 5 warm-up runs, 30 timed iterations, 10,000 bootstrap CIs, CPU pinning"
python3 bench/harness/runner.py
echo "[PASS] Empirical measurements recorded in bench/data/results.csv."
echo ""

# Stage 5: LaTeX Tables & Publication Figures Generation
echo "[STAGE 5/6] Generating LaTeX tables and publication-quality vector figures..."
python3 bench/harness/generate_tables.py
python3 bench/plot.py
echo "[PASS] Generated tables in paper/generated/ and figures in bench/figures/."
echo ""

# Stage 6: Summary Report
echo "======================================================================"
echo "                     EMPIRICAL EVALUATION SUMMARY                     "
echo "======================================================================"
python3 scripts/summary.py
