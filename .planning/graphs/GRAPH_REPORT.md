# Graph Report - numlang  (2026-10-01)

## Corpus Check
- 220 files · ~270,847 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 97 file(s) not represented in the graph (top: .nl 36, .tex 24, .lean 14)

## Summary
- 3244 nodes · 8005 edges · 192 communities (160 shown, 32 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 363 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `77b0e7ab`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- TypedExpr
- BvExpr
- drive.rs
- Token
- TermInterner
- TypeError
- tokenize
- MirProgram
- command
- ScopeEnvironment
- .compile_mir_statement
- TypedProgram
- mrsc.rs
- polyhedral.rs
- tokenize
- mir/supercompiler/mod.rs
- generalization.rs
- main.rs
- Type
- TypedBlock
- SymTermId
- Span
- MirFunction
- Expr
- memory_ssa.rs
- bce.rs
- ast.rs
- Place
- BasicBlockId
- while_unroll.rs
- TypedStmt
- mir_tests.rs
- polyhedral_and_validation_tests.rs
- supercompiler_phase34_tests.rs
- NumLang Adversarial Audit: Critical Vulnerabilities, Soundness Gaps & Engineering Traps
- stdint
- supercompile_program
- cache.rs
- 3. Statements and Control Flow
- link_executable
- loop_opt.rs
- Value
- runner.py
- Parser<'a>
- IrLowerer
- Terminator
- Env
- generate_tables.py
- SymExpr
- IrOp
- CompilerDiagnostic
- ParseError
- collections
- Remediation & Frontier Roadmap (Phases 20–28) [ACTIVE]
- BinaryOp
- monomorphize.rs
- Part I: Production Compiler Baseline (Phases 1–9)
- Part II: Core Supercompiler & Language Extensions (Phases 10–19)
- Pre-Written Rebuttals to Likely Reviewer Objections
- benchmark_harness.rs
- math_primitives_tests.rs
- multi_language_benchmarks.rs
- plot.py
- binaryop
- supercompiler_head_to_head.rs
- Part III: Remediation & Frontier Supercompilation (Phases 20–28)
- supercompiler_symbolic_tests.rs
- supercompiler_phase30_tests.rs
- NumLang: A Formally Verified, Self-Applicable Process-Tree Supercompiler
- Interval
- enum_tests.rs
- Formal Correctness and Certified Termination of the NumLang Supercompiler
- Remediation & Frontier Requirements
- Artifact Evaluation & Reproducibility Guide: NumLang
- opt/supercompiler/mod.rs
- array_math_tests.rs
- math_advanced_tests.rs
- coupled_recurrence_and_fusion_tests.rs
- mir
- time
- parse_expr_str
- deforestation_tests.rs
- ResidualObjective
- recursion.rs
- stdlib_tests.rs
- supercompiler_phase38_tests.rs
- List
- Peano
- Instruction
- match_tests.rs
- struct_tests.rs
- supercompiler_phase39_tests.rs
- unsigned_type_tests.rs
- NumLang Compiler Backends: Cranelift vs. LLVM
- peano_mul.c
- List
- List
- mir_codegen_tests.rs
- LLVM Toolchain Setup Guide for NumLang
- Implementation Tasks
- NumLang Compiler
- .parse_prefix
- generate_random_program
- supercompiler_phase35_tests.rs
- mrsc_lattice_tests.rs
- bit_intrinsics_tests.rs
- generic_tests.rs
- llvm_vs_cranelift_benchmarks.rs
- supercompiler_tests.rs
- double_nrev.c
- kmp.c
- List
- Tree
- differential_fuzz_100k.rs
- independence.rs
- benchmark_correctness_tests.rs
- io_tests.rs
- translation_validation_smt_tests.rs
- NumLang Agent Handoff Document
- differential_correctness_tests.rs
- nrev.c
- tree_flip.c
- supercompiler_phase32_tests.rs
- NumLang Academic & Engineering Integrity Rules
- memory_ssa_tests.rs
- Operand
- ir/mod.rs
- supercompiler_phase40_tests.rs
- true_futamura_projections_tests.rs
- append3.c
- Key Findings
- NumLang Roadmap & Technical Architecture
- explain.rs
- higher_order_tests.rs
- stream_fusion.c
- Critical Pitfalls & Prevention Strategies
- STACK.md
- Project State
- Phase 20: Fix Core Residualization, Back-Edge Knot Transfers & Textbook MSG
- Phase 23: Real Polyhedral Loop & Stencil Deforestation with Buffer Contraction
- Phase 24: Formal SMT-Based Translation Validation & Simulation Preorder
- Phase 25: Genuine Self-Applicable Specializer MinSpec.nl for 2nd and 3rd Futamura
- Phase 26: Rigorous Lean 4 Formal Verification (Zero Axioms, Recursive Semantics)
- Phase 27: Honest High-Precision Benchmarks & Direct Supercompiler Comparisons
- Phase 28: Paper Rewrite & 1-Click Reproducible Artifact Package
- Phase 1: Master Refactoring & Baseline Setup
- Phase 10: Turchin Supercompiler Core, ADTs & 1st Futamura Projection
- Phase 2: Zero-Warning Clippy Cleanliness
- stream_fusion.rs
- Feature Taxonomy
- Phase 5: Low-Bitwidth Signed Types (i8 and i16)
- Phase 7: Pattern Matching (Match Expressions)
- Phase 9: Built-in Standard Library (std)
- Phase 14: Self-Applicable Specializer Prototype & Multistage Futamura
- Phase 15: Differential Fuzzing (100k Cases) & Lean 4 Setup
- Phase 17: Multi-Stage Docker Artifact Packaging & PEPM Paper Draft
- Phase 18: Global Process-Tree Distillation & MRSC Interface
- Phase 19: Polyhedral Stencils, Translation Validation & Parallel Driving
- fib_matrix.rs
- matvec_4x4.rs
- Docker Reproducibility Environment for NumLang Artifact
- Phase 2.1 Plan: MemorySSA Core & Graph Construction
- Phase 2.1 Summary: MemorySSA Core & Graph Construction
- Phase 2.2 Plan: Points-To & Field-Sensitive Alias Analysis
- Phase 2.2 Summary: Points-To & Field-Sensitive Alias Analysis
- Phase 2.3 Plan: Mem2Reg Promotion Engine & Memory Optimizations
- Phase 2.3 Summary: Mem2Reg Promotion Engine & Memory Optimizations
- Phase 2.4 Plan: CLI Tooling, Integration & Verification Gate
- Phase 2.4 Summary: CLI Tooling, Integration & Verification Gate
- ackermann.rs
- power_spec.rs
- raytracer_sphere.rs
- sieve.rs
- supercompiler_benchmarks.rs
- Phase 20 Summary: Core Residualization, Back-Edge Knot Transfers & Textbook MSG
- ARCHITECTURE.md
- delete
- entrypoint.sh
- build_paper.sh
- PHASES_36_40.md
- run_all_experiments.sh script
- verify_proofs.sh script
- numlang

## God Nodes (most connected - your core abstractions)
1. `TypedExpr` - 115 edges
2. `tokenize()` - 104 edges
3. `TypedBlock` - 91 edges
4. `typecheck()` - 86 edges
5. `Span` - 76 edges
6. `Token` - 74 edges
7. `MirFunction` - 71 edges
8. `SymTermId` - 68 edges
9. `Place` - 59 edges
10. `TermInterner` - 56 edges

## Surprising Connections (you probably didn't know these)
- `Author Response` --references--> `double_nrev()`  [INFERRED]
  rebuttal/likely_objections.md → bench/c/double_nrev.c
- `Remediation & Frontier Roadmap (Phases 20–40)` --references--> `generate_head_to_head_table()`  [INFERRED]
  ROADMAP.md → bench/harness/generate_tables.py
- `Remediation & Frontier Roadmap (Phases 20–40)` --references--> `generate_ablation_table()`  [INFERRED]
  ROADMAP.md → bench/harness/generate_tables.py
- `Remediation & Frontier Roadmap (Phases 20–40)` --references--> `BenchmarkEntry`  [INFERRED]
  ROADMAP.md → bench/harness/runner.py
- `test_polyhedral_affine_algebra_and_inequalities()` --calls--> `BasicBlockId`  [INFERRED]
  tests/polyhedral_stencil_tests.rs → src/mir/mod.rs

## Import Cycles
- 2-file cycle: `src/fmt.rs -> src/typecheck/types.rs -> src/fmt.rs`
- 3-file cycle: `src/fmt.rs -> src/typecheck/typed_ast.rs -> src/typecheck/types.rs -> src/fmt.rs`

## Communities (192 total, 32 thin omitted)

### Community 0 - "TypedExpr"
Cohesion: 0.05
Nodes (79): all_assignments_are_nonneg_in_block(), all_assignments_are_u32_in_block(), CodegenError, BackendError, collect_bounds_in_block(), collect_constant_divisors_block(), collect_constant_divisors_expr(), collect_constant_divisors_stmt() (+71 more)

### Community 1 - "BvExpr"
Cohesion: 0.06
Nodes (57): BitBlaster, BoolFormula, And, Eq, False, Implies, Not, Or (+49 more)

### Community 2 - "drive.rs"
Cohesion: 0.08
Nodes (27): DriverConfig, evaluate_comparison_intervals(), flip_relational_op(), get_combinations(), invert_relational_op(), narrow_condition_intervals(), narrow_single_var(), narrow_two_vars() (+19 more)

### Community 3 - "Token"
Cohesion: 0.03
Nodes (64): parse_doc_comment(), parse_string_literal(), Token, Ampersand, Arrow, Assign, Box_, Break (+56 more)

### Community 4 - "TermInterner"
Cohesion: 0.09
Nodes (27): fold_const_binary(), fold_const_unary(), format_place_str(), is_one(), is_zero(), SymTerm, Binary, Call (+19 more)

### Community 5 - "TypeError"
Cohesion: 0.07
Nodes (33): collect_free_variables(), EnumInfo, EnumVariantInfo, StructInfo, TypeChecker, TypeError, ArityMismatch, ArrayElementMismatch (+25 more)

### Community 6 - "tokenize"
Cohesion: 0.08
Nodes (43): lower_to_ir(), LexError, InvalidToken, tokenize(), typecheck(), test_bce_modulo_and_bitand_safety(), test_bce_multi_variable_affine_safety(), test_constant_index_bce() (+35 more)

### Community 7 - "MirProgram"
Cohesion: 0.11
Nodes (32): MirProgram, apply_composition_to_caller(), CompositionCandidate, DistillationEngine<'a>, find_composition_candidates(), GlobalBranchArm, GlobalNodeId, GlobalNodeKind (+24 more)

### Community 8 - "command"
Cohesion: 0.05
Nodes (18): run_numlang_code(), test_box_in_enum(), test_box_roundtrip(), run_numlang_code(), test_horner_constant_mod_7(), test_nonneg_constant_modulo_prime(), test_nonneg_power_of_two_mod_and_div(), test_signed_negative_modulo_and_div() (+10 more)

### Community 9 - "ScopeEnvironment"
Cohesion: 0.07
Nodes (24): det_bareiss(), detect_nway_linear_system(), gcd(), mat_mul_nxn(), mat_pow_nxn(), NWayLinearSystem, solve_cramer(), solve_gaussian_integer() (+16 more)

### Community 10 - ".compile_mir_statement"
Cohesion: 0.11
Nodes (13): compile_mir_to_obj(), LlvmCompiler, LlvmError, BackendDisabled, CodegenError, IoError, TargetInitError, TypeLoweringError (+5 more)

### Community 11 - "TypedProgram"
Cohesion: 0.10
Nodes (40): optimize_program(), block_calls_function(), block_contains_transcendentals(), calls_external_impure(), collect_assigned_names(), collect_assigned_names_in_block(), collect_var_modulos(), compute_iters_for_step() (+32 more)

### Community 12 - "mrsc.rs"
Cohesion: 0.09
Nodes (21): CandidateMetrics, ConfigurationHypergraph, CustomWeightedObjective, HyperAction, BranchSplit, CallInline, DistillationDeforestation, DriveStep (+13 more)

### Community 13 - "polyhedral.rs"
Cohesion: 0.12
Nodes (21): AffineExpr, ArrayAccess, DependenceDistance, extract_affine_expr(), extract_array_accesses(), extract_iteration_domains(), find_fusible_pipelines(), fuse_and_contract_buffer() (+13 more)

### Community 14 - "tokenize"
Cohesion: 0.06
Nodes (17): run_numlang_code(), test_bitwise_typecheck_rejection_for_floats(), test_branchless_bsearch_kernel_execution(), test_branchless_conditional_swap_execution(), rejects_break_outside_loop(), run_numlang_code(), test_binary_search_bce_midpoint_is_safe(), test_while_true_loop_execution() (+9 more)

### Community 15 - "mir/supercompiler/mod.rs"
Cohesion: 0.08
Nodes (28): compact_mir_function(), compact_process_tree(), record_place_uses(), record_rvalue_uses(), record_terminator_uses(), run_copy_propagation_pass(), run_noop_removal_pass(), SupercompilerStats (+20 more)

### Community 16 - "generalization.rs"
Cohesion: 0.13
Nodes (36): all_vars_equal(), binom_float(), binom_int(), binom_mod(), classify_var_periodicity(), detect_whole_period(), evaluate_var_period_model(), fit_coupled_linear_recurrence() (+28 more)

### Community 17 - "main.rs"
Cohesion: 0.10
Nodes (30): Backend, Cranelift, Llvm, build_executable(), Cli, Commands, Build, Check (+22 more)

### Community 18 - "Type"
Cohesion: 0.05
Nodes (24): ClosureType, Type, Array, Bool, Box, Closure, Enum, F32 (+16 more)

### Community 19 - "TypedBlock"
Cohesion: 0.15
Nodes (37): block_contains_loop(), block_terminates_with_return(), collect_callees_block(), collect_callees_expr(), collect_local_names(), contains_return(), count_returns_block(), count_returns_stmt() (+29 more)

### Community 20 - "SymTermId"
Cohesion: 0.11
Nodes (19): anti_unify(), det_3x3(), GeneralizationResult, mat_mul_2x2(), mat_mul_3x3(), mat_pow_2x2(), mat_pow_3x3(), most_specific_generalization() (+11 more)

### Community 21 - "Span"
Cohesion: 0.11
Nodes (20): collect_inferred_types(), collect_inferred_types_block(), format_block(), format_enum(), format_expr(), format_expr_at(), format_function(), format_literal() (+12 more)

### Community 22 - "MirFunction"
Cohesion: 0.11
Nodes (25): lower_program(), MirBasicBlock, MirBuilder, MirFunction, MirLocalDecl, Rvalue, Alloc, Array (+17 more)

### Community 23 - "Expr"
Cohesion: 0.06
Nodes (32): Expr, ArrayLiteral, Binary, Box, Call, Deref, EnumConstructor, FieldAccess (+24 more)

### Community 24 - "memory_ssa.rs"
Cohesion: 0.11
Nodes (14): collect_reads(), collect_terminator_reads(), format_place(), format_rvalue(), MemoryAccess, Def, Phi, Use (+6 more)

### Community 25 - "bce.rs"
Cohesion: 0.20
Nodes (15): as_int_const(), BceContext, collect_call_param_ranges(), collect_calls_in_block(), collect_calls_in_expr(), collect_local_arrays(), eval_range(), optimize_function() (+7 more)

### Community 26 - "ast.rs"
Cohesion: 0.13
Nodes (23): Block, EnumDef, EnumVariant, Function, Item, Enum, Function, Struct (+15 more)

### Community 27 - "Place"
Cohesion: 0.09
Nodes (14): AliasAnalysis, AliasResult, MayAlias, MustAlias, NoAlias, collect_reads_for_alias(), ModRefResult, Mod (+6 more)

### Community 28 - "BasicBlockId"
Cohesion: 0.24
Nodes (20): compute_dominance(), detect_loops(), DominanceInfo, LoopInfo, BasicBlockId, bounds_match(), build_alias_map(), collect_read_arrays_from_rvalue() (+12 more)

### Community 29 - "while_unroll.rs"
Cohesion: 0.21
Nodes (24): apply_step(), collect_mutated_vars(), collect_read_vars(), collect_read_vars_expr(), count_var_assignments(), eval_const_expr(), expr_eq(), extract_loop_var() (+16 more)

### Community 30 - "TypedStmt"
Cohesion: 0.10
Nodes (22): patch_continue_in_body(), TypedEnumDef, TypedEnumVariant, TypedMatchArm, TypedMatchPattern, Literal, Variant, Wildcard (+14 more)

### Community 31 - "mir_tests.rs"
Cohesion: 0.14
Nodes (22): check_place_promotability(), collect_all_places(), compute_idf(), eliminate_dead_stores_and_redundant_loads(), find_promotion_candidates(), promote_memory_to_registers(), rename_in_dom_tree(), rewrite_statement_places() (+14 more)

### Community 32 - "polyhedral_and_validation_tests.rs"
Cohesion: 0.14
Nodes (19): supercompile_mir_program_with_mode(), compile_and_run_mode(), get_mir(), test_distillation_double_zip(), test_distillation_nested_tree_inversion(), test_mrsc_optimal_code_size(), compile_and_run_distill(), get_mir() (+11 more)

### Community 33 - "supercompiler_phase34_tests.rs"
Cohesion: 0.11
Nodes (17): get_mir(), test_refinement_bce_loop(), test_refinement_callsite_propagation(), test_refinement_dead_branch_elimination(), test_refinement_interval_propagation_add(), test_refinement_regression_no_spurious_prune(), get_mir(), run_numlang_code() (+9 more)

### Community 34 - "NumLang Adversarial Audit: Critical Vulnerabilities, Soundness Gaps & Engineering Traps"
Cohesion: 0.08
Nodes (25): 1.1 The Circular Tautology Pattern ($A \to A$), 1.2 Disconnected Semantics: `Evaluates` Ignores `Step`, 1.3 Pseudo-Multithreading in `Semantics.lean`, 1. Formal Verification Façades (Lean 4), 2.1 The Copy-Paste Trick in `minspec.nl`, 2. The Futamura Projection Façade, 3.1 Unbounded Memory Leaks (Zero Deallocation Runtime), 3.2 Hardcoded Windows APIs Break Linux & Docker Artifacts (+17 more)

### Community 35 - "stdint"
Cohesion: 0.14
Nodes (12): ack(), main(), fib(), main(), mat_mul(), dot4(), main(), matvec_step() (+4 more)

### Community 36 - "supercompile_program"
Cohesion: 0.14
Nodes (16): supercompile_program(), run_numlang_code(), test_1st_futamura_arithmetic_ast_interpreter(), test_1st_futamura_boolean_tree_evaluator(), test_1st_futamura_peano_inductive_numbers(), test_2nd_futamura_projection(), test_mir_switch_and_discriminant_supercompilation(), test_negative_discriminant_propagation() (+8 more)

### Community 37 - "cache.rs"
Cohesion: 0.13
Nodes (5): CachedSpecialization, CacheKey, find_json_files(), sha256_str(), SpecializationCache

### Community 38 - "3. Statements and Control Flow"
Cohesion: 0.08
Nodes (23): 1. Type System, 2. Operators and Precedence, 3. Statements and Control Flow, 4. Functions, 5. Structs, 6. Built-in Functions and Intrinsics, 7. Compiler CLI Reference, 8. Optimization & Supercompiler Model (+15 more)

### Community 39 - "link_executable"
Cohesion: 0.17
Nodes (16): ENTRY_BENCH_OBJ, find_msvc_link(), find_rust_lld(), find_windows_linker(), find_windows_sdk_lib_dirs(), link_executable(), link_unix(), link_windows() (+8 more)

### Community 40 - "loop_opt.rs"
Cohesion: 0.20
Nodes (19): collect_mutated_vars_stmts(), collect_read_vars_expr(), collect_read_vars_stmts(), is_int_one(), is_int_zero(), is_name_ident(), is_name_sub_one(), is_simple_inc_dec() (+11 more)

### Community 41 - "Value"
Cohesion: 0.15
Nodes (16): fold_binary(), fold_binary_typed(), fold_float_binary(), fold_int_binary(), fold_uint_binary(), BinOp, Value, Array (+8 more)

### Community 42 - "runner.py"
Cohesion: 0.11
Nodes (8): bootstrap_ci(), compile_benchmark(), find_vcvars(), is_crash_exit(), main(), measure_execution_times(), parse_compute_ns(), pin_cpu_affinity()

### Community 43 - "Parser<'a>"
Cohesion: 0.21
Nodes (4): parse(), Parser, Parser<'a>, SpannedToken

### Community 45 - "Terminator"
Cohesion: 0.12
Nodes (15): BasicBlock, compute_cfg(), Projection, Deref, Field, Index, Payload, Terminator (+7 more)

### Community 47 - "generate_tables.py"
Cohesion: 0.18
Nodes (13): compute_sha256(), escape_latex(), format_latency(), generate_ablation_table(), generate_head_to_head_table(), generate_table_benchmarks(), generate_table_codesize(), generate_table_compiletime() (+5 more)

### Community 48 - "SymExpr"
Cohesion: 0.15
Nodes (13): DUMMY_SPAN, fold_unary(), fold_unary_typed(), sym_result_type(), sym_to_expr(), SymExpr, Call, If (+5 more)

### Community 49 - "IrOp"
Cohesion: 0.10
Nodes (20): IrOp, Add, BitAnd, BitOr, BitXor, Div, Eq, Ge (+12 more)

### Community 50 - "CompilerDiagnostic"
Cohesion: 0.13
Nodes (7): CompilerDiagnostic, LexError, SyntaxError, TypeError, format_ast(), format_tokens(), format_typed_ast()

### Community 51 - "ParseError"
Cohesion: 0.26
Nodes (5): ParseError, InvalidPrefix, UnexpectedEof, UnexpectedToken, Parser<'a>

### Community 52 - "collections"
Cohesion: 0.27
Nodes (11): block_mutates_any(), block_mutates_name(), calls_function(), collect_calls_in_block(), collect_calls_in_expr(), collect_local_literal_bindings(), optimize_program(), params_are_mutated() (+3 more)

### Community 53 - "Remediation & Frontier Roadmap (Phases 20–28) [ACTIVE]"
Cohesion: 0.11
Nodes (17): Historical Foundation (Phases 1–19) [COMPLETE], Overview, Phase 1: Core Language & AOT Compiler, Phase 20: Fix Core Residualization, Knot Transfers & Textbook MSG [PLANNED], Phase 21: Real Hamilton Global Distillation [PLANNED], Phase 22: Real Multi-Result Supercompilation (MRSC) [PLANNED], Phase 23: Real Polyhedral Loop & Stencil Deforestation [PLANNED], Phase 24: Formal SMT-Based Translation Validation [PLANNED] (+9 more)

### Community 54 - "BinaryOp"
Cohesion: 0.11
Nodes (18): BinaryOp, Add, BitAnd, BitOr, BitXor, Div, Eq, Ge (+10 more)

### Community 55 - "monomorphize.rs"
Cohesion: 0.31
Nodes (11): mangle_generic_name(), monomorphize(), rewrite_calls_in_block(), rewrite_calls_in_expr(), rewrite_calls_in_stmt(), substitute_block(), substitute_expr(), substitute_stmt() (+3 more)

### Community 56 - "Part I: Production Compiler Baseline (Phases 1–9)"
Cohesion: 0.12
Nodes (17): Objective, Objective, Objective, Objective, Part I: Production Compiler Baseline (Phases 1–9), Phase 3: Core Control Flow (For Loops, Continue, Loop), Phase 4: Standard I/O Built-ins (Print and Println), Phase 6: Composite Struct Types & Field Access (+9 more)

### Community 57 - "Part II: Core Supercompiler & Language Extensions (Phases 10–19)"
Cohesion: 0.12
Nodes (17): Objective, Objective, Objective, Objective, Part II: Core Supercompiler & Language Extensions (Phases 10–19), Phase 11: Higher-Order Functions, Closures & Pipeline Deforestation, Phase 12: Polymorphic Types & Generics with Monomorphization, Phase 13: Heap Allocation (Box<T>, Deref) & Symbolic Pointer Driving (+9 more)

### Community 58 - "Pre-Written Rebuttals to Likely Reviewer Objections"
Cohesion: 0.12
Nodes (16): Author Response, Author Response, Author Response, Author Response, Author Response, Objection 1: "The benchmark suite is too small / cherry-picked.", Objection 2: "The Lean proofs don't cover the full implementation (gap between model and code).", Objection 3: "The parallel benchmark (Phase 36) speedup is marginal / machine-dependent." (+8 more)

### Community 59 - "benchmark_harness.rs"
Cohesion: 0.22
Nodes (7): BENCH_COUNTER, benchmark_exe(), compile_c(), compile_numlang(), compile_rust(), find_vcvars64(), test_comparative_benchmarks()

### Community 61 - "multi_language_benchmarks.rs"
Cohesion: 0.26
Nodes (11): benchmark_cmd(), BenchmarkWorkload, compile_c(), compile_numlang(), compile_rust(), find_vcvars64(), test_comprehensive_multi_language_benchmarks(), wrap_c() (+3 more)

### Community 62 - "plot.py"
Cohesion: 0.18
Nodes (5): load_data(), main(), plot_codesize(), plot_speedup(), plot_throughput()

### Community 63 - "binaryop"
Cohesion: 0.29
Nodes (9): collect_array_info(), expand_dot_calls(), expand_dots_in_block(), fold_constants(), fold_in_block(), optimize_arrays(), optimize_function_arrays(), propagate_array_elements() (+1 more)

### Community 64 - "supercompiler_head_to_head.rs"
Cohesion: 0.32
Nodes (9): benchmark_cmd(), compile_c(), compile_numlang(), compile_rust(), find_vcvars64(), HeadToHeadBench, test_run_supercompiler_head_to_head_benchmarks(), wrap_c() (+1 more)

### Community 65 - "Part III: Remediation & Frontier Supercompilation (Phases 20–28)"
Cohesion: 0.14
Nodes (13): Context & Problem Statement, Context & Problem Statement, NumLang: Complete Master Phase Prompts (Phases 1–28), Part III: Remediation & Frontier Supercompilation (Phases 20–28), Phase 21: Real Hamilton Global Process-Tree Distillation, Phase 22: Real Multi-Result Supercompilation (MRSC) Hypergraph Search, Table of Contents, Target Files (+5 more)

### Community 66 - "supercompiler_symbolic_tests.rs"
Cohesion: 0.15
Nodes (4): get_mir(), test_mir_supercompile_program_execution(), test_recurrence_solver_cubic_and_geometric(), test_recurrence_solver_linear_and_triangular()

### Community 67 - "supercompiler_phase30_tests.rs"
Cohesion: 0.22
Nodes (10): get_mir(), test_ast_inliner_precomputed_has_loop_behavior(), test_budget_overflow_leaf_is_unreachable(), test_msg_knot_materializes_gen_node(), test_unified_profitability_gate_consistency(), get_mir(), test_order3_recurrence_symbolic_n(), test_termination_witness_all_benchmarks() (+2 more)

### Community 68 - "NumLang: A Formally Verified, Self-Applicable Process-Tree Supercompiler"
Cohesion: 0.14
Nodes (13): 1-Click Reproducible Docker Artifact, 60-Second Quickstart, Architecture Overview, Build Compiler, Key Features, License, NumLang: A Formally Verified, Self-Applicable Process-Tree Supercompiler, Reproducibility & Artifact Evaluation (+5 more)

### Community 70 - "enum_tests.rs"
Cohesion: 0.25
Nodes (11): run_numlang_code(), test_enum_state_machine_loop(), test_enum_typecheck_errors(), test_multiple_payload_variants(), test_nested_enums(), test_none_variant_matching(), test_returning_enum_err_from_function(), test_returning_enum_from_function() (+3 more)

### Community 71 - "Formal Correctness and Certified Termination of the NumLang Supercompiler"
Cohesion: 0.15
Nodes (12): 1. NumLang Core Formal Operational Semantics, 2. Mechanized Driving and Soundness, 3. Homeomorphic Embedding and the Whistle, 4. Kruskal's Tree Theorem and Process Tree Termination, 5. Verification Instructions, Abstract, Definition (Homeomorphic Embedding), Formal Correctness and Certified Termination of the NumLang Supercompiler (+4 more)

### Community 72 - "Remediation & Frontier Requirements"
Cohesion: 0.15
Nodes (12): 1. Residualization & Generalization (Phase 20), 2. Hamilton Global Distillation (Phase 21), 3. Multi-Result Supercompilation (Phase 22), 4. Polyhedral Loop & Stencil Deforestation (Phase 23), 5. Formal SMT-Based Translation Validation (Phase 24), 6. Genuine Futamura Projections (Phase 25), 7. Rigorous Lean 4 Formal Verification (Phase 26), 8. Honest High-Precision Benchmarks (Phase 27) (+4 more)

### Community 73 - "Artifact Evaluation & Reproducibility Guide: NumLang"
Cohesion: 0.15
Nodes (12): 1. System Requirements, 2. Quickstart: 1-Click Docker Reproduction, 3. Bare Metal Reproduction, 4. Mapping Paper Claims to Artifact Evidence, 5. Benchmark Suite Catalog, Artifact Evaluation & Reproducibility Guide: NumLang, Hardware Requirements, Software Requirements (+4 more)

### Community 74 - "opt/supercompiler/mod.rs"
Cohesion: 0.21
Nodes (7): block_calls_function(), collect_literal_calls(), expr_calls_function(), function_is_recursive(), DUMMY_SPAN, residualize_return(), value_to_expr()

### Community 77 - "coupled_recurrence_and_fusion_tests.rs"
Cohesion: 0.24
Nodes (10): compile_and_run_supercompiled(), test_coupled_2var_linear_recurrence(), test_coupled_2var_recurrence_direct(), test_coupled_fibonacci_native_execution(), test_coupled_fibonacci_symbolic_recurrence(), test_interprocedural_inlining_and_folding(), test_interprocedural_triangular_loop_fusion(), test_order2_recurrence_fibonacci_direct() (+2 more)

### Community 78 - "mir"
Cohesion: 0.20
Nodes (4): run_numlang_code(), test_fork_terminator_successors(), test_parallel_residualize_correctness(), test_parallel_residualize_sequential_fallback()

### Community 79 - "time"
Cohesion: 0.29
Nodes (3): format_ns(), main(), run_cmd()

### Community 80 - "parse_expr_str"
Cohesion: 0.21
Nodes (7): parse_expr_str(), test_parse_function_and_statements(), test_parse_mut_and_assignment(), test_pratt_bitwise_operators(), test_pratt_exponentiation_right_associativity(), test_pratt_grouped_precedence(), test_pratt_operator_precedence()

### Community 81 - "deforestation_tests.rs"
Cohesion: 0.48
Nodes (11): supercompile_mir_program(), assert_buffer_eliminated(), compile_and_run_supercompiled(), get_mir(), test_accumulator_fold_closed_form(), test_accumulator_fold_cubic_closed_form(), test_array_map_fusion(), test_deforestation_chained_function_passing() (+3 more)

### Community 82 - "ResidualObjective"
Cohesion: 0.17
Nodes (4): MinCodeSizeObjective, MinDynamicBranchObjective, ParetoObjective, ResidualObjective

### Community 83 - "recursion.rs"
Cohesion: 0.32
Nodes (7): has_self_tail_call_block(), has_self_tail_call_stmt(), optimize_program(), rename_identifiers_block(), rename_identifiers_expr(), replace_tail_calls_block(), try_lower_tail_calls()

### Community 85 - "supercompiler_phase38_tests.rs"
Cohesion: 0.33
Nodes (8): get_lean_dir(), get_supercompiler_dir(), test_distillation_theorem_present(), test_end_to_end_theorem_present(), test_lean_build_succeeds(), test_lean_no_sorry(), test_lean_no_unproven_axiom(), visit_lean_files()

### Community 86 - "List"
Cohesion: 0.42
Nodes (9): append(), double_nrev(), List, Cons, Nil, main(), make_list(), nrev() (+1 more)

### Community 87 - "Peano"
Cohesion: 0.42
Nodes (9): add(), clone_peano(), from_int(), main(), mul(), Peano, Succ, Zero (+1 more)

### Community 88 - "Instruction"
Cohesion: 0.18
Nodes (10): Instruction, Assign, Binary, Branch, BranchIf, Call, IndexLoad, IndexStore (+2 more)

### Community 89 - "match_tests.rs"
Cohesion: 0.31
Nodes (8): run_numlang_code(), test_match_as_expression_assignment(), test_match_boolean(), test_match_integer_basic(), test_match_negative_patterns(), test_match_non_exhaustive_error(), test_match_or_patterns(), test_match_wildcard_catch_unmatched()

### Community 90 - "struct_tests.rs"
Cohesion: 0.31
Nodes (8): run_numlang_code(), test_array_of_structs(), test_basic_struct_literal_and_field_read(), test_nested_structs(), test_struct_field_mutation(), test_struct_pass_by_value(), test_struct_returned_from_function(), test_struct_type_errors()

### Community 91 - "supercompiler_phase39_tests.rs"
Cohesion: 0.33
Nodes (7): get_repo_root(), run_benchmark(), test_all_30_benchmarks_compile(), test_fusion_benchmarks_produce_single_loop(), test_new_numerical_benchmarks_correctness(), test_new_sort_benchmarks_correctness(), test_new_string_benchmarks_correctness()

### Community 92 - "unsigned_type_tests.rs"
Cohesion: 0.31
Nodes (8): run_numlang_code(), test_i8_i16_arithmetic_and_print(), test_signed_i8_and_i16_types(), test_signed_i8_i16_overflow_wrapping(), test_structured_panic_bounds_check(), test_unsigned_division_and_modulo(), test_unsigned_logical_shift(), test_unsigned_types_and_suffixed_literals()

### Community 93 - "NumLang Compiler Backends: Cranelift vs. LLVM"
Cohesion: 0.20
Nodes (9): 1. Architectural Summary, 2. When to Use Which Backend, 3. CLI Usage, 4. Benchmark Comparison, 5. Cross-Compilation and Future Work, NumLang Compiler Backends: Cranelift vs. LLVM, Optimization Levels (`--opt-level <0|1|2|3>`):, Use Cranelift When: (+1 more)

### Community 94 - "peano_mul.c"
Cohesion: 0.47
Nodes (9): add(), clone_peano(), free_peano(), from_int(), main(), mul(), succ(), to_int() (+1 more)

### Community 95 - "List"
Cohesion: 0.42
Nodes (8): List, Cons, Nil, main(), make_text(), match_state0(), match_state1(), match_state2()

### Community 96 - "List"
Cohesion: 0.44
Nodes (8): append(), List, Cons, Nil, main(), make_list(), nrev(), sum_list()

### Community 97 - "mir_codegen_tests.rs"
Cohesion: 0.31
Nodes (8): compile_and_run_mir_code(), compile_and_run_supercompiled_code(), test_mir_codegen_conditional_branch(), test_mir_codegen_loop_accumulation(), test_mir_codegen_simple_arithmetic(), test_mir_supercompiled_direct_execution(), test_mir_supercompiled_triangular_closed_form(), test_mir_supercompiler_explicit_pipeline()

### Community 98 - "LLVM Toolchain Setup Guide for NumLang"
Cohesion: 0.20
Nodes (9): 1. Overview, 2. Installing LLVM on Windows, 3. Environment Variables Configuration, 4. Building NumLang with the LLVM Backend, 5. Usage, LLVM Toolchain Setup Guide for NumLang, Option A: Via Chocolatey (Recommended), Option B: Via Winget (+1 more)

### Community 99 - "Implementation Tasks"
Cohesion: 0.20
Nodes (9): Context & Audit Findings, Goal, Implementation Tasks, Phase 20: Fix Core Residualization, Back-Edge Knot Transfers & Textbook MSG, Success Criteria, Task 1: Knot State Transfer & Parallel Copy Generation (`src/mir/supercompiler/residualize.rs`), Task 2: Residual Block ID Remapping for Phi Nodes (`src/mir/supercompiler/residualize.rs`), Task 3: Textbook Most-Specific Generalization (MSG) (`src/mir/supercompiler/generalize.rs`) (+1 more)

### Community 100 - "NumLang Compiler"
Cohesion: 0.20
Nodes (9): Active (Remediation & Frontier), Core Value, Current Milestone: Remediation & Frontier (Phases 20–28), Evolution & Governance, NumLang Compiler, Out of Scope, Requirements, Validated (Completed Foundation) (+1 more)

### Community 102 - "generate_random_program"
Cohesion: 0.40
Nodes (3): generate_deep_random_program(), generate_random_program(), SimpleRng

### Community 103 - "supercompiler_phase35_tests.rs"
Cohesion: 0.33
Nodes (6): get_mir(), run_numlang_code(), test_code_size_metric_populated(), test_compaction_preserves_correctness(), test_dead_node_elimination(), test_noop_assignment_removal()

### Community 104 - "mrsc_lattice_tests.rs"
Cohesion: 0.31
Nodes (7): compile_and_run_mode(), get_mir(), test_mrsc_competing_objectives_selection_and_divergence(), test_mrsc_custom_weighted_objective(), test_mrsc_end_to_end_execution_parity(), test_mrsc_hypergraph_construction_and_alternative_paths(), test_mrsc_pareto_dominance_mathematical_properties()

### Community 106 - "generic_tests.rs"
Cohesion: 0.29
Nodes (6): run_numlang_code(), TEST_COUNTER, test_generic_higher_order_apply(), test_generic_identity(), test_generic_multiple_instantiations(), test_generic_swap_pair()

### Community 107 - "llvm_vs_cranelift_benchmarks.rs"
Cohesion: 0.38
Nodes (4): compile_numlang(), format_duration(), run_binary(), test_llvm_vs_cranelift_comparative_benchmarks()

### Community 108 - "supercompiler_tests.rs"
Cohesion: 0.44
Nodes (7): run_numlang_code(), run_rust_reference(), test_novel_program_1_triangular_sum(), test_novel_program_2_power_of_two(), test_novel_program_3_xor_period(), test_novel_program_4_cubic_sum(), test_novel_program_5_fibonacci_coupled()

### Community 109 - "double_nrev.c"
Cohesion: 0.47
Nodes (8): append(), cons(), double_nrev(), free_list(), main(), make_list(), nrev(), sum_list()

### Community 110 - "kmp.c"
Cohesion: 0.42
Nodes (7): cons(), free_list(), main(), make_text(), match_state0(), match_state1(), match_state2()

### Community 111 - "List"
Cohesion: 0.44
Nodes (7): append(), List, Cons, Nil, main(), make_list(), sum_list()

### Community 112 - "Tree"
Cohesion: 0.50
Nodes (7): flip(), main(), make_tree(), sum_tree(), Tree, Leaf, Node

### Community 113 - "differential_fuzz_100k.rs"
Cohesion: 0.31
Nodes (4): run_fuzz_batch(), run_numlang_code(), test_differential_fuzz_100k(), test_differential_fuzz_smoke()

### Community 114 - "independence.rs"
Cohesion: 0.44
Nodes (5): collect_subtree_rw_set(), collect_term_vars(), find_parallel_knot_pairs(), ReadWriteSet, sets_are_independent()

### Community 115 - "benchmark_correctness_tests.rs"
Cohesion: 0.39
Nodes (6): run_benchmark(), test_canonical_double_nrev(), test_canonical_kmp(), test_canonical_nrev(), test_canonical_peano_mul(), test_canonical_power_spec()

### Community 116 - "io_tests.rs"
Cohesion: 0.39
Nodes (7): run_numlang_io(), test_print_booleans(), test_print_floats(), test_print_i64_and_negatives(), test_print_loop_concatenation(), test_print_verification_example(), test_println_string_literal()

### Community 117 - "translation_validation_smt_tests.rs"
Cohesion: 0.36
Nodes (7): get_mir(), test_smt_cli_verify_equivalence_with_mutations(), test_smt_loop_unrolling_and_closed_form_equivalence(), test_smt_mutation_detection_off_by_one_and_branch_boundary(), test_smt_mutation_detection_operator_swap(), test_smt_relational_path_vc_conditional_branches(), test_smt_uninterpreted_functions_and_congruence()

### Community 118 - "NumLang Agent Handoff Document"
Cohesion: 0.25
Nodes (7): 1. Project Location & Build Commands, 2. Codebase Organization, 3. Completed: Phase 3 (MIR Codegen Hookup & Supercompiler Pipeline), 4. Completed: Phase 4 (Coupled Recurrences & Interprocedural Fusion), 5. Next Milestone: Phase 5 (Empirical Benchmark Suite vs State-of-the-Art), 6. Completed: Phase 10 (Close All Turchin Supercompiler Gaps), NumLang Agent Handoff Document

### Community 119 - "differential_correctness_tests.rs"
Cohesion: 0.32
Nodes (3): run_numlang_code(), test_differential_10k(), test_differential_random_programs_small()

### Community 120 - "nrev.c"
Cohesion: 0.54
Nodes (7): append(), cons(), free_list(), main(), make_list(), nrev(), sum_list()

### Community 121 - "tree_flip.c"
Cohesion: 0.54
Nodes (7): flip(), free_tree(), main(), make_leaf(), make_node(), make_tree(), sum_tree()

### Community 122 - "supercompiler_phase32_tests.rs"
Cohesion: 0.39
Nodes (6): get_mir(), test_3way_linear_recurrence_closed_form(), test_detect_nway_system_unit(), test_existing_coupled_still_works(), test_hofstadter_male_female_recurrence(), test_tribonacci_closed_form()

### Community 123 - "NumLang Academic & Engineering Integrity Rules"
Cohesion: 0.25
Nodes (7): 1. Zero Tolerance for Data Fabrication & Tampering, 2. In-Process, High-Resolution Performance Measurement, 3. Real Algorithms vs. Named Stubs, 4. Formal Proof Integrity (Lean 4), 5. Authentic Futamura Projections, 6. Literature Grounding & Direct Supercompiler Comparisons, NumLang Academic & Engineering Integrity Rules

### Community 124 - "memory_ssa_tests.rs"
Cohesion: 0.39
Nodes (5): get_mir(), test_integration_mem2reg_full_pipeline(), test_integration_multi_branch_memory_ssa(), test_integration_nested_loops_memory_ssa(), test_integration_struct_field_alias_precision()

### Community 125 - "Operand"
Cohesion: 0.29
Nodes (6): Operand, BoolConst, FloatConst, IntConst, Value, ValueId

### Community 126 - "ir/mod.rs"
Cohesion: 0.54
Nodes (5): BasicBlock, format_ir(), IrFunction, IrParam, IrProgram

### Community 127 - "supercompiler_phase40_tests.rs"
Cohesion: 0.46
Nodes (6): get_repo_root(), test_dockerfile_exists_and_is_valid(), test_makefile_targets_present(), test_paper_compiles_pdflatex(), test_paper_sections_exist(), test_rebuttal_objections_complete()

### Community 128 - "true_futamura_projections_tests.rs"
Cohesion: 0.32
Nodes (3): run_minspec(), test_minspec_exits_42(), test_verify_soundness_all_programs()

### Community 129 - "append3.c"
Cohesion: 0.57
Nodes (6): append(), cons(), free_list(), main(), make_list(), sum_list()

### Community 130 - "Key Findings"
Cohesion: 0.29
Nodes (6): Architecture Approach, Critical Pitfalls, Executive Summary, Expected Features, Key Findings, Recommended Stack

### Community 131 - "NumLang Roadmap & Technical Architecture"
Cohesion: 0.29
Nodes (6): 1. High-Level Vision, 2. Milestone & Phase Tracker, 3. Architecture Deep-Dive: SSA Supercompiler Pipeline, 4. Development Invariants & Integrity Mandate, Completed Foundation (Phases 1–19), NumLang Roadmap & Technical Architecture

### Community 132 - "explain.rs"
Cohesion: 0.43
Nodes (5): ErrorExplanation, EXPLANATIONS, get_explanation(), print_explanation(), test_all_error_codes_e001_to_e020_have_explanations()

### Community 133 - "higher_order_tests.rs"
Cohesion: 0.43
Nodes (4): run_numlang_code(), test_closure_captures_constant(), test_higher_order_supercompile_constant(), test_lambda_inline_fold()

### Community 134 - "stream_fusion.c"
Cohesion: 0.53
Nodes (4): is_even(), main(), square(), stream_pipeline()

### Community 135 - "Critical Pitfalls & Prevention Strategies"
Cohesion: 0.33
Nodes (5): Critical Pitfalls & Prevention Strategies, Pitfall 1: Incorrect Phi Placement in Loops (The Incomplete Reaching Definition Bug), Pitfall 2: Overly Conservative Pointer Aliasing, Pitfall 3: Dead Store Elimination Across Memory Barriers / Calls, Pitfall 4: Combinatorial MemorySSA Graph Blowup

### Community 136 - "STACK.md"
Cohesion: 0.33
Nodes (5): Alternatives Considered, Core Technologies, Recommended Stack, Supporting Algorithms & Patterns, What NOT to Use

### Community 137 - "Project State"
Cohesion: 0.33
Nodes (5): Accumulated Context, Critical Audit Findings & Decisions, Current Position, Progress, Project State

### Community 139 - "Phase 20: Fix Core Residualization, Back-Edge Knot Transfers & Textbook MSG"
Cohesion: 0.40
Nodes (5): Context & Problem Statement, Phase 20: Fix Core Residualization, Back-Edge Knot Transfers & Textbook MSG, Target Files, Technical Tasks, Verification Gate

### Community 140 - "Phase 23: Real Polyhedral Loop & Stencil Deforestation with Buffer Contraction"
Cohesion: 0.40
Nodes (5): Context & Problem Statement, Phase 23: Real Polyhedral Loop & Stencil Deforestation with Buffer Contraction, Target Files, Technical Tasks, Verification Gate

### Community 141 - "Phase 24: Formal SMT-Based Translation Validation & Simulation Preorder"
Cohesion: 0.40
Nodes (5): Context & Problem Statement, Phase 24: Formal SMT-Based Translation Validation & Simulation Preorder, Target Files, Technical Tasks, Verification Gate

### Community 142 - "Phase 25: Genuine Self-Applicable Specializer MinSpec.nl for 2nd and 3rd Futamura"
Cohesion: 0.40
Nodes (5): Context & Problem Statement, Phase 25: Genuine Self-Applicable Specializer MinSpec.nl for 2nd and 3rd Futamura, Target Files, Technical Tasks, Verification Gate

### Community 143 - "Phase 26: Rigorous Lean 4 Formal Verification (Zero Axioms, Recursive Semantics)"
Cohesion: 0.40
Nodes (5): Context & Problem Statement, Phase 26: Rigorous Lean 4 Formal Verification (Zero Axioms, Recursive Semantics), Target Files, Technical Tasks, Verification Gate

### Community 144 - "Phase 27: Honest High-Precision Benchmarks & Direct Supercompiler Comparisons"
Cohesion: 0.40
Nodes (5): Context & Problem Statement, Phase 27: Honest High-Precision Benchmarks & Direct Supercompiler Comparisons, Target Files, Technical Tasks, Verification Gate

### Community 145 - "Phase 28: Paper Rewrite & 1-Click Reproducible Artifact Package"
Cohesion: 0.40
Nodes (5): Context & Problem Statement, Phase 28: Paper Rewrite & 1-Click Reproducible Artifact Package, Target Files, Technical Tasks, Verification Gate

### Community 146 - "Phase 1: Master Refactoring & Baseline Setup"
Cohesion: 0.40
Nodes (5): Objective, Phase 1: Master Refactoring & Baseline Setup, Target Files, Technical Tasks, Verification Gate

### Community 147 - "Phase 10: Turchin Supercompiler Core, ADTs & 1st Futamura Projection"
Cohesion: 0.40
Nodes (5): Objective, Phase 10: Turchin Supercompiler Core, ADTs & 1st Futamura Projection, Target Files, Technical Tasks, Verification Gate

### Community 148 - "Phase 2: Zero-Warning Clippy Cleanliness"
Cohesion: 0.40
Nodes (5): Objective, Phase 2: Zero-Warning Clippy Cleanliness, Target Files, Technical Tasks, Verification Gate

### Community 149 - "stream_fusion.rs"
Cohesion: 0.70
Nodes (4): is_even(), main(), square(), stream_pipeline()

### Community 150 - "Feature Taxonomy"
Cohesion: 0.40
Nodes (4): Anti-Features (What We Deliberately Defer / Avoid), Differentiators (Competitive Advantages for Phase 2), Feature Taxonomy, Table Stakes (Must Have for Phase 2)

### Community 151 - "Phase 5: Low-Bitwidth Signed Types (i8 and i16)"
Cohesion: 0.50
Nodes (4): Objective, Phase 5: Low-Bitwidth Signed Types (i8 and i16), Target Files, Verification Gate

### Community 152 - "Phase 7: Pattern Matching (Match Expressions)"
Cohesion: 0.50
Nodes (4): Objective, Phase 7: Pattern Matching (Match Expressions), Target Files, Verification Gate

### Community 153 - "Phase 9: Built-in Standard Library (std)"
Cohesion: 0.50
Nodes (4): Objective, Phase 9: Built-in Standard Library (std), Target Files, Verification Gate

### Community 154 - "Phase 14: Self-Applicable Specializer Prototype & Multistage Futamura"
Cohesion: 0.50
Nodes (4): Objective, Phase 14: Self-Applicable Specializer Prototype & Multistage Futamura, Target Files, Verification Gate

### Community 155 - "Phase 15: Differential Fuzzing (100k Cases) & Lean 4 Setup"
Cohesion: 0.50
Nodes (4): Objective, Phase 15: Differential Fuzzing (100k Cases) & Lean 4 Setup, Target Files, Verification Gate

### Community 156 - "Phase 17: Multi-Stage Docker Artifact Packaging & PEPM Paper Draft"
Cohesion: 0.50
Nodes (4): Objective, Phase 17: Multi-Stage Docker Artifact Packaging & PEPM Paper Draft, Target Files, Verification Gate

### Community 157 - "Phase 18: Global Process-Tree Distillation & MRSC Interface"
Cohesion: 0.50
Nodes (4): Objective, Phase 18: Global Process-Tree Distillation & MRSC Interface, Target Files, Verification Gate

### Community 158 - "Phase 19: Polyhedral Stencils, Translation Validation & Parallel Driving"
Cohesion: 0.50
Nodes (4): Objective, Phase 19: Polyhedral Stencils, Translation Validation & Parallel Driving, Target Files, Verification Gate

### Community 159 - "fib_matrix.rs"
Cohesion: 0.83
Nodes (3): fib(), main(), mat_mul()

### Community 160 - "matvec_4x4.rs"
Cohesion: 0.83
Nodes (3): dot4(), main(), matvec_step()

### Community 161 - "Docker Reproducibility Environment for NumLang Artifact"
Cohesion: 0.50
Nodes (3): Building the Docker Image, Docker Reproducibility Environment for NumLang Artifact, Running All Experiments

### Community 162 - "Phase 2.1 Plan: MemorySSA Core & Graph Construction"
Cohesion: 0.50
Nodes (3): Goal, Phase 2.1 Plan: MemorySSA Core & Graph Construction, Tasks

### Community 163 - "Phase 2.1 Summary: MemorySSA Core & Graph Construction"
Cohesion: 0.50
Nodes (3): Key Changes, Outcome, Phase 2.1 Summary: MemorySSA Core & Graph Construction

### Community 164 - "Phase 2.2 Plan: Points-To & Field-Sensitive Alias Analysis"
Cohesion: 0.50
Nodes (3): Goal, Phase 2.2 Plan: Points-To & Field-Sensitive Alias Analysis, Tasks

### Community 165 - "Phase 2.2 Summary: Points-To & Field-Sensitive Alias Analysis"
Cohesion: 0.50
Nodes (3): Key Features, Outcome, Phase 2.2 Summary: Points-To & Field-Sensitive Alias Analysis

### Community 166 - "Phase 2.3 Plan: Mem2Reg Promotion Engine & Memory Optimizations"
Cohesion: 0.50
Nodes (3): Goal, Phase 2.3 Plan: Mem2Reg Promotion Engine & Memory Optimizations, Tasks

### Community 167 - "Phase 2.3 Summary: Mem2Reg Promotion Engine & Memory Optimizations"
Cohesion: 0.50
Nodes (3): Key Features, Outcome, Phase 2.3 Summary: Mem2Reg Promotion Engine & Memory Optimizations

### Community 168 - "Phase 2.4 Plan: CLI Tooling, Integration & Verification Gate"
Cohesion: 0.50
Nodes (3): Goal, Phase 2.4 Plan: CLI Tooling, Integration & Verification Gate, Tasks

### Community 169 - "Phase 2.4 Summary: CLI Tooling, Integration & Verification Gate"
Cohesion: 0.50
Nodes (3): Key Deliverables, Outcome, Phase 2.4 Summary: CLI Tooling, Integration & Verification Gate

## Knowledge Gaps
- **639 isolated node(s):** `numlang`, `Nil`, `Nil`, `Nil`, `Nil` (+634 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 1069 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **32 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Type` connect `Type` to `TypedExpr`, `drive.rs`, `TermInterner`, `TypeError`, `MirProgram`, `ScopeEnvironment`, `.compile_mir_statement`, `TypedProgram`, `TypedBlock`, `Span`, `MirFunction`, `TypedStmt`, `Value`, `Terminator`, `SymExpr`, `.fmt`, `monomorphize.rs`, `Instruction`, `Operand`, `ir/mod.rs`?**
  _High betweenness centrality (0.153) - this node is a cross-community bridge._
- **Why does `MirFunction` connect `MirFunction` to `TypedExpr`, `BvExpr`, `drive.rs`, `MirProgram`, `.compile_mir_statement`, `mrsc.rs`, `polyhedral.rs`, `mir/supercompiler/mod.rs`, `Type`, `ResidualObjective`, `memory_ssa.rs`, `Place`, `BasicBlockId`, `mir_tests.rs`?**
  _High betweenness centrality (0.054) - this node is a cross-community bridge._
- **Why does `TypedExpr` connect `TypedExpr` to `TermInterner`, `TypeError`, `TypedProgram`, `Type`, `TypedBlock`, `Span`, `MirFunction`, `bce.rs`, `while_unroll.rs`, `TypedStmt`, `supercompile_program`, `loop_opt.rs`, `Value`, `IrLowerer`, `SymExpr`, `collections`, `monomorphize.rs`, `binaryop`, `opt/supercompiler/mod.rs`, `recursion.rs`?**
  _High betweenness centrality (0.053) - this node is a cross-community bridge._
- **Are the 98 inferred relationships involving `tokenize()` (e.g. with `format_source()` and `compile_source_to_typed()`) actually correct?**
  _`tokenize()` has 98 INFERRED edges - model-reasoned connections that need verification._
- **Are the 79 inferred relationships involving `typecheck()` (e.g. with `format_source()` and `compile_source_to_typed()`) actually correct?**
  _`typecheck()` has 79 INFERRED edges - model-reasoned connections that need verification._
- **What connects `numlang`, `Nil`, `Nil` to the rest of the system?**
  _639 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `TypedExpr` be split into smaller, more focused modules?**
  _Cohesion score 0.05082691759338466 - nodes in this community are weakly interconnected._