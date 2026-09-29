use std::fs;
use std::path::{Path, PathBuf};
use sha2::{Digest, Sha256};

pub fn sha256_str(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Key uniquely identifying a specialization.
/// Hashed to produce the cache filename.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CacheKey {
    /// Name of the function being specialized.
    pub function_name: String,
    /// SHA-256 hex string of the MirFunction's Debug representation (stable fingerprint).
    pub function_source_hash: String,
    /// SHA-256 hex string of the symbolic argument fingerprint at the call site.
    /// Use the Debug repr of the initial SymbolicState env as the fingerprint input.
    pub argument_fingerprint: String,
}

/// A cached specialization result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedSpecialization {
    pub key: CacheKey,
    /// The residualized MirFunction, serialized to JSON via serde.
    pub residual_json: String,
    /// SupercompilerStats from the original run.
    pub stats_nodes_explored: usize,
    pub stats_branches_pruned: usize,
    pub stats_loops_collapsed: usize,
    pub stats_knots_tied: usize,
    pub stats_calls_inlined: usize,
    pub stats_sc_bce_eliminated: usize,
    pub stats_residual_block_count: usize,
    pub stats_residual_stmt_count: usize,
}

pub struct SpecializationCache {
    cache_dir: PathBuf,
}

impl SpecializationCache {
    /// Open or create the cache at `cache_dir`. Creates the directory if needed.
    pub fn open(cache_dir: &Path) -> Self {
        let _ = fs::create_dir_all(cache_dir);
        Self {
            cache_dir: cache_dir.to_path_buf(),
        }
    }

    /// Return the cache file path for a given key (SHA-256 of key JSON → hex filename).
    pub fn key_to_path(&self, key: &CacheKey) -> PathBuf {
        let key_json = serde_json::to_string(key).unwrap_or_else(|_| {
            format!("{}:{}:{}", key.function_name, key.function_source_hash, key.argument_fingerprint)
        });
        let hex = sha256_str(&key_json);
        let prefix = &hex[0..2];
        self.cache_dir.join(prefix).join(format!("{}.json", hex))
    }

    /// Look up a cached specialization. Returns None on miss or hash mismatch.
    pub fn lookup(&self, key: &CacheKey) -> Option<CachedSpecialization> {
        let path = self.key_to_path(key);
        if !path.exists() {
            return None;
        }
        let content = fs::read_to_string(&path).ok()?;
        let entry: CachedSpecialization = serde_json::from_str(&content).ok()?;
        if entry.key == *key {
            Some(entry)
        } else {
            None
        }
    }

    /// Store a specialization in the cache.
    pub fn store(&self, entry: &CachedSpecialization) -> std::io::Result<()> {
        let path = self.key_to_path(&entry.key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(entry)
            .map_err(std::io::Error::other)?;
        fs::write(&path, json)
    }

    /// Invalidate all cache entries for a given function name
    /// (called when the source file changes).
    pub fn invalidate_function(&self, function_name: &str) -> usize {
        let mut count = 0;
        if !self.cache_dir.exists() {
            return 0;
        }
        let mut files = Vec::new();
        find_json_files(&self.cache_dir, &mut files);
        for file in files {
            if let Ok(content) = fs::read_to_string(&file) {
                if let Ok(cached) = serde_json::from_str::<CachedSpecialization>(&content) {
                    if cached.key.function_name == function_name
                        && fs::remove_file(&file).is_ok()
                    {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

fn find_json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                find_json_files(&p, out);
            } else if p.extension().is_some_and(|ext| ext == "json") {
                out.push(p);
            }
        }
    }
}
