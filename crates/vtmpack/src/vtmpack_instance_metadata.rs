use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VtmpackInstanceMetadata {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub game_version: String,
    pub modloader: String,
    #[serde(default)]
    pub modloader_version: String,
}
