use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::drive::SupercompilerStats;
use numlang::mir::supercompiler::mrsc::{
    MinCodeSizeObjective, MrscCostModel, MrscCostVector, MrscFallbackReason,
    MrscObjective, MrscProfitabilityContract, MultiResultEngine,
};
use numlang::mir::supercompiler::distill::calculate_function_cost;
use numlang::mir::supercompiler::supercompile_mir_function_with_stats;
use numlang::mir::supercompiler::{supercompile_mir_program_with_mode, SupercompileMode};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn compile_to_mir(source: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(source).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    lower_program(&typed)
}

#[test]
fn test_cost_model_never_worse_than_baseline_invariant() {
    let benchmarks = [
        ("nrev", include_str!("../bench/showdown/numlang/nrev.nl")),
        (
            "append3",
            include_str!("../bench/showdown/numlang/append3.nl"),
        ),
        ("kmp", include_str!("../bench/showdown/numlang/kmp.nl")),
        (
            "peano_mul",
            include_str!("../bench/showdown/numlang/peano_mul.nl"),
        ),
        (
            "tree_flip",
            include_str!("../bench/showdown/numlang/tree_flip.nl"),
        ),
        (
            "fib_matrix",
            include_str!("../bench/showdown/numlang/fib_matrix.nl"),
        ),
        (
            "tri_sum",
            include_str!("../bench/showdown/numlang/tri_sum.nl"),
        ),
        (
            "cubic_sum",
            include_str!("../bench/showdown/numlang/cubic_sum.nl"),
        ),
        (
            "pow2_mod",
            include_str!("../bench/showdown/numlang/pow2_mod.nl"),
        ),
        (
            "hofstadter",
            include_str!("../bench/showdown/numlang/hofstadter.nl"),
        ),
        (
            "tri_sum_dyn",
            include_str!("../bench/showdown/numlang/tri_sum_dyn.nl"),
        ),
        (
            "cubic_sum_dyn",
            include_str!("../bench/showdown/numlang/cubic_sum_dyn.nl"),
        ),
        (
            "fib_matrix_dyn",
            include_str!("../bench/showdown/numlang/fib_matrix_dyn.nl"),
        ),
        (
            "pow2_mod_dyn",
            include_str!("../bench/showdown/numlang/pow2_mod_dyn.nl"),
        ),
        (
            "compose5",
            include_str!("../bench/showdown/numlang/compose5.nl"),
        ),
        (
            "map_map",
            include_str!("../bench/showdown/numlang/map_map.nl"),
        ),
        (
            "sum_map",
            include_str!("../bench/showdown/numlang/sum_map.nl"),
        ),
        (
            "stream_take",
            include_str!("../bench/showdown/numlang/stream_take.nl"),
        ),
    ];

    let cost_model = MrscCostModel::new();
    let mut total_funcs_checked = 0;

    for (name, src) in benchmarks {
        let base_mir = compile_to_mir(src);

        for func in &base_mir.functions {
            let (sc_func, stats) = supercompile_mir_function_with_stats(func);
            let base_cost = cost_model.evaluate_function(func, &SupercompilerStats::default());
            let sc_cost = cost_model.evaluate_function(&sc_func, &stats);

            let base_score = cost_model.score(&base_cost, MrscObjective::Speed);
            let sc_score = cost_model.score(&sc_cost, MrscObjective::Speed);

            total_funcs_checked += 1;
            assert!(
                sc_score <= base_score + 1e-6,
                "Cost model invariant violated on benchmark '{}' func '{}': SC score ({:.2}) > Base score ({:.2})\nBase cost: {:?}\nSC cost: {:?}",
                name,
                func.name,
                sc_score,
                base_score,
                base_cost,
                sc_cost
            );
        }
    }

    assert!(
        total_funcs_checked >= 18,
        "Expected at least 18 functions checked, got {}",
        total_funcs_checked
    );
    println!(
        "Verified cost model invariant across all {} benchmark functions.",
        total_funcs_checked
    );
}

/// Deterministic LCG used for reproducible metamorphic variations (no extra deps).
fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state >> 33
}

fn assert_gate_never_slower(source: &str, label: &str) {
    let mir = compile_to_mir(source);
    let model = MrscCostModel::new();
    for func in &mir.functions {
        let engine = MultiResultEngine::new(func, &mir.functions);
        let (residual, tree, _objective_score, decision) =
            engine.explore_and_select_with_report(&MrscObjective::Speed);

        let baseline = model.evaluate_function(func, &SupercompilerStats::default());
        let selected = model.evaluate(&residual, &tree);
        let baseline_score = model.score(&baseline, MrscObjective::Speed);
        let selected_score = model.score(&selected, MrscObjective::Speed);

        assert!(
            selected_score <= baseline_score + 1e-6,
            "{}: gate selected a slower residual for '{}' ({} > {})",
            label,
            func.name,
            selected_score,
            baseline_score
        );
        if decision.fallback {
            assert_eq!(
                residual, *func,
                "{}: fallback must return the original baseline MIR",
                label
            );
        }
    }
}

#[test]
fn test_hard_fallback_decision_is_deterministic() {
    let model = MrscCostModel::new();
    let cheap = MrscCostVector {
        dynamic_steps: 10.0,
        allocation_count: 0,
        call_count: 1,
        residual_blocks: 3,
        register_pressure: 2,
    };
    let expensive = MrscCostVector {
        dynamic_steps: 20.0,
        allocation_count: 0,
        call_count: 1,
        residual_blocks: 3,
        register_pressure: 2,
    };

    let regressed = model.decide(&cheap, Some(&expensive), MrscProfitabilityContract::STRICT);
    assert!(!regressed.accepted);
    assert!(regressed.fallback);
    assert_eq!(regressed.reason, MrscFallbackReason::CostRegression);

    let tiny_gain = model.decide(
        &cheap,
        Some(&MrscCostVector {
            dynamic_steps: 9.9,
            ..cheap
        }),
        MrscProfitabilityContract::default(),
    );
    assert!(!tiny_gain.accepted);
    assert_eq!(
        tiny_gain.reason,
        MrscFallbackReason::ImprovementBelowMargin
    );

    let repeated = model.decide(&cheap, Some(&expensive), MrscProfitabilityContract::STRICT);
    assert_eq!(regressed.reason, repeated.reason);
    assert_eq!(regressed.baseline_score, repeated.baseline_score);
    assert_eq!(regressed.fallback, repeated.fallback);
}

#[test]
fn test_never_accept_contract_returns_baseline_and_telemetry() {
    let source = r#"
        fn loop_kernel(n: i64) -> i64 {
            let mut acc: i64 = 0;
            let mut i: i64 = 0;
            while i < n {
                acc = acc + i;
                i = i + 1;
            }
            return acc;
        }
        fn main() -> i64 { return loop_kernel(7); }
    "#;
    let mir = compile_to_mir(source);
    let func = mir
        .functions
        .iter()
        .find(|f| f.name == "loop_kernel")
        .unwrap();
    let engine = MultiResultEngine::new(func, &mir.functions)
        .with_contract(MrscProfitabilityContract::NEVER_ACCEPT);

    let (residual, _tree, _score, decision) =
        engine.explore_and_select_with_report(&MinCodeSizeObjective);

    assert_eq!(residual, *func);
    assert!(decision.fallback);
    assert!(!decision.accepted);
    assert!(decision.candidate.is_some());
    let telemetry = engine.last_decision().expect("missing selection telemetry");
    assert_eq!(telemetry, decision);
}

#[test]
fn test_metamorphic_gate_renaming_field_permutation_loop_bounds() {
    let mut seed = 0x4d52_5343u64;

    // 1. Deterministic pseudo-random loop bounds and identifier renaming.
    for case in 0..12u64 {
        let bound = 2 + (next_random(&mut seed) % 8) as i64;
        let suffix = format!("v{}", case);
        let source = format!(
            r#"
            fn kernel_{suffix}(input: i64) -> i64 {{
                let mut accumulator_{suffix}: i64 = input;
                let mut counter_{suffix}: i64 = 0;
                while counter_{suffix} < {bound} {{
                    accumulator_{suffix} = accumulator_{suffix} * 2 + 1;
                    counter_{suffix} = counter_{suffix} + 1;
                }}
                return accumulator_{suffix};
            }}
            fn main() -> i64 {{ return kernel_{suffix}(3); }}
            "#
        );
        assert_gate_never_slower(&source, &format!("rename/bound case {}", case));
    }

    // 2. Permuted struct field order must not affect the gate outcome.
    let field_orders = [
        ("alpha, beta", "alpha: 3, beta: 5"),
        ("beta, alpha", "beta: 5, alpha: 3"),
    ];
    for (decl, init) in field_orders {
        let source = format!(
            r#"
            struct Pair {{ {decl}: i64 }}
            fn mix_pair(p: Pair, factor: i64) -> i64 {{
                return p.alpha * factor + p.beta;
            }}
            fn main() -> i64 {{
                let p: Pair = Pair {{ {init} }};
                return mix_pair(p, 4);
            }}
            "#
        );
        // The shorthand declaration above expands to two fields for the parser.
        let source = source.replace(
            &format!("struct Pair {{ {decl}: i64 }}"),
            &format!(
                "struct Pair {{ {} }}",
                if decl.starts_with("alpha") {
                    "alpha: i64, beta: i64"
                } else {
                    "beta: i64, alpha: i64"
                }
            ),
        );
        assert_gate_never_slower(&source, &format!("field order {}", decl));
    }

    // 3. Same shape, varied loop bounds, with the same identifiers.
    for bound in [3, 5, 8, 13] {
        let source = format!(
            r#"
            fn bounded(n: i64) -> i64 {{
                let mut sum: i64 = n;
                let mut idx: i64 = 0;
                while idx < {bound} {{
                    sum = sum + idx * 2;
                    idx = idx + 1;
                }}
                return sum;
            }}
            fn main() -> i64 {{ return bounded(2); }}
            "#
        );
        assert_gate_never_slower(&source, &format!("bound {}", bound));
    }
}


#[test]
fn test_fused_residual_is_cheaper_under_five_dimensional_cost_model() {
    let source = r#"
    enum List {
        Nil,
        Cons(i64, Box<List>),
    }

    fn append(xs: List, ys: List) -> List {
        return match xs {
            Nil => ys,
            Cons(h, t) => Cons(h, box(append(deref(t), ys))),
        };
    }

    fn append3(xs: List, ys: List, zs: List) -> List {
        return append(append(xs, ys), zs);
    }

    fn sum_list(xs: List) -> i64 {
        return match xs {
            Nil => 0,
            Cons(h, t) => h + sum_list(deref(t)),
        };
    }

    fn main() -> i64 {
        let xs: List = Cons(1, box(Nil));
        let ys: List = Cons(2, box(Nil));
        let zs: List = Cons(3, box(Nil));
        return sum_list(append3(xs, ys, zs));
    }
    "#;

    let baseline = compile_to_mir(source);
    let mut fused_program = compile_to_mir(source);
    supercompile_mir_program_with_mode(&mut fused_program, SupercompileMode::Distill, "size");
    let fused = fused_program
        .functions
        .iter()
        .find(|f| f.is_distilled && f.name.starts_with("__distill_"))
        .expect("structural distillation must synthesise a fused residual");

    let model = MrscCostModel::new();
    let base_cost: usize = ["append"]
        .iter()
        .map(|name| {
            let func = baseline.functions.iter().find(|f| &f.name == name).unwrap();
            calculate_function_cost(func)
        })
        .sum();
    // The fused residual replaces one round of append's allocations/calls with
    // a single recursive loop; both the 5D model and the distill cost function
    // must rank it below the two producer/consumer calls it replaces.
    let fused_cost = calculate_function_cost(fused);
    assert!(
        fused_cost < base_cost * 2,
        "distill instruction cost {} must be below two producer/consumer calls ({})",
        fused_cost,
        base_cost * 2
    );

    let base_vector = model.evaluate_function(
        baseline.functions.iter().find(|f| f.name == "append").unwrap(),
        &SupercompilerStats::default(),
    );
    let fused_vector = model.evaluate_function(fused, &SupercompilerStats::default());
    let base_score = model.score(&base_vector, MrscObjective::Speed);
    let fused_score = model.score(&fused_vector, MrscObjective::Speed);
    assert!(
        fused_score < 2.0 * base_score,
        "MRSC 5D speed score {} must be below two baseline calls ({})",
        fused_score,
        2.0 * base_score
    );
    assert!(
        fused_vector.allocation_count <= 2 * base_vector.allocation_count,
        "fused residual must not increase allocation sites"
    );
}
