#!/usr/bin/env bash
set -euo pipefail

echo "=== numlang artifact ==="
echo "[1/4] Running benchmark suite..."
python3 bench/harness/runner.py

echo "[2/4] Generating tables..."
python3 bench/harness/generate_tables.py

echo "[3/4] Checking SHA-256 checksums..."
sha256sum -c bench/data/checksums.sha256

echo "[4/4] Compiling paper..."
cd paper && pdflatex -interaction=nonstopmode main.tex && pdflatex -interaction=nonstopmode main.tex
echo "=== Done. See paper/main.pdf ==="
