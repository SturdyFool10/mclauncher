use crate::{ProjectDependency, ProjectVersionFile};

/// A compatible Modrinth version entry for a project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectVersion {
    pub id: String,
    pub project_id: String,
    pub version_number: String,
    pub date_published: String,
    pub downloads: u64,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub dependencies: Vec<ProjectDependency>,
    pub files: Vec<ProjectVersionFile>,
}
