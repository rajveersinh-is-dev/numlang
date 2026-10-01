use std::fs;
use std::path::Path;

#[test]
fn test_entry_bench_c_is_cross_platform() {
    let content = fs::read_to_string("src/codegen/entry_bench.c")
        .expect("src/codegen/entry_bench.c must exist");

    assert!(
        content.contains("#ifdef _WIN32"),
        "entry_bench.c must contain #ifdef _WIN32"
    );
    assert!(
        content.contains("clock_gettime(CLOCK_MONOTONIC"),
        "entry_bench.c must contain POSIX clock_gettime timing"
    );
    assert!(
        content.contains("exit((int)ret);"),
        "entry_bench.c must contain standard ISO C exit((int)ret);"
    );
    assert!(
        content.contains("numlang_main"),
        "entry_bench.c must reference numlang_main entry for POSIX"
    );
}

#[test]
fn test_c_benchmarks_are_cross_platform() {
    let benchmarks = [
        "ackermann.c",
        "append3.c",
        "double_nrev.c",
        "fib_matrix.c",
        "kmp.c",
        "matvec_4x4.c",
        "nrev.c",
        "peano_mul.c",
        "power_spec.c",
        "raytracer_sphere.c",
        "sieve.c",
        "stream_fusion.c",
        "tree_flip.c",
    ];

    for name in &benchmarks {
        let p = Path::new("bench/c").join(name);
        let src = fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", p.display(), e));

        assert!(
            src.contains("#ifdef _WIN32"),
            "{} must be guarded with #ifdef _WIN32",
            name
        );
        assert!(
            src.contains("clock_gettime(CLOCK_MONOTONIC"),
            "{} must include POSIX clock_gettime fallback",
            name
        );
    }
}

#[test]
fn test_runner_py_is_cross_platform() {
    let content = fs::read_to_string("bench/harness/runner.py")
        .expect("runner.py must exist");

    assert!(
        content.contains("exe_ext = \".exe\" if sys.platform == \"win32\" else \"\""),
        "runner.py must adapt executable extensions across platforms"
    );
    assert!(
        content.contains("clang") && content.contains("-lm"),
        "runner.py must support clang/gcc with -lm for Linux C compilation"
    );
}

#[test]
fn test_linker_handles_posix_bench_mode() {
    let content = fs::read_to_string("src/codegen/linker.rs")
        .expect("linker.rs must exist");

    assert!(
        content.contains("static ENTRY_BENCH_C: &str = include_str!(\"entry_bench.c\");"),
        "linker.rs must embed entry_bench.c for POSIX benchmark mode"
    );
    assert!(
        content.contains("cmd.arg(\"-lm\").arg(\"-no-pie\");") || content.contains(".arg(\"-lm\")"),
        "linker.rs link_unix must pass -lm and -no-pie"
    );
}

#[test]
fn test_cranelift_backend_abstracts_win32_symbols() {
    let raw = fs::read_to_string("src/codegen/cranelift_backend.rs")
        .expect("cranelift_backend.rs must exist");
    let content = raw.replace("\r\n", "\n");

    // Ensure ExitProcess and WriteFile declarations are guarded
    assert!(
        content.contains("#[cfg(target_os = \"windows\")]\n    exit_process_id: FuncId"),
        "exit_process_id must be guarded behind target_os = windows"
    );
    assert!(
        content.contains("#[cfg(not(target_os = \"windows\"))]\n    exit_id: FuncId"),
        "exit_id must be declared for non-Windows platforms"
    );
    assert!(
        content.contains("#[cfg(not(target_os = \"windows\"))]\n    write_id: FuncId"),
        "write_id must be declared for non-Windows platforms"
    );
}

#[test]
fn test_llvm_backend_abstracts_win32_symbols() {
    let raw = fs::read_to_string("src/codegen/llvm_backend.rs")
        .expect("llvm_backend.rs must exist");
    let content = raw.replace("\r\n", "\n");

    assert!(
        content.contains("TargetMachine::get_default_triple()"),
        "llvm_backend.rs must use TargetMachine::get_default_triple() rather than hardcoded Windows MSVC triple"
    );
    assert!(
        content.contains("#[cfg(target_os = \"windows\")]\n        {\n            // ExitProcess"),
        "llvm_backend.rs must guard ExitProcess behind target_os = windows"
    );
}
