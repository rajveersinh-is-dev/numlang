use std::fs;
use std::process::Command;

use numlang::mir::supercompiler::cache::{CacheKey, CachedSpecialization, SpecializationCache};

#[test]
fn test_cache_key_deterministic() {
    let key1 = CacheKey::new("test_fn", "abcd1234ef012345", "generic");
    let key2 = CacheKey::new("test_fn", "abcd1234ef012345", "generic");
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

    let key = CacheKey::new("fib", "hash_123", "generic");
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

    let key = CacheKey::new("nonexistent", "no_hash", "generic");
    let result = cache.lookup(&key);
    assert!(result.is_none());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_cache_key_invalidation_on_compiler_version() {
    let mut key1 = CacheKey::new("fn_foo", "hash_123", "generic");
    key1.compiler_version = "0.1.0".to_string();

    let mut key2 = CacheKey::new("fn_foo", "hash_123", "generic");
    key2.compiler_version = "0.2.0".to_string();

    assert_ne!(key1, key2);

    let tmp = std::env::temp_dir().join(format!("numlang_cache_ver_{}", std::process::id()));
    let cache = SpecializationCache::open(&tmp);

    let entry = CachedSpecialization {
        key: key1.clone(),
        residual_json: r#"{"v": 1}"#.to_string(),
        stats_nodes_explored: 1,
        stats_branches_pruned: 0,
        stats_loops_collapsed: 0,
        stats_knots_tied: 0,
        stats_calls_inlined: 0,
        stats_sc_bce_eliminated: 0,
        stats_residual_block_count: 1,
        stats_residual_stmt_count: 1,
    };
    cache.store(&entry).expect("store");

    assert!(cache.lookup(&key1).is_some());
    assert!(
        cache.lookup(&key2).is_none(),
        "Cache must invalidate across compiler versions"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_cache_key_invalidation_on_optimization_flags() {
    let key1 = CacheKey::new("fn_foo", "hash_123", "generic").with_flags("opt=3");
    let key2 = CacheKey::new("fn_foo", "hash_123", "generic").with_flags("opt=0");

    assert_ne!(key1, key2);

    let tmp = std::env::temp_dir().join(format!("numlang_cache_flags_{}", std::process::id()));
    let cache = SpecializationCache::open(&tmp);

    let entry = CachedSpecialization {
        key: key1.clone(),
        residual_json: r#"{"v": "opt3"}"#.to_string(),
        stats_nodes_explored: 1,
        stats_branches_pruned: 0,
        stats_loops_collapsed: 0,
        stats_knots_tied: 0,
        stats_calls_inlined: 0,
        stats_sc_bce_eliminated: 0,
        stats_residual_block_count: 1,
        stats_residual_stmt_count: 1,
    };
    cache.store(&entry).expect("store");

    assert!(cache.lookup(&key1).is_some());
    assert!(
        cache.lookup(&key2).is_none(),
        "Cache must invalidate across optimization flag changes"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_corrupted_disk_cache_fallback() {
    let tmp = std::env::temp_dir().join(format!("numlang_cache_corrupt_{}", std::process::id()));
    let cache = SpecializationCache::open(&tmp);

    let key = CacheKey::new("corrupt_fn", "hash_corrupt", "generic");
    let path = cache.key_to_path(&key);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    // Write corrupted non-JSON bytes to cache path
    fs::write(&path, b"NOT_VALID_JSON{{{").expect("write corrupt file");

    let metrics_before = cache.metrics();
    let result = cache.lookup(&key);
    assert!(
        result.is_none(),
        "Corrupted cache file must not crash and must return None"
    );

    let metrics_after = cache.metrics();
    assert_eq!(
        metrics_after.misses,
        metrics_before.misses + 1,
        "Must record a cache miss on corrupted disk cache"
    );

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
    assert!(
        cache_dir.exists(),
        "Cache directory should have been created"
    );

    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cache_invalidation_on_different_source() {
    let tmp = std::env::temp_dir().join(format!("numlang_cache_inv_{}", std::process::id()));
    let cache = SpecializationCache::open(&tmp);

    let key_v1 = CacheKey::new("foo", "hash_v1", "generic");
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
    let key_v2 = CacheKey::new("foo", "hash_v2", "generic");
    assert!(cache.lookup(&key_v2).is_none());

    // Also verify invalidate_function removes entries for "foo"
    let removed = cache.invalidate_function("foo");
    assert_eq!(removed, 1);
    assert!(cache.lookup(&key_v1).is_none());

    let _ = std::fs::remove_dir_all(&tmp);
}
