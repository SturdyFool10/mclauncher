use crate::{InstalledContentHashCacheUpdate, ResolvedInstalledContent};

#[derive(Clone, Debug)]
pub struct ResolveInstalledContentResult {
    pub resolution: Option<ResolvedInstalledContent>,
    pub hash_cache_updates: Vec<InstalledContentHashCacheUpdate>,
}
