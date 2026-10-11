use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::drive::SupercompilerStats;
use numlang::mir::supercompiler::mrsc::{MrscCostModel, MrscObjective};
use numlang::mir::supercompiler::supercompile_mir_function_with_stats;
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
        ("append3", include_str!("../bench/showdown/numlang/append3.nl")),
        ("kmp", include_str!("../bench/showdown/numlang/kmp.nl")),
        ("peano_mul", include_str!("../bench/showdown/numlang/peano_mul.nl")),
        ("tree_flip", include_str!("../bench/showdown/numlang/tree_flip.nl")),
        ("fib_matrix", include_str!("../bench/showdown/numlang/fib_matrix.nl")),
        ("tri_sum", include_str!("../bench/showdown/numlang/tri_sum.nl")),
        ("cubic_sum", include_str!("../bench/showdown/numlang/cubic_sum.nl")),
        ("pow2_mod", include_str!("../bench/showdown/numlang/pow2_mod.nl")),
        ("hofstadter", include_str!("../bench/showdown/numlang/hofstadter.nl")),
        ("tri_sum_dyn", include_str!("../bench/showdown/numlang/tri_sum_dyn.nl")),
        ("cubic_sum_dyn", include_str!("../bench/showdown/numlang/cubic_sum_dyn.nl")),
        ("fib_matrix_dyn", include_str!("../bench/showdown/numlang/fib_matrix_dyn.nl")),
        ("pow2_mod_dyn", include_str!("../bench/showdown/numlang/pow2_mod_dyn.nl")),
        ("compose5", include_str!("../bench/showdown/numlang/compose5.nl")),
        ("map_map", include_str!("../bench/showdown/numlang/map_map.nl")),
        ("sum_map", include_str!("../bench/showdown/numlang/sum_map.nl")),
        ("stream_take", include_str!("../bench/showdown/numlang/stream_take.nl")),
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
    println!("Verified cost model invariant across all {} benchmark functions.", total_funcs_checked);
}
