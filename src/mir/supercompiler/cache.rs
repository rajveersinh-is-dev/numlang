use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::mir::lower::MirFunction;

pub fn sha256_str(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Inter-function dependency graph tracking call relationships.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DependencyGraph {
    /// Maps caller function -> list of direct callee functions called by caller.
    pub callers_to_callees: HashMap<String, Vec<String>>,
    /// Maps callee function -> list of caller functions that call/depend on callee.
    pub callees_to_callers: HashMap<String, Vec<String>>,
}

fn default_compiler_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Key uniquely identifying a specialization.
/// Hashed to produce the cache filename.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CacheKey {
    /// Name of the function being specialized.
    pub function_name: String,
    /// SHA-256 hex string of the function MIR body and reachable callee bodies (composite fingerprint).
    pub function_source_hash: String,
    /// SHA-256 hex string of the symbolic argument fingerprint at the call site.
    /// Use the Debug repr of the initial SymbolicState env as the fingerprint input.
    pub argument_fingerprint: String,
    /// Compiler version string (prevents stale residuals across compiler upgrades).
    #[serde(default = "default_compiler_version")]
    pub compiler_version: String,
    /// Compiler optimization flags or mode (e.g. "opt_level=3;backend=cranelift;mode=classic").
    #[serde(default)]
    pub optimization_flags: String,
}

impl Default for CacheKey {
    fn default() -> Self {
        Self {
            function_name: String::new(),
            function_source_hash: String::new(),
            argument_fingerprint: String::new(),
            compiler_version: default_compiler_version(),
            optimization_flags: String::new(),
        }
    }
}

impl CacheKey {
    pub fn new(
        function_name: impl Into<String>,
        function_source_hash: impl Into<String>,
        argument_fingerprint: impl Into<String>,
    ) -> Self {
        Self {
            function_name: function_name.into(),
            function_source_hash: function_source_hash.into(),
            argument_fingerprint: argument_fingerprint.into(),
            compiler_version: default_compiler_version(),
            optimization_flags: String::new(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<String>) -> Self {
        self.optimization_flags = flags.into();
        self
    }
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

/// Hit/miss telemetry for two-level specialization caching.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CacheMetrics {
    pub l1_hits: usize,
    pub l2_hits: usize,
    pub misses: usize,
    pub stores: usize,
}

/// Production-grade two-level content-addressed SHA-256 specialization cache:
/// - L1: In-memory hashmap for sub-microsecond lookups.
/// - L2: Persistent directory-sharded disk cache (`<hex[0..2]>/<hex[2..]>.json`).
/// - Dependency Graph: Inter-function caller/callee DAG for fine-grained invalidation.
pub struct SpecializationCache {
    cache_dir: PathBuf,
    l1_cache: RwLock<HashMap<CacheKey, CachedSpecialization>>,
    metrics: RwLock<CacheMetrics>,
    dep_graph: RwLock<DependencyGraph>,
}

impl SpecializationCache {
    /// Open or create the cache at `cache_dir`. Creates the directory if needed.
    pub fn open(cache_dir: &Path) -> Self {
        let _ = fs::create_dir_all(cache_dir);
        let cache = Self {
            cache_dir: cache_dir.to_path_buf(),
            l1_cache: RwLock::new(HashMap::new()),
            metrics: RwLock::new(CacheMetrics::default()),
            dep_graph: RwLock::new(DependencyGraph::default()),
        };
        let _ = cache.load_dependency_graph();
        cache
    }

    /// Return reference to cache directory path.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Return the cache file path for a given key (SHA-256 of key JSON → hex filename).
    pub fn key_to_path(&self, key: &CacheKey) -> PathBuf {
        let key_json = serde_json::to_string(key).unwrap_or_else(|_| {
            format!(
                "{}:{}:{}:{}:{}",
                key.function_name,
                key.function_source_hash,
                key.argument_fingerprint,
                key.compiler_version,
                key.optimization_flags,
            )
        });
        let hex = sha256_str(&key_json);
        let prefix = &hex[0..2];
        self.cache_dir.join(prefix).join(format!("{}.json", hex))
    }

    /// Return current cache performance telemetry.
    pub fn metrics(&self) -> CacheMetrics {
        self.metrics.read().map(|m| m.clone()).unwrap_or_default()
    }

    /// Records direct callee dependencies for a caller function and updates both forward and reverse maps.
    pub fn record_dependencies(&self, caller: &str, callees: HashSet<String>) {
        if let Ok(mut graph) = self.dep_graph.write() {
            let mut callees_vec: Vec<String> = callees.into_iter().collect();
            callees_vec.sort();

            // Update callees_to_callers (remove old dependencies for caller first)
            let old_callees_opt = graph.callers_to_callees.get(caller).cloned();
            if let Some(old_callees) = old_callees_opt {
                for old_c in &old_callees {
                    if let Some(callers) = graph.callees_to_callers.get_mut(old_c) {
                        callers.retain(|c| c != caller);
                    }
                }
            }

            for callee in &callees_vec {
                let callers = graph.callees_to_callers.entry(callee.clone()).or_default();
                if !callers.contains(&caller.to_string()) {
                    callers.push(caller.to_string());
                    callers.sort();
                }
            }

            graph
                .callers_to_callees
                .insert(caller.to_string(), callees_vec);
        }
    }

    /// Returns a copy of the current dependency graph snapshot.
    pub fn dependency_graph(&self) -> DependencyGraph {
        self.dep_graph.read().map(|g| g.clone()).unwrap_or_default()
    }

    /// Invalidate a function and all of its transitive callers across the dependency DAG.
    /// Returns the list of invalidated function names.
    pub fn invalidate_transitive(&self, changed_func: &str) -> Vec<String> {
        let mut invalidated = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(changed_func.to_string());
        visited.insert(changed_func.to_string());

        let dep_graph = self.dependency_graph();

        while let Some(current) = queue.pop_front() {
            self.invalidate_function(&current);
            invalidated.push(current.clone());

            if let Some(callers) = dep_graph.callees_to_callers.get(&current) {
                for caller in callers {
                    if visited.insert(caller.clone()) {
                        queue.push_back(caller.clone());
                    }
                }
            }
        }

        invalidated
    }

    /// Persist the dependency graph to `<cache_dir>/deps.json`.
    pub fn save_dependency_graph(&self) -> std::io::Result<()> {
        let path = self.cache_dir.join("deps.json");
        if let Ok(graph) = self.dep_graph.read() {
            let json = serde_json::to_string_pretty(&*graph).map_err(std::io::Error::other)?;
            fs::write(path, json)?;
        }
        Ok(())
    }

    /// Load the dependency graph from `<cache_dir>/deps.json`.
    pub fn load_dependency_graph(&self) -> std::io::Result<()> {
        let path = self.cache_dir.join("deps.json");
        if path.exists() {
            let content = fs::read_to_string(path)?;
            if let Ok(graph) = serde_json::from_str::<DependencyGraph>(&content) {
                if let Ok(mut g) = self.dep_graph.write() {
                    *g = graph;
                }
            }
        }
        Ok(())
    }

    /// Look up a cached specialization. First checks L1 in-memory cache (< 1µs),
    /// then checks L2 disk cache (< 1ms). Returns None on miss or hash mismatch.
    pub fn lookup(&self, key: &CacheKey) -> Option<CachedSpecialization> {
        // 1. Check L1 in-memory cache
        if let Ok(l1) = self.l1_cache.read() {
            if let Some(entry) = l1.get(key) {
                if let Ok(mut m) = self.metrics.write() {
                    m.l1_hits += 1;
                }
                return Some(entry.clone());
            }
        }

        // 2. Check L2 disk cache
        let path = self.key_to_path(key);
        if !path.exists() {
            if let Ok(mut m) = self.metrics.write() {
                m.misses += 1;
            }
            return None;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => {
                if let Ok(mut m) = self.metrics.write() {
                    m.misses += 1;
                }
                return None;
            }
        };

        let entry: CachedSpecialization = match serde_json::from_str(&content) {
            Ok(e) => e,
            Err(_) => {
                // Gracefully treat corrupted cache JSON as a cache miss
                if let Ok(mut m) = self.metrics.write() {
                    m.misses += 1;
                }
                return None;
            }
        };

        if entry.key == *key {
            // Populate L1 cache for subsequent lookups
            if let Ok(mut l1) = self.l1_cache.write() {
                l1.insert(key.clone(), entry.clone());
            }
            if let Ok(mut m) = self.metrics.write() {
                m.l2_hits += 1;
            }
            Some(entry)
        } else {
            if let Ok(mut m) = self.metrics.write() {
                m.misses += 1;
            }
            None
        }
    }

    /// Store a specialization in both L1 memory and L2 persistent disk cache.
    pub fn store(&self, entry: &CachedSpecialization) -> std::io::Result<()> {
        let path = self.key_to_path(&entry.key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(entry).map_err(std::io::Error::other)?;
        fs::write(&path, json)?;

        // Populate L1 cache
        if let Ok(mut l1) = self.l1_cache.write() {
            l1.insert(entry.key.clone(), entry.clone());
        }
        if let Ok(mut m) = self.metrics.write() {
            m.stores += 1;
        }

        Ok(())
    }

    /// Invalidate all cache entries for a given function name
    /// in both L1 in-memory cache and L2 disk cache.
    pub fn invalidate_function(&self, function_name: &str) -> usize {
        let mut count = 0;

        // Remove from L1
        if let Ok(mut l1) = self.l1_cache.write() {
            l1.retain(|k, _| k.function_name != function_name);
        }

        // Remove from L2 disk
        if self.cache_dir.exists() {
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
        }

        count
    }

    /// Clear all cached entries in memory and on disk.
    pub fn clear(&self) -> std::io::Result<()> {
        if let Ok(mut l1) = self.l1_cache.write() {
            l1.clear();
        }
        if let Ok(mut g) = self.dep_graph.write() {
            *g = DependencyGraph::default();
        }
        if self.cache_dir.exists() {
            let _ = fs::remove_dir_all(&self.cache_dir);
            let _ = fs::create_dir_all(&self.cache_dir);
        }
        Ok(())
    }
}

/// Extracts all direct function callees referenced in a MirFunction body.
pub fn extract_callees(func: &MirFunction) -> HashSet<String> {
    let mut callees = HashSet::new();
    for block in &func.blocks {
        for stmt in &block.statements {
            let crate::mir::lower::Statement::Assign(_, rval) = stmt;
            match rval {
                crate::mir::lower::Rvalue::Call(name, _) => {
                    callees.insert(name.clone());
                }
                crate::mir::lower::Rvalue::FnPtr(name) => {
                    callees.insert(name.clone());
                }
                crate::mir::lower::Rvalue::ClosureAlloc { fn_name, .. } => {
                    callees.insert(fn_name.clone());
                }
                crate::mir::lower::Rvalue::Thunk { body, .. } => {
                    callees.insert(body.clone());
                }
                _ => {}
            }
        }
    }
    callees
}

/// Computes a composite SHA-256 fingerprint for a function, incorporating its own
/// MIR body representation and the MIR body representations of all reachable callees in the call graph.
pub fn compute_composite_hash(
    func_name: &str,
    funcs_by_name: &HashMap<String, &MirFunction>,
) -> String {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(func_name.to_string());
    visited.insert(func_name.to_string());

    let mut reachable = Vec::new();

    while let Some(current) = queue.pop_front() {
        if let Some(f) = funcs_by_name.get(&current) {
            reachable.push((current.clone(), format!("{:?}", f)));
            let callees = extract_callees(f);
            for callee in callees {
                if visited.insert(callee.clone()) {
                    queue.push_back(callee);
                }
            }
        }
    }

    // Sort by name for deterministic hashing
    reachable.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (name, repr) in &reachable {
        hasher.update(name.as_bytes());
        hasher.update(b":");
        hasher.update(repr.as_bytes());
        hasher.update(b";");
    }
    format!("{:x}", hasher.finalize())
}

fn find_json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                find_json_files(&p, out);
            } else if p.extension().is_some_and(|ext| ext == "json")
                && p.file_name().and_then(|n| n.to_str()) != Some("deps.json")
            {
                out.push(p);
            }
        }
    }
}
