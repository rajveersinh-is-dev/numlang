use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_supercompile_all(c: &mut Criterion) {
    let benchmarks = [
        ("nrev", include_str!("../bench/numlang/nrev.nl")),
        ("append3", include_str!("../bench/numlang/append3.nl")),
        ("stream_fusion", include_str!("../bench/numlang/stream_fusion.nl")),
        ("ackermann", include_str!("../bench/numlang/ackermann.nl")),
        ("fib_matrix", include_str!("../bench/numlang/fib_matrix.nl")),
        ("sieve", include_str!("../bench/numlang/sieve.nl")),
        ("matvec_4x4", include_str!("../bench/numlang/matvec_4x4.nl")),
        ("raytracer_sphere", include_str!("../bench/numlang/raytracer_sphere.nl")),
        ("tree_flip", include_str!("../bench/numlang/tree_flip.nl")),
        ("peano_mul", include_str!("../bench/numlang/peano_mul.nl")),
    ];

    for (name, source) in benchmarks {
        let tokens = numlang::token::tokenize(source).unwrap();
        let program = numlang::parser::parse(&tokens).unwrap();
        let typed = numlang::typecheck::typecheck(&program).unwrap();

        c.bench_function(&format!("supercompile_{}", name), |b| {
            b.iter(|| {
                let mut p = typed.clone();
                numlang::opt::supercompiler::supercompile_program(black_box(&mut p), None);
            });
        });
    }
}

criterion_group!(benches, bench_supercompile_all);
criterion_main!(benches);
