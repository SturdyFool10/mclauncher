/// Dependency edge declared by a Modrinth project version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectDependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: String,
    pub file_name: Option<String>,
}
