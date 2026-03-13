use std::path::PathBuf;

use managed_content::InstalledContentIdentity;

use crate::InstalledContentKind;

#[derive(Clone, Debug)]
pub struct ResolveInstalledContentRequest {
    pub file_path: PathBuf,
    pub disk_file_name: String,
    pub lookup_query: String,
    pub fallback_lookup_key: Option<String>,
    pub fallback_lookup_query: Option<String>,
    pub managed_identity: Option<InstalledContentIdentity>,
    pub kind: InstalledContentKind,
    pub game_version: String,
    pub loader: String,
}
