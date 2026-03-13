/// A normalized search entry returned from Modrinth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchProject {
    pub project_id: String,
    pub slug: Option<String>,
    pub title: String,
    pub description: String,
    pub project_type: String,
    pub icon_url: Option<String>,
    pub author: Option<String>,
    pub project_url: String,
    pub downloads: u64,
    pub date_modified: Option<String>,
}
