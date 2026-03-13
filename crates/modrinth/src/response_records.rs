use serde::Deserialize;

use crate::project::build_project_url;
use crate::{Project, ProjectDependency, ProjectVersion, ProjectVersionFile, SearchProject};

#[derive(Debug, Deserialize)]
pub(crate) struct SearchResponse {
    #[serde(default)]
    pub(crate) hits: Vec<SearchHit>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchHit {
    pub(crate) project_id: String,
    pub(crate) slug: Option<String>,
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) description: String,
    pub(crate) project_type: String,
    pub(crate) icon_url: Option<String>,
    pub(crate) author: Option<String>,
    #[serde(default)]
    pub(crate) downloads: u64,
    pub(crate) date_modified: Option<String>,
}

impl SearchHit {
    pub(crate) fn into_search_project(self) -> SearchProject {
        let project_url = build_project_url(
            self.project_type.as_str(),
            self.slug.as_deref(),
            self.project_id.as_str(),
        );

        SearchProject {
            project_id: self.project_id,
            slug: self.slug,
            title: self.title,
            description: self.description,
            project_type: self.project_type,
            icon_url: self.icon_url,
            author: self.author,
            project_url,
            downloads: self.downloads,
            date_modified: self.date_modified,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProjectRecord {
    pub(crate) id: String,
    pub(crate) slug: Option<String>,
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) description: String,
    pub(crate) project_type: String,
    pub(crate) icon_url: Option<String>,
}

impl ProjectRecord {
    pub(crate) fn into_project(self) -> Project {
        let project_url = build_project_url(
            self.project_type.as_str(),
            self.slug.as_deref(),
            self.id.as_str(),
        );

        Project {
            project_id: self.id,
            slug: self.slug,
            title: self.title,
            description: self.description,
            project_type: self.project_type,
            icon_url: self.icon_url,
            project_url,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProjectVersionRecord {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) project_id: String,
    #[serde(default)]
    pub(crate) version_number: String,
    #[serde(default)]
    pub(crate) date_published: String,
    #[serde(default)]
    pub(crate) downloads: u64,
    #[serde(default)]
    pub(crate) loaders: Vec<String>,
    #[serde(default)]
    pub(crate) game_versions: Vec<String>,
    #[serde(default)]
    pub(crate) dependencies: Vec<ProjectDependencyRecord>,
    #[serde(default)]
    pub(crate) files: Vec<ProjectVersionFileRecord>,
}

impl ProjectVersionRecord {
    pub(crate) fn into_project_version(self) -> ProjectVersion {
        ProjectVersion {
            id: self.id,
            project_id: self.project_id,
            version_number: self.version_number,
            date_published: self.date_published,
            downloads: self.downloads,
            loaders: self.loaders,
            game_versions: self.game_versions,
            dependencies: self
                .dependencies
                .into_iter()
                .map(ProjectDependencyRecord::into_project_dependency)
                .collect(),
            files: self
                .files
                .into_iter()
                .map(|file| ProjectVersionFile {
                    url: file.url,
                    filename: file.filename,
                    primary: file.primary,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProjectDependencyRecord {
    pub(crate) project_id: Option<String>,
    pub(crate) version_id: Option<String>,
    #[serde(default)]
    pub(crate) dependency_type: String,
    pub(crate) file_name: Option<String>,
}

impl ProjectDependencyRecord {
    pub(crate) fn into_project_dependency(self) -> ProjectDependency {
        ProjectDependency {
            project_id: self.project_id,
            version_id: self.version_id,
            dependency_type: self.dependency_type,
            file_name: self.file_name,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProjectVersionFileRecord {
    pub(crate) url: String,
    pub(crate) filename: String,
    #[serde(default)]
    pub(crate) primary: bool,
}
