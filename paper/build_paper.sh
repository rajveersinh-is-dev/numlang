#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}"

echo "Building research paper draft (PEPM '26)..."

if command -v latexmk &> /dev/null; then
    latexmk -pdf -interaction=nonstopmode main.tex
    echo "✓ PDF successfully compiled: paper/main.pdf"
elif command -v pdflatex &> /dev/null; then
    pdflatex -interaction=nonstopmode main.tex
    bibtex main || true
    pdflatex -interaction=nonstopmode main.tex
    pdflatex -interaction=nonstopmode main.tex
    echo "✓ PDF successfully compiled: paper/main.pdf"
else
    echo "⚠ latexmk/pdflatex not found on host."
    echo "You can compile this paper using Docker:"
    echo "  docker run --rm -v \$(pwd):/numlang/paper numlang-artifact bash -c 'cd /numlang/paper && latexmk -pdf main.tex'"
fi
