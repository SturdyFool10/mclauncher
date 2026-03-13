use serde::{Deserialize, Serialize};

use crate::{VtmpackDownloadableEntry, VtmpackInstanceMetadata};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VtmpackManifest {
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub exported_at_ms: u64,
    pub instance: VtmpackInstanceMetadata,
    #[serde(default)]
    pub downloadable_content: Vec<VtmpackDownloadableEntry>,
    #[serde(default)]
    pub bundled_mods: Vec<String>,
    #[serde(default)]
    pub configs: Vec<String>,
    #[serde(default)]
    pub additional_paths: Vec<String>,
}
