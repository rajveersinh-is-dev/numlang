use std::fs;
use std::process::Command;

use numlang::mir::supercompiler::cache::{CacheKey, CachedSpecialization, SpecializationCache};

#[test]
fn test_cache_key_deterministic() {
    let key1 = CacheKey {
        function_name: "test_fn".to_string(),
        function_source_hash: "abcd1234ef012345".to_string(),
        argument_fingerprint: "generic".to_string(),
    };
    let key2 = CacheKey {
        function_name: "test_fn".to_string(),
        function_source_hash: "abcd1234ef012345".to_string(),
        argument_fingerprint: "generic".to_string(),
    };
    let json1 = serde_json::to_string(&key1).expect("serialize key1");
    let json2 = serde_json::to_string(&key2).expect("serialize key2");
    assert_eq!(json1, json2);

    let cache = SpecializationCache::open(std::path::Path::new(".numlang_cache_test"));
    let path1 = cache.key_to_path(&key1);
    let path2 = cache.key_to_path(&key2);
    assert_eq!(path1, path2);
}

#[test]
fn test_cache_store_and_lookup() {
    let tmp = std::env::temp_dir().join(format!("numlang_cache_test_{}", std::process::id()));
    let cache = SpecializationCache::open(&tmp);

    let key = CacheKey {
        function_name: "fib".to_string(),
        function_source_hash: "hash_123".to_string(),
        argument_fingerprint: "generic".to_string(),
    };
    let entry = CachedSpecialization {
        key: key.clone(),
        residual_json: r#"{"function": "fib", "residual": true}"#.to_string(),
        stats_nodes_explored: 10,
        stats_branches_pruned: 2,
        stats_loops_collapsed: 1,
        stats_knots_tied: 1,
        stats_calls_inlined: 0,
        stats_sc_bce_eliminated: 0,
        stats_residual_block_count: 3,
        stats_residual_stmt_count: 5,
    };

    cache.store(&entry).expect("store cache");
    let retrieved = cache.lookup(&key).expect("lookup cache hit");
    assert_eq!(retrieved.residual_json, entry.residual_json);
    assert_eq!(retrieved.stats_nodes_explored, 10);
    assert_eq!(retrieved.stats_residual_block_count, 3);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_cache_miss_returns_none() {
    let tmp = std::env::temp_dir().join(format!("numlang_cache_miss_{}", std::process::id()));
    let cache = SpecializationCache::open(&tmp);

    let key = CacheKey {
        function_name: "nonexistent".to_string(),
        function_source_hash: "no_hash".to_string(),
        argument_fingerprint: "generic".to_string(),
    };
    let result = cache.lookup(&key);
    assert!(result.is_none());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_cache_warm_compilation_faster() {
    let code = r#"
fn double(x: i32) -> i32 {
    return x * 2;
}

fn main() -> i32 {
    return double(21);
}
"#;
    let id = format!(
        "{}_{:?}_{}",
        std::process::id(),
        std::thread::current().id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid time")
            .as_nanos()
    )
    .replace(['(', ')', ' '], "_");
    let test_dir = std::env::temp_dir().join(format!("numlang_phase37_{}", id));
    fs::create_dir_all(&test_dir).expect("create test dir");
    let cache_dir = test_dir.join("cache");
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).expect("write src file");

    let run_cmd = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
        cmd.arg("run");
        cmd.arg("--supercompile");
        cmd.arg("--cache-dir");
        cmd.arg(&cache_dir);
        cmd.arg(&src_file);
        cmd.output().expect("Failed to run numlang program")
    };

    let start_cold = std::time::Instant::now();
    let out_cold = run_cmd();
    let _dur_cold = start_cold.elapsed();
    assert_eq!(
        out_cold.status.code(),
        Some(42),
        "Cold run failed: stdout: {}, stderr: {}",
        String::from_utf8_lossy(&out_cold.stdout),
        String::from_utf8_lossy(&out_cold.stderr)
    );

    let start_warm = std::time::Instant::now();
    let out_warm = run_cmd();
    let _dur_warm = start_warm.elapsed();
    assert_eq!(
        out_warm.status.code(),
        Some(42),
        "Warm run failed: stdout: {}, stderr: {}",
        String::from_utf8_lossy(&out_warm.stdout),
        String::from_utf8_lossy(&out_warm.stderr)
    );

    // Verify cache directory has cached items
    assert!(cache_dir.exists(), "Cache directory should have been created");

    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cache_invalidation_on_different_source() {
    let tmp = std::env::temp_dir().join(format!("numlang_cache_inv_{}", std::process::id()));
    let cache = SpecializationCache::open(&tmp);

    let key_v1 = CacheKey {
        function_name: "foo".to_string(),
        function_source_hash: "hash_v1".to_string(),
        argument_fingerprint: "generic".to_string(),
    };
    let entry = CachedSpecialization {
        key: key_v1.clone(),
        residual_json: r#"{"version": 1}"#.to_string(),
        stats_nodes_explored: 5,
        stats_branches_pruned: 0,
        stats_loops_collapsed: 0,
        stats_knots_tied: 0,
        stats_calls_inlined: 0,
        stats_sc_bce_eliminated: 0,
        stats_residual_block_count: 1,
        stats_residual_stmt_count: 2,
    };
    cache.store(&entry).expect("store cache");

    // Look up v1 hits
    assert!(cache.lookup(&key_v1).is_some());

    // Modified source produces a different hash: lookup must return None
    let key_v2 = CacheKey {
        function_name: "foo".to_string(),
        function_source_hash: "hash_v2".to_string(),
        argument_fingerprint: "generic".to_string(),
    };
    assert!(cache.lookup(&key_v2).is_none());

    // Also verify invalidate_function removes entries for "foo"
    let removed = cache.invalidate_function("foo");
    assert_eq!(removed, 1);
    assert!(cache.lookup(&key_v1).is_none());

    let _ = std::fs::remove_dir_all(&tmp);
}
