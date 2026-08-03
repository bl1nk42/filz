use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SearchCache {
    pub queries: HashMap<String, Vec<String>>,
}

impl SearchCache {
    pub fn load(path: &Path) -> Self {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(cache) = serde_json::from_slice(&bytes) {
                return cache;
            }
        }
        Self::default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let bytes = serde_json::to_vec_pretty(self).unwrap_or_default();
        fs::write(path, bytes)
    }

    pub fn cached_results(&self, query: &str) -> Option<Vec<String>> {
        self.queries.get(query).cloned()
    }

    pub fn remember(&mut self, query: &str, results: &[String]) {
        self.queries.insert(query.to_lowercase(), results.to_vec());
    }

    pub fn cache_path(root: &Path) -> PathBuf {
        root.join(".filz-cache").join("search-cache.json")
    }
}
