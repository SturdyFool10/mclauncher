use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{InstalledContentHashCacheUpdate, ResolvedInstalledContent};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstalledContentHashCache {
    #[serde(default = "default_hash_cache_version")]
    pub version: u32,
    #[serde(default)]
    pub entries: HashMap<String, Option<ResolvedInstalledContent>>,
}

impl Default for InstalledContentHashCache {
    fn default() -> Self {
        Self {
            version: default_hash_cache_version(),
            entries: HashMap::new(),
        }
    }
}

impl InstalledContentHashCache {
    pub fn apply_updates(
        &mut self,
        updates: impl IntoIterator<Item = InstalledContentHashCacheUpdate>,
    ) -> bool {
        let mut changed = false;
        for update in updates {
            let previous = self
                .entries
                .insert(update.hash_key, update.resolution.clone());
            if previous != Some(update.resolution) {
                changed = true;
            }
        }
        changed
    }
}

fn default_hash_cache_version() -> u32 {
    1
}
