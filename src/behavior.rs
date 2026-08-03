use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BehaviorProfile {
    pub preferred_types: HashMap<String, usize>,
    pub preferred_stacks: HashMap<String, usize>,
}

impl BehaviorProfile {
    pub fn observe(&mut self, r#type: &str, stack: &str) {
        *self.preferred_types.entry(r#type.to_lowercase()).or_default() += 1;
        *self.preferred_stacks.entry(stack.to_lowercase()).or_default() += 1;
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
}
