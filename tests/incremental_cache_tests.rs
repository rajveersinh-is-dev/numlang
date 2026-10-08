use numlang::mir::lower::{lower_program, MirFunction, MirProgram};
use numlang::mir::supercompiler::cache::{
    compute_composite_hash, CacheKey, CachedSpecialization, SpecializationCache,
};
use numlang::mir::supercompiler::{supercompile_mir_program_with_cache, SupercompileMode};
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::process::Command;

fn compile_src_to_mir(src: &str) -> MirProgram {
    let tokens = tokenize(src).expect("tokenize");
    let ast = parse(&tokens).expect("parse");
    let typed = typecheck(&ast).expect("typecheck");
    lower_program(&typed)
}

fn dummy_entry(func_name: &str, hash: &str) -> CachedSpecialization {
    CachedSpecialization {
        key: CacheKey {
            function_name: func_name.to_string(),
            function_source_hash: hash.to_string(),
            argument_fingerprint: "generic".to_string(),
        },
        residual_json: format!(r#"{{"name": "{}"}}"#, func_name),
        stats_nodes_explored: 1,
        stats_branches_pruned: 0,
        stats_loops_collapsed: 0,
        stats_knots_tied: 0,
        stats_calls_inlined: 0,
        stats_sc_bce_eliminated: 0,
        stats_residual_block_count: 1,
        stats_residual_stmt_count: 1,
    }
}

#[test]
fn test_dependency_graph_tracking() {
    let tmp = std::env::temp_dir().join(format!("numlang_inc_dep_{}", std::process::id()));
    let cache = SpecializationCache::open(&tmp);

    let mut c1 = HashSet::new();
    c1.insert("leaf1".to_string());
    c1.insert("leaf2".to_string());
    cache.record_dependencies("caller1", c1);

    let mut c2 = HashSet::new();
    c2.insert("leaf2".to_string());
    c2.insert("leaf3".to_string());
    cache.record_dependencies("caller2", c2);

    let graph = cache.dependency_graph();
    assert_eq!(
        graph.callers_to_callees.get("caller1"),
        Some(&vec!["leaf1".to_string(), "leaf2".to_string()])
    );
    assert_eq!(
        graph.callers_to_callees.get("caller2"),
        Some(&vec!["leaf2".to_string(), "leaf3".to_string()])
    );

    assert_eq!(
        graph.callees_to_callers.get("leaf1"),
        Some(&vec!["caller1".to_string()])
    );
    let mut leaf2_callers = graph.callees_to_callers.get("leaf2").cloned().unwrap();
    leaf2_callers.sort();
    assert_eq!(
        leaf2_callers,
        vec!["caller1".to_string(), "caller2".to_string()]
    );
    assert_eq!(
        graph.callees_to_callers.get("leaf3"),
        Some(&vec!["caller2".to_string()])
    );

    // Update caller1 to only call leaf1: leaf2 callers must update
    let mut c1_new = HashSet::new();
    c1_new.insert("leaf1".to_string());
    cache.record_dependencies("caller1", c1_new);

    let graph_updated = cache.dependency_graph();
    assert_eq!(
        graph_updated.callees_to_callers.get("leaf2"),
        Some(&vec!["caller2".to_string()])
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_transitive_invalidation_dag() {
    let tmp = std::env::temp_dir().join(format!("numlang_inc_dag_{}", std::process::id()));
    let cache = SpecializationCache::open(&tmp);

    // DAG: main -> calc -> helper -> leaf
    // Also: other_root -> other_leaf
    let mut h_leaf = HashSet::new();
    h_leaf.insert("leaf".to_string());
    cache.record_dependencies("helper", h_leaf);

    let mut h_helper = HashSet::new();
    h_helper.insert("helper".to_string());
    cache.record_dependencies("calc", h_helper);

    let mut h_calc = HashSet::new();
    h_calc.insert("calc".to_string());
    cache.record_dependencies("main", h_calc);

    let mut h_other = HashSet::new();
    h_other.insert("other_leaf".to_string());
    cache.record_dependencies("other_root", h_other);

    // Populate cache entries
    let funcs = ["main", "calc", "helper", "leaf", "other_root", "other_leaf"];
    for name in &funcs {
        let entry = dummy_entry(name, "hash1");
        cache.store(&entry).expect("store");
        assert!(cache.lookup(&entry.key).is_some());
    }

    // Invalidate "leaf": must invalidate leaf, helper, calc, main
    let invalidated = cache.invalidate_transitive("leaf");
    assert!(invalidated.contains(&"leaf".to_string()));
    assert!(invalidated.contains(&"helper".to_string()));
    assert!(invalidated.contains(&"calc".to_string()));
    assert!(invalidated.contains(&"main".to_string()));
    assert!(!invalidated.contains(&"other_root".to_string()));
    assert!(!invalidated.contains(&"other_leaf".to_string()));

    // Lookups for invalidated functions must return None
    for name in &["leaf", "helper", "calc", "main"] {
        let key = dummy_entry(name, "hash1").key;
        assert!(
            cache.lookup(&key).is_none(),
            "{} should be invalidated",
            name
        );
    }

    // Lookups for unaffected functions must remain valid
    for name in &["other_root", "other_leaf"] {
        let key = dummy_entry(name, "hash1").key;
        assert!(
            cache.lookup(&key).is_some(),
            "{} should remain in cache",
            name
        );
    }

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_composite_hash_propagation() {
    let src1 = r#"
fn leaf() -> i32 {
    return 10;
}

fn middle() -> i32 {
    return leaf() + 5;
}

fn top() -> i32 {
    return middle() * 2;
}

fn unrelated() -> i32 {
    return 99;
}
"#;

    let src2 = r#"
fn leaf() -> i32 {
    return 20;
}

fn middle() -> i32 {
    return leaf() + 5;
}

fn top() -> i32 {
    return middle() * 2;
}

fn unrelated() -> i32 {
    return 99;
}
"#;

    let mir1 = compile_src_to_mir(src1);
    let map1: HashMap<String, &MirFunction> =
        mir1.functions.iter().map(|f| (f.name.clone(), f)).collect();

    let hash_leaf1 = compute_composite_hash("leaf", &map1);
    let hash_mid1 = compute_composite_hash("middle", &map1);
    let hash_top1 = compute_composite_hash("top", &map1);
    let hash_unrelated1 = compute_composite_hash("unrelated", &map1);

    let mir2 = compile_src_to_mir(src2);
    let map2: HashMap<String, &MirFunction> =
        mir2.functions.iter().map(|f| (f.name.clone(), f)).collect();

    let hash_leaf2 = compute_composite_hash("leaf", &map2);
    let hash_mid2 = compute_composite_hash("middle", &map2);
    let hash_top2 = compute_composite_hash("top", &map2);
    let hash_unrelated2 = compute_composite_hash("unrelated", &map2);

    // Leaf changed -> leaf hash must differ
    assert_ne!(
        hash_leaf1, hash_leaf2,
        "leaf hash must change when body changes"
    );

    // Leaf changed -> middle calls leaf -> middle composite hash must change
    assert_ne!(
        hash_mid1, hash_mid2,
        "middle composite hash must change when leaf changes"
    );

    // Middle changed -> top calls middle -> top composite hash must change
    assert_ne!(
        hash_top1, hash_top2,
        "top composite hash must change when callee changes"
    );

    // Unrelated function was NOT modified and does not call leaf -> hash must be identical
    assert_eq!(
        hash_unrelated1, hash_unrelated2,
        "unrelated function composite hash must remain identical"
    );
}

#[test]
fn test_incremental_cache_reuse_70_percent() {
    let tmp = std::env::temp_dir().join(format!("numlang_inc_70_{}", std::process::id()));
    let cache = SpecializationCache::open(&tmp);

    // 10-function module: leaf_a, caller_a (calls leaf_a), and f1..f8 (unrelated)
    let src_v1 = r#"
fn leaf_a() -> i32 { return 1; }
fn caller_a() -> i32 { return leaf_a() + 10; }
fn f1() -> i32 { return 100; }
fn f2() -> i32 { return 200; }
fn f3() -> i32 { return 300; }
fn f4() -> i32 { return 400; }
fn f5() -> i32 { return 500; }
fn f6() -> i32 { return 600; }
fn f7() -> i32 { return 700; }
fn f8() -> i32 { return 800; }
"#;

    // Pass 1: Cold build - all 10 functions are supercompiled and cached
    let mut mir1 = compile_src_to_mir(src_v1);
    supercompile_mir_program_with_cache(
        &mut mir1,
        SupercompileMode::Classic,
        "size",
        false,
        Some(&cache),
    );

    let m1 = cache.metrics();
    assert_eq!(m1.stores, 10, "First pass should store all 10 functions");

    // Pass 2: Modify ONLY leaf_a; caller_a depends on leaf_a; f1..f8 are unaffected
    let src_v2 = r#"
fn leaf_a() -> i32 { return 2; }
fn caller_a() -> i32 { return leaf_a() + 10; }
fn f1() -> i32 { return 100; }
fn f2() -> i32 { return 200; }
fn f3() -> i32 { return 300; }
fn f4() -> i32 { return 400; }
fn f5() -> i32 { return 500; }
fn f6() -> i32 { return 600; }
fn f7() -> i32 { return 700; }
fn f8() -> i32 { return 800; }
"#;

    let mut mir2 = compile_src_to_mir(src_v2);
    supercompile_mir_program_with_cache(
        &mut mir2,
        SupercompileMode::Classic,
        "size",
        false,
        Some(&cache),
    );

    let m2 = cache.metrics();
    let second_pass_hits = (m2.l1_hits + m2.l2_hits) - (m1.l1_hits + m1.l2_hits);
    let total_funcs = 10;
    let hit_rate = (second_pass_hits as f64) / (total_funcs as f64);

    // 8 out of 10 functions (f1..f8) should hit the cache -> 80% >= 70%
    assert!(
        hit_rate >= 0.70,
        "Expected >= 70% cache reuse on second pass with 1 modified leaf, got {:.1}% ({} hits / {} funcs)",
        hit_rate * 100.0,
        second_pass_hits,
        total_funcs
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_deps_json_persistence() {
    let tmp = std::env::temp_dir().join(format!("numlang_deps_json_{}", std::process::id()));
    let cache1 = SpecializationCache::open(&tmp);

    let mut c1 = HashSet::new();
    c1.insert("helper_a".to_string());
    c1.insert("helper_b".to_string());
    cache1.record_dependencies("algo_core", c1);

    let mut c2 = HashSet::new();
    c2.insert("algo_core".to_string());
    cache1.record_dependencies("main", c2);

    cache1.save_dependency_graph().expect("save deps.json");

    let deps_file = tmp.join("deps.json");
    assert!(deps_file.exists(), "deps.json must exist on disk");

    // Open a fresh cache instance on the same directory
    let cache2 = SpecializationCache::open(&tmp);
    let loaded_graph = cache2.dependency_graph();

    assert_eq!(
        loaded_graph.callers_to_callees.get("algo_core"),
        Some(&vec!["helper_a".to_string(), "helper_b".to_string()])
    );
    assert_eq!(
        loaded_graph.callers_to_callees.get("main"),
        Some(&vec!["algo_core".to_string()])
    );
    assert_eq!(
        loaded_graph.callees_to_callers.get("algo_core"),
        Some(&vec!["main".to_string()])
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_cycle_safe_dependency_handling() {
    let tmp = std::env::temp_dir().join(format!("numlang_cycle_safe_{}", std::process::id()));
    let cache = SpecializationCache::open(&tmp);

    // Cycle: f_even <-> f_odd
    let mut even_callees = HashSet::new();
    even_callees.insert("f_odd".to_string());
    cache.record_dependencies("f_even", even_callees);

    let mut odd_callees = HashSet::new();
    odd_callees.insert("f_even".to_string());
    cache.record_dependencies("f_odd", odd_callees);

    // Invalidation must terminate without hanging or crashing
    let invalidated = cache.invalidate_transitive("f_even");
    assert_eq!(invalidated.len(), 2);
    assert!(invalidated.contains(&"f_even".to_string()));
    assert!(invalidated.contains(&"f_odd".to_string()));

    // Cycle in compute_composite_hash
    let src = r#"
fn f_even(n: i32) -> i32 {
    if n == 0 {
        return 1;
    }
    return f_odd(n - 1);
}

fn f_odd(n: i32) -> i32 {
    if n == 0 {
        return 0;
    }
    return f_even(n - 1);
}
"#;
    let mir = compile_src_to_mir(src);
    let map: HashMap<String, &MirFunction> =
        mir.functions.iter().map(|f| (f.name.clone(), f)).collect();

    let hash_even = compute_composite_hash("f_even", &map);
    let hash_odd = compute_composite_hash("f_odd", &map);
    assert!(!hash_even.is_empty());
    assert!(!hash_odd.is_empty());

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_cli_incremental_flag() {
    let tmp = std::env::temp_dir().join(format!("numlang_inc_cli_{}", std::process::id()));
    let cache_dir = tmp.join("cache");
    let src_file = tmp.join("inc_test.nl");
    fs::create_dir_all(&tmp).expect("create test dir");

    let code = r#"
fn inc_helper(x: i32) -> i32 {
    return x * 3;
}

fn main() -> i32 {
    return inc_helper(14);
}
"#;
    fs::write(&src_file, code).expect("write src file");

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg("--incremental")
        .arg("--cache-dir")
        .arg(&cache_dir)
        .arg(&src_file)
        .output()
        .expect("run numlang with --incremental");

    assert_eq!(
        output.status.code(),
        Some(42),
        "Expected exit code 42, got {:?}. stdout: {}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify cache directory and deps.json exist
    assert!(cache_dir.exists(), "Cache directory should exist");
    let deps_file = cache_dir.join("deps.json");
    assert!(deps_file.exists(), "deps.json should be saved");

    let _ = fs::remove_dir_all(&tmp);
}
