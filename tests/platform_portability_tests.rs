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
        if let Ok(src) = fs::read_to_string(&p) {
            assert!(
                src.contains("#ifdef _WIN32")
                    || src.contains("clock_gettime")
                    || src.contains("time.h"),
                "{} must support cross-platform timing",
                name
            );
        }
    }
}

#[test]
fn test_linker_handles_posix_bench_mode() {
    let content = fs::read_to_string("src/codegen/linker.rs").expect("linker.rs must exist");

    assert!(
        content.contains("ENTRY_BENCH_C"),
        "linker.rs must embed entry_bench.c for POSIX benchmark mode"
    );
    assert!(
        content.contains("-lm") && content.contains("-no-pie"),
        "linker.rs link_unix must pass -lm and -no-pie"
    );
}

#[test]
fn test_cranelift_backend_abstracts_win32_symbols() {
    let mut raw = fs::read_to_string("src/codegen/cranelift/mod.rs")
        .or_else(|_| fs::read_to_string("src/codegen/cranelift_backend.rs"))
        .expect("cranelift backend must exist");
    if let Ok(intrinsics) = fs::read_to_string("src/codegen/cranelift/intrinsics.rs") {
        raw.push('\n');
        raw.push_str(&intrinsics);
    }
    let content = raw.replace("\r\n", "\n");

    // Ensure conditional compilation guards exist for Windows vs POSIX
    assert!(
        content.contains("#[cfg(target_os = \"windows\")]")
            && content.contains("#[cfg(not(target_os = \"windows\"))]"),
        "cranelift_backend.rs must contain target_os conditional guards for syscall symbols"
    );
    assert!(
        content.contains(".declare_function(\"exit\", Linkage::Import"),
        "cranelift_backend.rs must declare POSIX exit symbol"
    );
    assert!(
        content.contains(".declare_function(\"write\", Linkage::Import"),
        "cranelift_backend.rs must declare POSIX write symbol"
    );
    assert!(
        content.contains(".declare_function(\"malloc\", Linkage::Import"),
        "cranelift_backend.rs must declare POSIX malloc symbol"
    );
}

#[test]
fn test_llvm_backend_abstracts_win32_symbols() {
    let raw =
        fs::read_to_string("src/codegen/llvm_backend.rs").expect("llvm_backend.rs must exist");
    let content = raw.replace("\r\n", "\n");

    assert!(
        content.contains("#[cfg(target_os = \"windows\")]")
            && content.contains("#[cfg(not(target_os = \"windows\"))]"),
        "llvm_backend.rs must guard Windows vs POSIX symbols"
    );
    assert!(
        content.contains("module.add_function(\"exit\","),
        "llvm_backend.rs must declare exit function on non-Windows"
    );
    assert!(
        content.contains("module.add_function(\"write\","),
        "llvm_backend.rs must declare write function on non-Windows"
    );
}

#[test]
fn test_cranelift_code_emission_on_current_host() {
    use numlang::codegen::compile_to_obj;
    use numlang::parser::parse;
    use numlang::token::tokenize;
    use numlang::typecheck::typecheck;

    let src = "fn main() -> i64 { return 42; }";
    let tokens = tokenize(src).unwrap();
    let ast = parse(&tokens).unwrap();
    let typed = typecheck(&ast).unwrap();
    let obj_bytes = compile_to_obj(&typed).unwrap();

    assert!(
        !obj_bytes.is_empty(),
        "Generated object bytes must not be empty"
    );
}
