use content_resolver::{InstalledContentHashCacheUpdate, ResolvedInstalledContent};

#[derive(Clone, Debug)]
pub(super) struct ContentLookupResult {
    pub(super) lookup_key: String,
    pub(super) resolution: Option<ResolvedInstalledContent>,
    pub(super) hash_cache_updates: Vec<InstalledContentHashCacheUpdate>,
}
