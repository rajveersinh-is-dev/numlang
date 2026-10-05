# Phase 28: Paper Rewrite & Reproducibility Package — Plan

> **Phase**: 28
> **Status**: Completed
> **Traceability**: Master Plan §9, Requirements PAPER-01..PAPER-03
> **Milestone**: Remediation & Frontier (Phases 20–28)

## Objective
Eliminate data fabrication in `paper/main.tex` and deliver a 1-click Docker reproduction package backed by automated SHA-256 data pipeline.

## Root Cause / Motivation
Prior paper drafts contained manual tables disconnected from empirical data. A reproducible, automated evaluation pipeline was required.

## Requirements
- **PAPER-01**: Rewrite `paper/main.tex` with automated SHA-256 data pipeline directly populating tables from `bench/data/results.csv`.
- **PAPER-02**: Accurately describe verified algorithms, honest limitations, and measured speedups without data fabrication.
- **PAPER-03**: Package a hermetic multi-stage Docker container where `make reproduce` compiles `paper/main.pdf` in one command.

## Key Deliverables
- `paper/main.tex`
- `bench/harness/generate_tables.py`
- `docker/Dockerfile`
- `Makefile`

## Verification
- Docker container and `make reproduce` compile `paper/main.pdf` in one command.
- LaTeX tables generated directly from verified `results.csv` with matching SHA-256 checksums.
- Zero fabricated numbers.
