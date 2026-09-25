#!/usr/bin/env bash
set -euo pipefail

echo "=========================================="
echo "Verifying Lean 4 NumLang Formal Proofs..."
echo "=========================================="

cd "$(dirname "$0")/../proof"

lake build

echo "Scanning for sorry placeholders..."
if grep -rn "sorry" NumLangProofs/ NumLangProofs.lean; then
    echo "ERROR: Proofs contain incomplete 'sorry' placeholders!"
    exit 1
fi

echo "=========================================="
echo "All Lean 4 proofs machine-checked: SUCCESS"
echo "=========================================="
