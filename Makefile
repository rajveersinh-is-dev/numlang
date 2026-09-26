# NumLang Supercompiler: Top-level Reproducibility Makefile
# Compatible with Linux, macOS, and Windows environments

PYTHON ?= python3

.PHONY: build test lean bench tables figures paper reproduce clean-root

build:
	cargo build --release --all-features

test:
	cargo test --tests --release

lean:
	cd proof && lake build

bench: build
	$(PYTHON) bench/harness/runner.py

tables:
	$(PYTHON) bench/harness/generate_tables.py

figures:
	$(PYTHON) bench/plot.py

paper: figures tables
	cd paper && latexmk -pdf -interaction=nonstopmode main.tex

reproduce: build test lean bench tables figures
	@echo "======================================================================"
	@echo "[DONE] Full end-to-end empirical reproduction complete."
	@echo "[INTEGRITY] Cryptographic SHA-256 hashes embedded in paper/generated/*.tex"
	@echo "======================================================================"

clean-root:
	$(PYTHON) -c "import glob, os; [os.remove(f) for f in glob.glob('*.exe') + glob.glob('*.obj') + glob.glob('*.pdb') + ['_panic.nl'] if os.path.exists(f)]"
