use std::path::PathBuf;

use managed_content::InstalledContentIdentity;

#[derive(Clone, Debug)]
pub struct InstalledContentFile {
    pub file_name: String,
    pub file_path: PathBuf,
    pub lookup_query: String,
    pub lookup_key: String,
    pub fallback_lookup_query: Option<String>,
    pub fallback_lookup_key: Option<String>,
    pub managed_identity: Option<InstalledContentIdentity>,
}
