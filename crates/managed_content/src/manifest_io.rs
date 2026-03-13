use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{CONTENT_MANIFEST_FILE_NAME, ContentInstallManifest, InstalledContentIdentity};

#[must_use]
pub fn content_manifest_path(instance_root: &Path) -> PathBuf {
    instance_root.join(CONTENT_MANIFEST_FILE_NAME)
}

#[must_use]
pub fn load_content_manifest(instance_root: &Path) -> ContentInstallManifest {
    let path = content_manifest_path(instance_root);
    let mut manifest = std::fs::read_to_string(path.as_path())
        .ok()
        .and_then(|raw| toml::from_str::<ContentInstallManifest>(&raw).ok())
        .unwrap_or_default();
    normalize_content_manifest(instance_root, &mut manifest);
    manifest
}

pub fn save_content_manifest(
    instance_root: &Path,
    manifest: &ContentInstallManifest,
) -> Result<(), String> {
    let mut normalized = manifest.clone();
    normalize_content_manifest(instance_root, &mut normalized);
    let path = content_manifest_path(instance_root);
    if normalized.projects.is_empty() {
        if path.exists() {
            let _ = std::fs::remove_file(path.as_path());
        }
        return Ok(());
    }
    let raw = toml::to_string_pretty(&normalized)
        .map_err(|err| format!("failed to serialize content manifest: {err}"))?;
    std::fs::write(path.as_path(), raw)
        .map_err(|err| format!("failed to write content manifest {}: {err}", path.display()))
}

#[must_use]
pub fn load_managed_content_identities(
    instance_root: &Path,
) -> HashMap<String, InstalledContentIdentity> {
    let manifest = load_content_manifest(instance_root);
    manifest
        .projects
        .into_values()
        .filter_map(|project| {
            let source = project.selected_source?;
            Some((
                normalize_content_path_key(project.file_path.as_str()),
                InstalledContentIdentity {
                    name: project.name,
                    file_path: project.file_path,
                    source: source.into(),
                    modrinth_project_id: project.modrinth_project_id,
                    curseforge_project_id: project.curseforge_project_id,
                    selected_version_id: project.selected_version_id.unwrap_or_default(),
                },
            ))
        })
        .collect()
}

pub fn normalize_content_manifest(instance_root: &Path, manifest: &mut ContentInstallManifest) {
    let missing_keys: Vec<String> = manifest
        .projects
        .iter()
        .filter_map(|(key, value)| {
            let file_path = instance_root.join(value.file_path.as_str());
            if file_path.exists() {
                None
            } else {
                Some(key.clone())
            }
        })
        .collect();
    for key in missing_keys {
        manifest.projects.remove(key.as_str());
    }

    let project_keys: std::collections::HashSet<String> =
        manifest.projects.keys().cloned().collect();
    for (key, value) in &mut manifest.projects {
        value.project_key = key.clone();
        value.file_path = normalize_content_path_key(value.file_path.as_str());
        value
            .direct_dependencies
            .retain(|dependency| dependency != key && project_keys.contains(dependency));
        value.direct_dependencies.sort();
        value.direct_dependencies.dedup();
        if value.selected_version_id.is_none() {
            value.selected_version_id = Some(String::new());
        }
        if value.selected_version_name.is_none() {
            value.selected_version_name = Some(String::new());
        }
    }
}

#[must_use]
pub fn normalize_content_path_key(value: &str) -> String {
    value
        .trim()
        .trim_start_matches("./")
        .trim_start_matches(".\\")
        .replace('\\', "/")
        .to_ascii_lowercase()
}
