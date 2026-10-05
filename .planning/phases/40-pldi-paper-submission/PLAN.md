# Phase 40: PLDI/ICFP Paper Submission & Artifact Package — Plan

> **Phase**: 40
> **Status**: Completed
> **Traceability**: Master Plan §Remediation & Frontier Roadmap, Phase 40
> **Milestone**: Supercompiler Refinements & Rigor (Phases 29–40)

## Objective
Modularize `paper/main.tex` into 9 section files covering all axes of dominance, produce camera-ready ACM formatting, generate SHA-256 checksums, build hermetic Docker reproduction package, and draft reviewer rebuttals.

## Root Cause / Motivation
Preparing a top-tier PLDI/ICFP submission demands rigorous artifact evaluation packaging, modular manuscript layout, verifiable checksum verification, and detailed preemptive rebuttals.

## Requirements
- **SUBMIT-01**: Modularize `paper/main.tex` into `paper/sections/01_introduction.tex` through `09_conclusion.tex` covering 9 axes of dominance.
- **SUBMIT-02**: Generate `acmart.cls`, `ACM-Reference-Format.bst`, and `table_termination.tex`.
- **SUBMIT-03**: Update `references.bib` with complete citations.
- **SUBMIT-04**: Create reproducible `docker/Dockerfile`, `docker/entrypoint.sh`, and top-level `Makefile`.
- **SUBMIT-05**: Generate SHA-256 checksums in `bench/data/checksums.sha256`.
- **SUBMIT-06**: Write comprehensive reviewer rebuttals in `rebuttal/likely_objections.md`.
- **SUBMIT-07**: Create test suite `tests/supercompiler_phase40_tests.rs`.

## Key Deliverables
- `paper/sections/*.tex`
- `docker/Dockerfile`
- `docker/entrypoint.sh`
- `bench/data/checksums.sha256`
- `rebuttal/likely_objections.md`
- `tests/supercompiler_phase40_tests.rs`

## Verification
- `cargo test --test supercompiler_phase40_tests` passes 5/5 tests green.
- Docker container hermetically builds artifact and paper with zero network access during run.
- Checksums match all benchmark results.
