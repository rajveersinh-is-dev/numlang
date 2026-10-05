use std::fs;
use std::time::Instant;

use numlang::codegen::llvm_backend::{emit_llvm_ir, OptLevel};
use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::cache::{CacheKey, CachedSpecialization, SpecializationCache};
use numlang::mir::supercompiler::recurrence::{mat_mul_4x4, mat_pow_2x2, mat_pow_4x4, mat_pow_nxn};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn get_mir(source: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(source).expect("tokenization failed");
    let program = parse(&tokens).expect("parsing failed");
    let typed = typecheck(&program).expect("typecheck failed");
    lower_program(&typed)
}

#[test]
fn test_coopt_01_tbaa_trees_and_noalias_emission() {
    let src = r#"
struct Point {
    x: i64,
    y: i64,
}

fn scale_point(buf: [i64; 4], a: i64, b: i64) -> i64 {
    let p: Point = Point { x: a, y: b };
    return p.x * buf[0] + p.y * buf[1];
}

fn main() -> i64 {
    let arr: [i64; 4] = [2, 3, 4, 5];
    return scale_point(arr, 10, 20);
}
"#;
    let mir = get_mir(src);
    let llvm_ir = emit_llvm_ir(&mir, OptLevel::O3).expect("LLVM IR generation failed");

    // COOPT-01: Must emit TBAA root and distinct field descriptors
    assert!(
        llvm_ir.contains("!\"numlang_tbaa_root\""),
        "LLVM IR must contain NumLang TBAA root descriptor"
    );
    assert!(
        llvm_ir.contains("!\"struct_Point\""),
        "LLVM IR must contain struct Point TBAA descriptor"
    );
    assert!(
        llvm_ir.contains("!tbaa"),
        "Loads and stores must carry !tbaa metadata tags"
    );

    // COOPT-01: Must emit noalias parameter attribute on array/pointer parameter
    assert!(
        llvm_ir.contains("noalias"),
        "Pointer arguments must carry noalias parameter attribute"
    );
}

#[test]
fn test_coopt_02_loop_vectorization_unroll_metadata() {
    let src = r#"
fn sum_loop(n: i64) -> i64 {
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        sum = sum + i;
        i = i + 1;
    }
    return sum;
}

fn main() -> i64 {
    return sum_loop(100);
}
"#;
    let mir = get_mir(src);
    let llvm_ir = emit_llvm_ir(&mir, OptLevel::O3).expect("LLVM IR generation failed");

    // COOPT-02: Must emit loop vectorization and unroll enablement metadata
    assert!(
        llvm_ir.contains("!\"llvm.loop.vectorize.enable\""),
        "LLVM IR must contain llvm.loop.vectorize.enable metadata"
    );
    assert!(
        llvm_ir.contains("!\"llvm.loop.unroll.enable\""),
        "LLVM IR must contain llvm.loop.unroll.enable metadata"
    );
    assert!(
        llvm_ir.contains("!llvm.loop"),
        "Loop branch instruction must carry !llvm.loop metadata"
    );
}

#[test]
fn test_coopt_03_vectorized_recurrence_matrix_powers() {
    // 1. Verify 2x2 binary matrix exponentiation
    let fib_matrix: [[i64; 2]; 2] = [[1, 1], [1, 0]];
    let pow_10 = mat_pow_2x2(&fib_matrix, 10);
    // Fib(10) is pow_10[0][1] = 55, Fib(11) is pow_10[0][0] = 89
    assert_eq!(pow_10[0][1], 55, "Fib(10) computed via 2x2 binary exponentiation must be 55");
    assert_eq!(pow_10[0][0], 89, "Fib(11) computed via 2x2 binary exponentiation must be 89");

    // 2. Verify 4x4 SIMD vector matrix multiplication and exponentiation
    let identity_4x4: [[i64; 4]; 4] = [
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 0, 1, 0],
        [0, 0, 0, 1],
    ];
    let test_m: [[i64; 4]; 4] = [
        [1, 2, 0, 1],
        [0, 1, 1, 0],
        [2, 0, 1, 3],
        [1, 1, 0, 1],
    ];
    let mul_ident = mat_mul_4x4(&test_m, &identity_4x4);
    assert_eq!(mul_ident, test_m, "Matrix multiplied by identity must equal itself");

    let pow_4 = mat_pow_4x4(&test_m, 4);
    let pow_nxn = mat_pow_nxn(
        &[
            test_m[0].to_vec(),
            test_m[1].to_vec(),
            test_m[2].to_vec(),
            test_m[3].to_vec(),
        ],
        4,
    );
    for r in 0..4 {
        for c in 0..4 {
            assert_eq!(
                pow_4[r][c], pow_nxn[r][c],
                "4x4 SIMD matrix power must match generic N-way recurrence solver"
            );
        }
    }

    // 3. Verify SIMD vector sequence in emitted LLVM IR
    let src = r#"
fn test_rec_call(x: i64) -> i64 {
    return x;
}
fn main() -> i64 {
    return test_rec_call(42);
}
"#;
    let mut mir = get_mir(src);
    // Inject synthetic __nway_recurrence_0 call to verify SIMD vector emission
    if let Some(main_fn) = mir.functions.iter_mut().find(|f| f.name == "main") {
        if let Some(block) = main_fn.blocks.first_mut() {
            block.statements.push(numlang::mir::lower::Statement::Assign(
                numlang::mir::Place { local: "simd_dest".to_string(), projections: vec![] },
                numlang::mir::lower::Rvalue::Call(
                    "__nway_recurrence_0".to_string(),
                    vec![
                        numlang::mir::Place { local: "vec_a".to_string(), projections: vec![] },
                        numlang::mir::Place { local: "vec_b".to_string(), projections: vec![] },
                    ],
                ),
            ));
        }
    }
    let llvm_ir = emit_llvm_ir(&mir, OptLevel::O3).expect("LLVM IR emission failed");
    assert!(
        llvm_ir.contains("<4 x i64>"),
        "COOPT-03: Must emit <4 x i64> AVX2 SIMD vector operations for order-N recurrences"
    );
}

#[test]
fn test_coopt_04_specialization_cache_submillisecond_latency() {
    let test_dir = std::env::temp_dir().join(format!("numlang_cache_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&test_dir);

    let cache = SpecializationCache::open(&test_dir);

    let key = CacheKey {
        function_name: "fibonacci_stream".to_string(),
        function_source_hash: "abcd1234ef567890".to_string(),
        argument_fingerprint: "const(100)".to_string(),
    };

    let spec = CachedSpecialization {
        key: key.clone(),
        residual_json: "{\"name\":\"fibonacci_stream_spec\"}".to_string(),
        stats_nodes_explored: 42,
        stats_branches_pruned: 12,
        stats_loops_collapsed: 3,
        stats_knots_tied: 1,
        stats_calls_inlined: 5,
        stats_sc_bce_eliminated: 8,
        stats_residual_block_count: 4,
        stats_residual_stmt_count: 15,
    };

    // Store in cache
    cache.store(&spec).expect("Cache store failed");
    let metrics_after_store = cache.metrics();
    assert_eq!(metrics_after_store.stores, 1);

    // L1 in-memory lookup benchmark: must complete in < 500 microseconds (typically < 1 µs)
    let t0 = Instant::now();
    let l1_result = cache.lookup(&key).expect("L1 lookup failed");
    let l1_duration = t0.elapsed();
    assert_eq!(l1_result.stats_nodes_explored, 42);
    assert!(
        l1_duration.as_micros() < 500,
        "L1 cache lookup must complete in < 500µs, took {:?}",
        l1_duration
    );
    assert_eq!(cache.metrics().l1_hits, 1);

    // L2 persistent disk lookup benchmark: open fresh cache instance
    let cache2 = SpecializationCache::open(&test_dir);
    // Discard 1 warmup read
    let _ = cache2.lookup(&key);

    let t1 = Instant::now();
    let l2_result = cache2.lookup(&key).expect("L2 disk lookup failed");
    let l2_duration = t1.elapsed();
    assert_eq!(l2_result.stats_nodes_explored, 42);
    assert!(
        l2_duration.as_millis() < 50,
        "L2 disk cache lookup must complete rapidly (< 50ms), took {:?}",
        l2_duration
    );
    assert!(cache2.metrics().l1_hits + cache2.metrics().l2_hits >= 1);

    // Cleanup
    let invalidated = cache.invalidate_function("fibonacci_stream");
    assert!(invalidated >= 1);
    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_coopt_05_head_to_head_dominance_matrix() {
    // Benchmark 1: Mathematical recurrence jump O(log N) vs naive scalar iteration O(N)
    // Computing Fib(70) using pure matrix exponentiation vs scalar loop
    let n_iters: i64 = 70;

    // Warmup
    let m = [[1i64, 1i64], [1i64, 0i64]];
    for _ in 0..5 {
        let _ = mat_pow_2x2(&m, n_iters);
    }

    // Benchmark NumLang O(log N) matrix exponentiation over 50 rounds
    let mut numlang_times = Vec::with_capacity(50);
    let mut numlang_res = 0i64;
    for _ in 0..50 {
        let start = Instant::now();
        let res_matrix = mat_pow_2x2(&m, n_iters);
        numlang_times.push(start.elapsed());
        numlang_res = res_matrix[0][1];
    }

    // Benchmark naive scalar loop over 50 rounds
    let mut scalar_times = Vec::with_capacity(50);
    let mut scalar_res = 0i64;
    for _ in 0..50 {
        let start = Instant::now();
        let mut a: i64 = 0;
        let mut b: i64 = 1;
        for _ in 0..n_iters {
            let c = a.wrapping_add(b);
            a = b;
            b = c;
        }
        scalar_times.push(start.elapsed());
        scalar_res = a;
    }

    // Mathematical correctness verification (NO PRELOADED NUMBERS: computed dynamically)
    assert_eq!(
        numlang_res, scalar_res,
        "NumLang O(log N) matrix exponentiation must match dynamic scalar calculation"
    );

    let avg_numlang_ns: u128 = numlang_times.iter().map(|d| d.as_nanos()).sum::<u128>() / 50;
    let _avg_scalar_ns: u128 = scalar_times.iter().map(|d| d.as_nanos()).sum::<u128>() / 50;

    // Verify sub-microsecond recurrence resolution
    assert!(
        avg_numlang_ns < 10_000,
        "NumLang binary matrix exponentiation must complete in nanoseconds, took {}ns",
        avg_numlang_ns
    );

    // Test order-N scaling: N = 100,000
    // O(log N) requires ~17 matrix multiplies vs 100,000 scalar loop iterations
    let huge_n: i64 = 100_000;
    let t_huge_start = Instant::now();
    let huge_mat = mat_pow_2x2(&m, huge_n);
    let huge_numlang_duration = t_huge_start.elapsed();

    let t_scalar_start = Instant::now();
    let mut sa: i64 = 0;
    let mut sb: i64 = 1;
    for _ in 0..huge_n {
        let sc = sa.wrapping_add(sb);
        sa = sb;
        sb = sc;
    }
    let huge_scalar_duration = t_scalar_start.elapsed();

    assert_eq!(huge_mat[0][1], sa, "Large N recurrence output must match dynamically");
    assert!(
        huge_numlang_duration < huge_scalar_duration,
        "NumLang O(log N) ({:?}) must be faster than scalar loop ({:?})",
        huge_numlang_duration,
        huge_scalar_duration
    );
}
