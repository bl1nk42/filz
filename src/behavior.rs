use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BehaviorProfile {
    pub preferred_types: HashMap<String, usize>,
    pub preferred_stacks: HashMap<String, usize>,
}

impl BehaviorProfile {
    pub fn load(path: &Path) -> Self {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(profile) = serde_json::from_slice(&bytes) {
                return profile;
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

    pub fn observe(&mut self, r#type: &str, stack: &str) {
        *self
            .preferred_types
            .entry(r#type.to_lowercase())
            .or_default() += 1;
        *self
            .preferred_stacks
            .entry(stack.to_lowercase())
            .or_default() += 1;
    }

    pub fn suggest_type(&self) -> Option<String> {
        self.preferred_types
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(name, _)| name.clone())
    }

    pub fn suggest_stack(&self) -> Option<String> {
        self.preferred_stacks
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(name, _)| name.clone())
    }

    pub fn path(root: &Path) -> PathBuf {
        root.join(".filz-cache").join("behavior.json")
    }
}
