# Docker Reproducibility Environment for NumLang Artifact

This container provides a hermetic, fully reproducible environment containing:
- **Rust Toolchain**: `rustc 1.81.0` and `cargo`
- **C/C++ Toolchain**: `Clang 17` and `LLVM 17`
- **Haskell Toolchain**: `GHC` and `Cabal`
- **Proof Assistant**: `Lean 4 (v4.11.0)` with `elan` and `lake`
- **Scientific Python**: Python 3 with `numpy`, `scipy`, `matplotlib`, and `pandas`
- **LaTeX Suite**: `latexmk` and `texlive` for compiling the research paper draft

## Building the Docker Image

From the repository root:
```bash
docker build -t numlang-artifact -f docker/Dockerfile .
```

## Running All Experiments

To execute the entire empirical suite, verification gates, and generate all tables and figures:
```bash
docker run --rm -v $(pwd)/output:/numlang/bench/figures numlang-artifact
```

Or run interactively:
```bash
docker run -it --rm numlang-artifact bash
```
