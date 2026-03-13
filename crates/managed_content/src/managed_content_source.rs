use modprovider::ContentSource;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManagedContentSource {
    Modrinth,
    CurseForge,
}

impl ManagedContentSource {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            ManagedContentSource::Modrinth => "Modrinth",
            ManagedContentSource::CurseForge => "CurseForge",
        }
    }
}

impl From<ContentSource> for ManagedContentSource {
    fn from(value: ContentSource) -> Self {
        match value {
            ContentSource::Modrinth => ManagedContentSource::Modrinth,
            ContentSource::CurseForge => ManagedContentSource::CurseForge,
        }
    }
}

impl From<ManagedContentSource> for ContentSource {
    fn from(value: ManagedContentSource) -> Self {
        match value {
            ManagedContentSource::Modrinth => ContentSource::Modrinth,
            ManagedContentSource::CurseForge => ContentSource::CurseForge,
        }
    }
}
