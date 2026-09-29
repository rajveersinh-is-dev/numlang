# NumLang Supercompiler: Top-level Reproducibility Makefile
# Targets: build, test, reproduce, paper, artifact-docker

PYTHON ?= python3

.PHONY: build test reproduce paper artifact-docker clean

build:
	cargo build --release

test:
	cargo test

reproduce:
	$(PYTHON) bench/harness/runner.py
	$(PYTHON) bench/harness/generate_tables.py
	sha256sum -c bench/data/checksums.sha256

paper:
	cd paper && pdflatex -interaction=nonstopmode main.tex && pdflatex -interaction=nonstopmode main.tex

artifact-docker:
	docker build -f docker/Dockerfile -t numlang-artifact .
	docker run --rm numlang-artifact

clean:
	cargo clean
	rm -f bench/data/*.tmp
