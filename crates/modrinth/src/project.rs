/// Detailed project metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub project_id: String,
    pub slug: Option<String>,
    pub title: String,
    pub description: String,
    pub project_type: String,
    pub icon_url: Option<String>,
    pub project_url: String,
}

pub(crate) fn build_project_url(
    project_type: &str,
    slug: Option<&str>,
    fallback_id: &str,
) -> String {
    let canonical_slug = slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_id);
    let canonical_type = project_type.trim();
    format!("https://modrinth.com/{canonical_type}/{canonical_slug}")
}
