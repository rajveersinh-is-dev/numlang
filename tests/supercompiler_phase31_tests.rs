use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::drive::{SupercompilerDriver, WhistleKind};
use numlang::mir::supercompiler::residualize::residualize_process_tree;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use std::fs;
use std::path::PathBuf;

fn get_mir(source: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(source).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    lower_program(&typed)
}

#[test]
fn test_termination_witness_fib() {
    let src = r#"
fn fib(n: i64, a: i64, b: i64) -> i64 {
    let mut cur_a: i64 = a;
    let mut cur_b: i64 = b;
    let mut i: i64 = n;
    while i > 0 {
        let t: i64 = cur_a + cur_b;
        cur_a = cur_b;
        cur_b = t;
        i = i - 1;
    }
    return cur_a;
}

fn main() -> i64 {
    return fib(10, 0, 1);
}
"#;
    let mir = get_mir(src);
    let fib_func = mir.functions.iter().find(|f| f.name == "fib").unwrap();
    let driver = SupercompilerDriver::new(fib_func);
    let tree = driver.run();

    assert!(
        !tree.witness.firings.is_empty(),
        "tree.witness.firings must be non-empty"
    );
    let has_he = tree
        .witness
        .firings
        .iter()
        .any(|f| f.kind == WhistleKind::HomeomorphicEmbedding);
    assert!(
        has_he,
        "Expected at least one HomeomorphicEmbedding whistle firing in fib witness, got: {:?}",
        tree.witness.firings
    );
}

#[test]
#[allow(clippy::len_zero)]
fn test_termination_witness_kmp() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let kmp_path = root.join("bench/numlang/kmp.nl");
    let src = fs::read_to_string(&kmp_path).expect("read kmp.nl");
    let mir = get_mir(&src);

    let kmp_func = mir
        .functions
        .iter()
        .find(|f| f.name == "main")
        .expect("main function in kmp.nl");

    let driver = SupercompilerDriver::new(kmp_func).with_program_functions(&mir.functions);
    let tree = driver.run();

    assert!(
        !tree.witness.firings.is_empty(),
        "tree.witness.firings for KMP must be non-empty"
    );
    assert!(
        tree.witness.firings.len() >= 1,
        "KMP termination witness firings count must be >= 1"
    );
}

#[test]
#[allow(clippy::len_zero)]
fn test_termination_witness_all_benchmarks() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bench_files = [
        "power_spec.nl",
        "nrev.nl",
        "kmp.nl",
        "double_nrev.nl",
        "peano_mul.nl",
    ];

    for file_name in &bench_files {
        let path = root.join("bench/numlang").join(file_name);
        let src = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read benchmark {:?}: {}", path, e));
        let mir = get_mir(&src);

        let main_func = mir
            .functions
            .iter()
            .find(|f| f.name == "main")
            .unwrap_or_else(|| panic!("main function in {}", file_name));

        let driver = SupercompilerDriver::new(main_func).with_program_functions(&mir.functions);
        let tree = driver.run();

        assert!(
            !tree.witness.firings.is_empty() || tree.stats.loops_collapsed > 0,
            "Benchmark {} main must have witness firings or collapsed loops (firings: {}, collapsed: {})",
            file_name,
            tree.witness.firings.len(),
            tree.stats.loops_collapsed
        );
    }
}

#[test]
fn test_order3_recurrence_symbolic_n() {
    let src = r#"
fn trib(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 0;
    let mut c: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let next: i64 = a + b + c;
        a = b;
        b = c;
        c = next;
        i = i + 1;
    }
    return a;
}

fn main() -> i64 {
    return trib(10);
}
"#;
    let mir = get_mir(src);
    let trib_func = mir.functions.iter().find(|f| f.name == "trib").unwrap();
    let driver = SupercompilerDriver::new(trib_func);
    let tree = driver.run();
    assert!(
        tree.stats.loops_collapsed >= 1,
        "Expected at least 1 loop to be collapsed via order-3 recurrence solver, got {}",
        tree.stats.loops_collapsed
    );

    let residualized = residualize_process_tree(&tree, trib_func);
    let mir_str = format!("{:#?}", residualized);
    assert!(
        mir_str.contains("__order3_recurrence"),
        "Residualized MIR must contain call to __order3_recurrence intrinsic. MIR:\n{}",
        mir_str
    );
}
