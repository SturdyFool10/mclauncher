use modprovider::UnifiedContentEntry;
use serde::{Deserialize, Serialize};

use crate::{InstalledContentResolutionKind, InstalledContentUpdate};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedInstalledContent {
    pub entry: UnifiedContentEntry,
    #[serde(default)]
    pub installed_version_id: Option<String>,
    #[serde(default)]
    pub installed_version_label: Option<String>,
    pub resolution_kind: InstalledContentResolutionKind,
    #[serde(default)]
    pub warning_message: Option<String>,
    #[serde(skip, default)]
    pub update: Option<InstalledContentUpdate>,
}
