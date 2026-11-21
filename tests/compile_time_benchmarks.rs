//! Compile-time benchmarks for the NumLang supercompiler.
//!
//! This closes the contract v1.0 gap in `BENCHMARK_CONTRACT.md` §7, where
//! compile-time was published as `N/A`.  The measured quantity is the
//! supercompiler *specialization stage* (typed-AST distillation, MIR lowering,
//! and MIR supercompilation), compared with the same pipeline without
//! supercompilation.  Linking and process startup are deliberately excluded so
//! the number isolates the cost introduced by the optimizer itself.
//!
//! Measurement protocol:
//! - monotonic clock only: `std::time::Instant` (QueryPerformanceCounter on
//!   Windows, `clock_gettime(CLOCK_MONOTONIC)` on POSIX);
//! - default: 5 discarded warmups + 30 measured rounds per benchmark per system;
//! - `COMPILE_TIME_QUICK=1` (or `QUICK_BENCHMARKS=1`) selects the contract's
//!   documented quick mode (2 warmups + 5 rounds) for CI smoke runs;
//! - every input is cloned before timing so mutation inside the compiler pass
//!   cannot bias later rounds, and every result is passed through
//!   `std::hint::black_box` so dead-code elimination cannot remove the work.
//!
//! Output: `bench/data/compile_times.csv` with columns
//! `benchmark_id,system,warmup_ns,median_compile_ns,p95_compile_ns,rounds,status`.
//! `warmup_ns` is the summed wall-clock duration of the discarded warmups.
//! Missing sources yield `SKIPPED_MISSING_SOURCE` rows rather than a failure.

use std::fs::{self, File};
use std::hint::black_box;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use numlang::compiler::{distill_and_optimize, CompilerConfig};
use numlang::mir::lower::{lower_program, MirProgram};
use numlang::mir::supercompiler::supercompile_mir_program;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use numlang::typecheck::typed_ast::TypedProgram;

const CSV_HEADER: &str =
    "benchmark_id,system,warmup_ns,median_compile_ns,p95_compile_ns,rounds,status";

/// The 18 contract-registered canonical workloads plus 4 extended realistic
/// workloads added under `bench/showdown/extended/`.
const BENCHMARKS: &[(&str, &str)] = &[
    ("nrev", "bench/showdown/numlang/nrev.nl"),
    ("append3", "bench/showdown/numlang/append3.nl"),
    ("kmp", "bench/showdown/numlang/kmp.nl"),
    ("peano_mul", "bench/showdown/numlang/peano_mul.nl"),
    ("tree_flip", "bench/showdown/numlang/tree_flip.nl"),
    ("fib_matrix", "bench/showdown/numlang/fib_matrix.nl"),
    ("tri_sum", "bench/showdown/numlang/tri_sum.nl"),
    ("cubic_sum", "bench/showdown/numlang/cubic_sum.nl"),
    ("pow2_mod", "bench/showdown/numlang/pow2_mod.nl"),
    ("hofstadter", "bench/showdown/numlang/hofstadter.nl"),
    ("compose5", "bench/showdown/numlang/compose5.nl"),
    ("map_map", "bench/showdown/numlang/map_map.nl"),
    ("sum_map", "bench/showdown/numlang/sum_map.nl"),
    ("stream_take", "bench/showdown/numlang/stream_take.nl"),
    ("expr_eval", "bench/showdown/extended/numlang/expr_eval.nl"),
    ("window_stats", "bench/showdown/extended/numlang/window_stats.nl"),
    ("record_pipeline", "bench/showdown/extended/numlang/record_pipeline.nl"),
    ("nested_stencil", "bench/showdown/extended/numlang/nested_stencil.nl"),
];

const SYSTEM_SC: &str = "NumLang-SC-specialize";
const SYSTEM_BASE: &str = "NumLang-Base-lower";

struct Measurement {
    warmup_ns: u64,
    median_ns: u64,
    p95_ns: u64,
    rounds: usize,
}

struct Row {
    benchmark_id: &'static str,
    system: &'static str,
    measurement: Option<Measurement>,
    status: &'static str,
}

impl Row {
    fn skipped(benchmark_id: &'static str, system: &'static str, status: &'static str) -> Self {
        Self {
            benchmark_id,
            system,
            measurement: None,
            status,
        }
    }

    fn measured(benchmark_id: &'static str, system: &'static str, measurement: Measurement) -> Self {
        Self {
            benchmark_id,
            system,
            measurement: Some(measurement),
            status: "SUCCESS",
        }
    }

    fn csv_line(&self) -> String {
        match &self.measurement {
            Some(m) => format!(
                "{},{},{},{},{},{},{}",
                self.benchmark_id,
                self.system,
                m.warmup_ns,
                m.median_ns,
                m.p95_ns,
                m.rounds,
                self.status
            ),
            None => format!(
                "{},{},0,0,0,0,{}",
                self.benchmark_id, self.system, self.status
            ),
        }
    }
}

fn is_quick_mode() -> bool {
    std::env::var("COMPILE_TIME_QUICK").is_ok() || std::env::var("QUICK_BENCHMARKS").is_ok()
}

fn sc_pass(typed: &mut TypedProgram) -> MirProgram {
    let config = CompilerConfig {
        ho_distill: true,
        supercompile: true,
        ..CompilerConfig::default()
    };
    distill_and_optimize(typed, &config);
    let mut mir = lower_program(typed);
    supercompile_mir_program(&mut mir);
    mir
}

fn base_pass(typed: &mut TypedProgram) -> MirProgram {
    let config = CompilerConfig {
        ho_distill: false,
        supercompile: false,
        ..CompilerConfig::default()
    };
    distill_and_optimize(typed, &config);
    lower_program(typed)
}

/// Times `work(input)` for `warmup` discarded calls followed by `rounds`
/// measured calls. Inputs are owned here and prepared outside the timed region.
fn measure<F>(inputs: Vec<TypedProgram>, mut work: F, warmup: usize, rounds: usize) -> Measurement
where
    F: FnMut(TypedProgram) -> MirProgram,
{
    let mut warmup_total_ns: u64 = 0;
    let mut measured_ns: Vec<u64> = Vec::with_capacity(rounds);

    for (index, input) in inputs.into_iter().enumerate() {
        let start = Instant::now();
        let mir = work(input);
        let elapsed_ns = start.elapsed().as_nanos() as u64;
        black_box(mir);

        if index < warmup {
            warmup_total_ns = warmup_total_ns.saturating_add(elapsed_ns);
        } else {
            measured_ns.push(elapsed_ns);
        }
    }

    measured_ns.sort_unstable();
    let len = measured_ns.len();
    let median_ns = measured_ns[len / 2];
    // Nearest-rank p95, matching tests/supercompiler_showdown.rs.
    let p95_index = ((len as f64) * 0.95).min((len - 1) as f64) as usize;
    let p95_ns = measured_ns[p95_index];

    Measurement {
        warmup_ns: warmup_total_ns,
        median_ns,
        p95_ns,
        rounds: len,
    }
}

fn load_typed_program(path: &Path) -> Result<TypedProgram, &'static str> {
    let source = fs::read_to_string(path).map_err(|_| "SKIPPED_UNREADABLE_SOURCE")?;
    let tokens = tokenize(&source).map_err(|_| "SKIPPED_LEX_FAILED")?;
    let ast = parse(&tokens).map_err(|_| "SKIPPED_PARSE_FAILED")?;
    typecheck(&ast).map_err(|_| "SKIPPED_TYPECHECK_FAILED")
}

fn write_csv(path: &Path, rows: &[Row]) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "{}", CSV_HEADER)?;
    for row in rows {
        writeln!(writer, "{}", row.csv_line())?;
    }
    writer.flush()
}

#[test]
fn test_compile_time_benchmarks() {
    let quick = is_quick_mode();
    let warmup = if quick { 2 } else { 5 };
    let rounds = if quick { 5 } else { 30 };

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let csv_path = root.join("bench/data/compile_times.csv");
    let mut rows = Vec::with_capacity(BENCHMARKS.len() * 2);

    println!(
        "\nCompile-time benchmark mode: {} (warmup={}, rounds={})",
        if quick { "QUICK" } else { "FULL" },
        warmup,
        rounds
    );

    for (benchmark_id, rel) in BENCHMARKS {
        let path = root.join(rel);
        if !path.is_file() {
            eprintln!("[compile-time] missing source: {}", path.display());
            rows.push(Row::skipped(
                benchmark_id,
                SYSTEM_SC,
                "SKIPPED_MISSING_SOURCE",
            ));
            rows.push(Row::skipped(
                benchmark_id,
                SYSTEM_BASE,
                "SKIPPED_MISSING_SOURCE",
            ));
            continue;
        }

        let typed = match load_typed_program(&path) {
            Ok(t) => t,
            Err(status) => {
                eprintln!("[compile-time] {}: {}", benchmark_id, status);
                rows.push(Row::skipped(benchmark_id, SYSTEM_SC, status));
                rows.push(Row::skipped(benchmark_id, SYSTEM_BASE, status));
                continue;
            }
        };

        // Clone once per round so no timed region pays for AST cloning and so
        // every round starts from byte-identical compiler input.
        let count = warmup + rounds;
        let sc_inputs: Vec<TypedProgram> = (0..count).map(|_| typed.clone()).collect();
        let base_inputs: Vec<TypedProgram> = (0..count).map(|_| typed.clone()).collect();

        let sc_measurement = measure(sc_inputs, |mut input| sc_pass(&mut input), warmup, rounds);
        rows.push(Row::measured(benchmark_id, SYSTEM_SC, sc_measurement));

        let base_measurement =
            measure(base_inputs, |mut input| base_pass(&mut input), warmup, rounds);
        rows.push(Row::measured(benchmark_id, SYSTEM_BASE, base_measurement));

        println!(
            "  {:>16}: SC median {:>12} ns | Base median {:>12} ns",
            benchmark_id,
            rows[rows.len() - 2]
                .measurement
                .as_ref()
                .map_or(0, |m| m.median_ns),
            rows[rows.len() - 1]
                .measurement
                .as_ref()
                .map_or(0, |m| m.median_ns),
        );
    }

    write_csv(&csv_path, &rows)
        .unwrap_or_else(|e| panic!("failed to write {}: {}", csv_path.display(), e));

    let successful = rows.iter().filter(|r| r.status == "SUCCESS").count();
    println!(
        "[compile-time] wrote {} rows ({} measured, {} skipped) to {}",
        rows.len(),
        successful,
        rows.len() - successful,
        csv_path.display()
    );
}
