use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VtmpackDownloadableEntry {
    #[serde(default)]
    pub project_key: String,
    #[serde(default)]
    pub name: String,
    pub file_path: String,
    #[serde(default)]
    pub modrinth_project_id: Option<String>,
    #[serde(default)]
    pub curseforge_project_id: Option<u64>,
    #[serde(default)]
    pub selected_source: Option<String>,
    #[serde(default)]
    pub selected_version_id: Option<String>,
    #[serde(default)]
    pub selected_version_name: Option<String>,
}
