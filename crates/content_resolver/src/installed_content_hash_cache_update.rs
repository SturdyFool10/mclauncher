use crate::ResolvedInstalledContent;

#[derive(Clone, Debug)]
pub struct InstalledContentHashCacheUpdate {
    pub hash_key: String,
    pub resolution: Option<ResolvedInstalledContent>,
}
