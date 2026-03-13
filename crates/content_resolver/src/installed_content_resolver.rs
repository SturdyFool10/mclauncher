use std::collections::HashMap;
use std::path::{Path, PathBuf};

use curseforge::Client as CurseForgeClient;
use managed_content::InstalledContentIdentity;
use modprovider::{ContentSource, UnifiedContentEntry, search_minecraft_content};
use modrinth::Client as ModrinthClient;

use crate::{
    InstalledContentFile, InstalledContentHashCache, InstalledContentHashCacheUpdate,
    InstalledContentKind, InstalledContentResolutionKind, InstalledContentUpdate,
    ResolveInstalledContentRequest, ResolveInstalledContentResult, ResolvedInstalledContent,
};

const CONTENT_HASH_CACHE_DIR_NAME: &str = "cache";
const CONTENT_HASH_CACHE_FILE_NAME: &str = "content_hash_cache.json";
const CURSEFORGE_VERSION_LOOKUP_PAGE_SIZE: u32 = 50;
const CURSEFORGE_VERSION_LOOKUP_MAX_PAGES: u32 = 5;
const HEURISTIC_WARNING_MESSAGE: &str =
    "Resolved from filename search. This match is heuristic and may be wrong.";

pub struct InstalledContentResolver;

impl InstalledContentResolver {
    pub fn scan_installed_content_files(
        instance_root: &Path,
        kind: InstalledContentKind,
        managed_identities: &HashMap<String, InstalledContentIdentity>,
    ) -> Vec<InstalledContentFile> {
        let dir = instance_root.join(kind.folder_name());
        let mut files = Vec::new();
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return files;
        };

        for entry in read_dir.flatten() {
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with('.') {
                continue;
            }

            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            let allowed = match kind {
                InstalledContentKind::Mods => file_type.is_file() && extension == "jar",
                InstalledContentKind::ResourcePacks
                | InstalledContentKind::ShaderPacks
                | InstalledContentKind::DataPacks => file_type.is_dir() || extension == "zip",
            };
            if !allowed {
                continue;
            }

            let relative_path_key = normalize_installed_content_path_key(
                path.strip_prefix(instance_root)
                    .unwrap_or(path.as_path())
                    .to_string_lossy()
                    .as_ref(),
            );
            let managed_identity = managed_identities.get(relative_path_key.as_str()).cloned();
            let lookup_query = managed_identity
                .as_ref()
                .map(|identity| identity.name.clone())
                .unwrap_or_else(|| {
                    derive_installed_lookup_query(path.as_path(), file_name.as_str())
                });
            let (fallback_lookup_query, fallback_lookup_key) = if managed_identity.is_some() {
                (None, None)
            } else {
                let fallback_query = derive_raw_lookup_query(path.as_path(), file_name.as_str());
                let fallback_key_suffix = normalize_lookup_key(fallback_query.as_str());
                if fallback_key_suffix.is_empty()
                    || fallback_key_suffix == normalize_lookup_key(lookup_query.as_str())
                {
                    (None, None)
                } else {
                    (
                        Some(fallback_query),
                        Some(format!("{}::{fallback_key_suffix}", kind.folder_name())),
                    )
                }
            };
            let lookup_key = format!(
                "{}::{}",
                kind.folder_name(),
                managed_lookup_key_suffix(managed_identity.as_ref(), lookup_query.as_str())
            );
            files.push(InstalledContentFile {
                file_name,
                file_path: path,
                lookup_query,
                lookup_key,
                fallback_lookup_query,
                fallback_lookup_key,
                managed_identity,
            });
        }

        files.sort_by(|left, right| {
            left.file_name
                .to_ascii_lowercase()
                .cmp(&right.file_name.to_ascii_lowercase())
        });
        files
    }

    pub fn load_hash_cache(instance_root: &Path) -> InstalledContentHashCache {
        let cache_path = content_hash_cache_path(instance_root);
        let Ok(raw) = std::fs::read_to_string(cache_path.as_path()) else {
            return InstalledContentHashCache::default();
        };
        let Ok(cache) = serde_json::from_str::<InstalledContentHashCache>(raw.as_str()) else {
            return InstalledContentHashCache::default();
        };
        if cache.version == InstalledContentHashCache::default().version {
            cache
        } else {
            InstalledContentHashCache::default()
        }
    }

    pub fn save_hash_cache(
        instance_root: &Path,
        cache: &InstalledContentHashCache,
    ) -> Result<(), std::io::Error> {
        let cache_path = content_hash_cache_path(instance_root);
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(cache)
            .map_err(|err| std::io::Error::other(err.to_string()))?;
        std::fs::write(cache_path, raw)
    }

    pub fn clear_hash_cache(instance_root: &Path) -> Result<(), std::io::Error> {
        let cache_path = content_hash_cache_path(instance_root);
        if cache_path.exists() {
            std::fs::remove_file(cache_path)?;
        }
        Ok(())
    }

    pub fn resolve(
        request: &ResolveInstalledContentRequest,
        hash_cache: &InstalledContentHashCache,
    ) -> ResolveInstalledContentResult {
        let mut hash_cache_updates = Vec::new();

        let exact_hash_resolution = if request.kind == InstalledContentKind::Mods
            && is_jar_file(request.file_path.as_path())
        {
            let (resolution, updates) = resolve_modrinth_hash_metadata(
                request.file_path.as_path(),
                request.game_version.as_str(),
                request.loader.as_str(),
                hash_cache,
            );
            hash_cache_updates = updates;
            resolution
        } else {
            None
        };

        let resolution = exact_hash_resolution
            .or_else(|| {
                managed_content_metadata(
                    request.file_path.as_path(),
                    request.disk_file_name.as_str(),
                    request.managed_identity.as_ref(),
                    request.kind,
                )
            })
            .or_else(|| heuristic_content_metadata(request));

        ResolveInstalledContentResult {
            resolution,
            hash_cache_updates,
        }
    }
}

fn resolve_modrinth_hash_metadata(
    file_path: &Path,
    game_version: &str,
    loader: &str,
    hash_cache: &InstalledContentHashCache,
) -> (
    Option<ResolvedInstalledContent>,
    Vec<InstalledContentHashCacheUpdate>,
) {
    let Ok((sha1, sha512)) = modrinth::hash_file_sha1_and_sha512_hex(file_path) else {
        return (None, Vec::new());
    };

    let modrinth = ModrinthClient::default();
    let loaders = modrinth_loader_slugs(loader);
    let game_versions = normalized_game_versions(game_version);

    for (algorithm, hash) in [("sha512", sha512.as_str()), ("sha1", sha1.as_str())] {
        let hash_key = format!("{algorithm}:{hash}");
        if let Some(cached) = hash_cache.entries.get(hash_key.as_str()) {
            if let Some(mut cached_resolution) = cached.clone() {
                cached_resolution.update = resolve_modrinth_hash_update(
                    &modrinth,
                    hash,
                    algorithm,
                    loaders.as_slice(),
                    game_versions.as_slice(),
                    cached_resolution.installed_version_id.as_deref(),
                );
                return (Some(cached_resolution), Vec::new());
            }
            continue;
        }

        let Some(version) = modrinth
            .get_version_from_hash(hash, algorithm)
            .ok()
            .flatten()
        else {
            continue;
        };
        let Some(entry) = modrinth_entry_from_project_id(&modrinth, version.project_id.as_str())
        else {
            continue;
        };

        let resolution = ResolvedInstalledContent {
            entry,
            installed_version_id: non_empty_owned(version.id.as_str()),
            installed_version_label: non_empty_owned(version.version_number.as_str()),
            resolution_kind: InstalledContentResolutionKind::ExactHash,
            warning_message: None,
            update: resolve_modrinth_hash_update(
                &modrinth,
                hash,
                algorithm,
                loaders.as_slice(),
                game_versions.as_slice(),
                Some(version.id.as_str()),
            ),
        };
        let mut cached_resolution = resolution.clone();
        cached_resolution.update = None;
        let updates = vec![
            InstalledContentHashCacheUpdate {
                hash_key: format!("sha512:{sha512}"),
                resolution: Some(cached_resolution.clone()),
            },
            InstalledContentHashCacheUpdate {
                hash_key: format!("sha1:{sha1}"),
                resolution: Some(cached_resolution),
            },
        ];
        return (Some(resolution), updates);
    }

    (
        None,
        vec![
            InstalledContentHashCacheUpdate {
                hash_key: format!("sha512:{sha512}"),
                resolution: None,
            },
            InstalledContentHashCacheUpdate {
                hash_key: format!("sha1:{sha1}"),
                resolution: None,
            },
        ],
    )
}

fn resolve_modrinth_hash_update(
    modrinth: &ModrinthClient,
    hash: &str,
    algorithm: &str,
    loaders: &[String],
    game_versions: &[String],
    installed_version_id: Option<&str>,
) -> Option<InstalledContentUpdate> {
    let latest = modrinth
        .get_latest_version_from_hash(hash, algorithm, loaders, game_versions)
        .ok()
        .flatten()?;
    if installed_version_id.is_some_and(|value| value == latest.id) {
        return None;
    }

    Some(InstalledContentUpdate {
        latest_version_id: latest.id,
        latest_version_label: non_empty_owned(latest.version_number.as_str())
            .unwrap_or_else(|| "Unknown update".to_owned()),
    })
}

fn modrinth_entry_from_project_id(
    modrinth: &ModrinthClient,
    project_id: &str,
) -> Option<UnifiedContentEntry> {
    let project = modrinth.get_project(project_id).ok()?;
    Some(UnifiedContentEntry {
        id: format!("modrinth:{}", project.project_id),
        name: project.title,
        summary: project.description.trim().to_owned(),
        content_type: project.project_type,
        source: ContentSource::Modrinth,
        project_url: Some(project.project_url),
        icon_url: project.icon_url,
    })
}

fn managed_content_metadata(
    file_path: &Path,
    disk_file_name: &str,
    managed_identity: Option<&InstalledContentIdentity>,
    kind: InstalledContentKind,
) -> Option<ResolvedInstalledContent> {
    let identity = managed_identity?;

    match identity.source {
        ContentSource::Modrinth => {
            let project_id = identity.modrinth_project_id.as_deref()?;
            let version_id = identity.selected_version_id.trim();
            if version_id.is_empty()
                || !managed_identity_matches_file_name(identity, disk_file_name)
            {
                return None;
            }

            let modrinth = ModrinthClient::default();
            let version = modrinth.get_version(version_id).ok()?;
            if version.project_id != project_id
                || !version_contains_file_name(version.files.as_slice(), disk_file_name)
            {
                return None;
            }
            let entry = modrinth_entry_from_project_id(&modrinth, project_id)?;
            Some(ResolvedInstalledContent {
                entry,
                installed_version_id: Some(version.id),
                installed_version_label: non_empty_owned(version.version_number.as_str()),
                resolution_kind: InstalledContentResolutionKind::Managed,
                warning_message: None,
                update: None,
            })
        }
        ContentSource::CurseForge => {
            let project_id = identity.curseforge_project_id?;
            let version_id = identity.selected_version_id.trim().parse::<u64>().ok()?;
            let curseforge = CurseForgeClient::from_env()?;
            let file = find_curseforge_project_file(&curseforge, project_id, version_id)?;
            if file.file_name != disk_file_name
                || file_path.file_name()?.to_str()? != disk_file_name
            {
                return None;
            }

            let project = curseforge.get_mod(project_id).ok()?;
            Some(ResolvedInstalledContent {
                entry: UnifiedContentEntry {
                    id: format!("curseforge:{}", project.id),
                    name: project.name,
                    summary: project.summary.trim().to_owned(),
                    content_type: kind.content_type_key().to_owned(),
                    source: ContentSource::CurseForge,
                    project_url: project.website_url,
                    icon_url: project.icon_url,
                },
                installed_version_id: Some(file.id.to_string()),
                installed_version_label: non_empty_owned(file.display_name.as_str()),
                resolution_kind: InstalledContentResolutionKind::Managed,
                warning_message: None,
                update: None,
            })
        }
    }
}

fn heuristic_content_metadata(
    request: &ResolveInstalledContentRequest,
) -> Option<ResolvedInstalledContent> {
    let mut candidates = Vec::new();
    if !request.lookup_query.trim().is_empty() {
        candidates.push((request.lookup_query.as_str(), None::<&str>));
    }
    if let (Some(fallback_key), Some(fallback_query)) = (
        request.fallback_lookup_key.as_deref(),
        request.fallback_lookup_query.as_deref(),
    ) && !fallback_key.trim().is_empty()
        && !fallback_query.trim().is_empty()
    {
        candidates.push((fallback_query, Some(fallback_key)));
    }

    for (query, override_key) in candidates {
        let lookup_key = override_key.unwrap_or(request.lookup_query.as_str());
        let mut entries = search_modrinth_heuristic_content(
            query,
            request.kind,
            request.game_version.as_str(),
            request.loader.as_str(),
        );
        if entries.is_empty() {
            entries = search_minecraft_content(query, 10).ok()?.entries;
        }
        if let Some(entry) = choose_preferred_content_entry(entries, lookup_key, request.kind) {
            return Some(ResolvedInstalledContent {
                entry,
                installed_version_id: None,
                installed_version_label: None,
                resolution_kind: InstalledContentResolutionKind::HeuristicSearch,
                warning_message: Some(HEURISTIC_WARNING_MESSAGE.to_owned()),
                update: None,
            });
        }
    }

    None
}

fn search_modrinth_heuristic_content(
    query: &str,
    kind: InstalledContentKind,
    game_version: &str,
    loader: &str,
) -> Vec<UnifiedContentEntry> {
    let modrinth = ModrinthClient::default();
    let loader_filter = if kind == InstalledContentKind::Mods {
        modrinth_loader_slug(loader)
    } else {
        None
    };
    let game_version = normalize_optional(game_version);
    let Ok(entries) = modrinth.search_projects_with_filters(
        query,
        10,
        0,
        Some(kind.modrinth_project_type()),
        game_version.as_deref(),
        loader_filter,
    ) else {
        return Vec::new();
    };

    entries
        .into_iter()
        .map(|entry| UnifiedContentEntry {
            id: format!("modrinth:{}", entry.project_id),
            name: entry.title,
            summary: entry.description.trim().to_owned(),
            content_type: entry.project_type,
            source: ContentSource::Modrinth,
            project_url: Some(entry.project_url),
            icon_url: entry.icon_url,
        })
        .collect()
}

fn managed_identity_matches_file_name(
    identity: &InstalledContentIdentity,
    disk_file_name: &str,
) -> bool {
    let expected = Path::new(identity.file_path.as_str())
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    !expected.is_empty() && expected == disk_file_name
}

fn version_contains_file_name(
    files: &[modrinth::ProjectVersionFile],
    disk_file_name: &str,
) -> bool {
    files.iter().any(|file| file.filename == disk_file_name)
}

fn find_curseforge_project_file(
    client: &CurseForgeClient,
    project_id: u64,
    version_id: u64,
) -> Option<curseforge::File> {
    let mut index = 0u32;
    for _ in 0..CURSEFORGE_VERSION_LOOKUP_MAX_PAGES {
        let batch = client
            .list_mod_files(
                project_id,
                None,
                None,
                index,
                CURSEFORGE_VERSION_LOOKUP_PAGE_SIZE,
            )
            .ok()?;
        let batch_len = batch.len() as u32;
        if let Some(file) = batch.into_iter().find(|file| file.id == version_id) {
            return Some(file);
        }
        if batch_len < CURSEFORGE_VERSION_LOOKUP_PAGE_SIZE {
            break;
        }
        index = index.saturating_add(CURSEFORGE_VERSION_LOOKUP_PAGE_SIZE);
    }
    None
}

fn is_jar_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
}

fn modrinth_loader_slugs(loader: &str) -> Vec<String> {
    modrinth_loader_slug(loader)
        .map(|value| vec![value.to_owned()])
        .unwrap_or_default()
}

fn modrinth_loader_slug(loader: &str) -> Option<&'static str> {
    match loader.trim().to_ascii_lowercase().as_str() {
        "fabric" => Some("fabric"),
        "forge" => Some("forge"),
        "neoforge" => Some("neoforge"),
        "quilt" => Some("quilt"),
        _ => None,
    }
}

fn normalized_game_versions(game_version: &str) -> Vec<String> {
    normalize_optional(game_version).into_iter().collect()
}

fn normalize_optional(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn non_empty_owned(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn content_hash_cache_path(instance_root: &Path) -> PathBuf {
    instance_root
        .join(CONTENT_HASH_CACHE_DIR_NAME)
        .join(CONTENT_HASH_CACHE_FILE_NAME)
}

fn normalize_lookup_key(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_installed_content_path_key(value: &str) -> String {
    value
        .trim()
        .trim_start_matches("./")
        .trim_start_matches(".\\")
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn derive_installed_lookup_query(path: &Path, fallback_file_name: &str) -> String {
    let raw = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(fallback_file_name)
        .trim();
    if raw.is_empty() {
        return fallback_file_name.to_owned();
    }

    let pieces: Vec<&str> = raw
        .split(['-', '_'])
        .map(str::trim)
        .filter(|piece| !piece.is_empty())
        .collect();
    if pieces.is_empty() {
        return raw.to_owned();
    }

    let mut kept = Vec::new();
    for piece in pieces {
        if looks_like_version_segment(piece) {
            break;
        }
        kept.push(piece);
    }

    if kept.is_empty() {
        raw.to_owned()
    } else {
        kept.join(" ")
    }
}

fn derive_raw_lookup_query(path: &Path, fallback_file_name: &str) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_file_name)
        .to_owned()
}

fn looks_like_version_segment(value: &str) -> bool {
    let normalized = value
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '.' && ch != '+')
        .to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    if normalized.chars().all(|ch| ch.is_ascii_digit()) {
        return true;
    }
    if normalized.starts_with('v')
        && normalized
            .chars()
            .skip(1)
            .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '+')
    {
        return true;
    }
    if normalized.starts_with("mc")
        && normalized
            .chars()
            .skip(2)
            .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '+')
    {
        return true;
    }
    if normalized.len() >= 8 && normalized.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return true;
    }
    normalized.chars().any(|ch| ch.is_ascii_digit())
        && normalized.chars().any(|ch| ch == '.' || ch == '+')
}

fn managed_lookup_key_suffix(
    managed_identity: Option<&InstalledContentIdentity>,
    lookup_query: &str,
) -> String {
    if let Some(identity) = managed_identity {
        if let Some(project_id) = identity.modrinth_project_id.as_deref() {
            return format!("modrinth:{project_id}");
        }
        if let Some(project_id) = identity.curseforge_project_id {
            return format!("curseforge:{project_id}");
        }
    }
    normalize_lookup_key(lookup_query)
}

fn choose_preferred_content_entry(
    entries: Vec<UnifiedContentEntry>,
    lookup_key: &str,
    kind: InstalledContentKind,
) -> Option<UnifiedContentEntry> {
    let target_key = lookup_key
        .split_once("::")
        .map(|(_, value)| value)
        .unwrap_or(lookup_key);
    if target_key.trim().is_empty() {
        return None;
    }

    let lookup_tokens = split_lookup_tokens(target_key);
    let canonical_lookup_tokens = trim_ignorable_lookup_suffix(lookup_tokens.as_slice());
    let mut best: Option<(i32, UnifiedContentEntry)> = None;

    for entry in entries {
        let mut score = 0i32;
        if kind_accepts_content_type(kind, entry.content_type.as_str()) {
            score += 80;
        } else {
            continue;
        }

        let normalized_name = normalize_lookup_key(entry.name.as_str());
        let entry_tokens = split_lookup_tokens(normalized_name.as_str());
        let mut overlap = 0i32;
        for token in canonical_lookup_tokens {
            if token.len() < 2 {
                continue;
            }
            if entry_tokens.iter().any(|entry_token| entry_token == token) {
                overlap += 1;
            }
        }
        let candidate_covers_lookup = !canonical_lookup_tokens.is_empty()
            && canonical_lookup_tokens
                .iter()
                .all(|token| entry_tokens.iter().any(|entry_token| entry_token == token));
        let lookup_covers_candidate = query_has_only_ignorable_suffix_tokens(
            lookup_tokens.as_slice(),
            entry_tokens.as_slice(),
        );
        if normalized_name != target_key
            && entry_tokens.as_slice() != canonical_lookup_tokens
            && !candidate_covers_lookup
            && !lookup_covers_candidate
        {
            continue;
        }

        if normalized_name == target_key || entry_tokens.as_slice() == canonical_lookup_tokens {
            score += 600;
        } else {
            if candidate_covers_lookup {
                score += 300;
                if normalized_name.contains(target_key) {
                    score += 40;
                }
            }
            score += overlap * 60;
            if lookup_covers_candidate {
                score += 140;
            }
            score -= (entry_tokens.len() as i32 - canonical_lookup_tokens.len() as i32).abs() * 8;
        }

        let distance = levenshtein_distance(normalized_name.as_str(), target_key);
        score -= distance.min(64);

        if !entry.summary.trim().is_empty() {
            score += 8;
        }
        if entry.icon_url.is_some() {
            score += 10;
        }
        score += match entry.source {
            ContentSource::Modrinth => 20,
            ContentSource::CurseForge => 10,
        };

        let should_replace = best.as_ref().is_none_or(|(best_score, best_entry)| {
            score > *best_score
                || (score == *best_score
                    && content_source_priority(entry.source)
                        > content_source_priority(best_entry.source))
        });
        if should_replace {
            best = Some((score, entry));
        }
    }

    best.map(|(_, entry)| entry)
}

fn split_lookup_tokens(value: &str) -> Vec<&str> {
    value
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .collect()
}

fn trim_ignorable_lookup_suffix<'a>(tokens: &'a [&'a str]) -> &'a [&'a str] {
    let trimmed_len = tokens
        .iter()
        .rposition(|token| !is_ignorable_lookup_suffix_token(token))
        .map(|index| index + 1)
        .unwrap_or(tokens.len());
    &tokens[..trimmed_len]
}

fn is_ignorable_lookup_suffix_token(token: &str) -> bool {
    matches!(
        token,
        "fabric"
            | "forge"
            | "neoforge"
            | "quilt"
            | "rift"
            | "liteloader"
            | "loader"
            | "mod"
            | "mods"
            | "minecraft"
            | "mc"
            | "client"
            | "server"
    )
}

fn query_has_only_ignorable_suffix_tokens(
    query_tokens: &[&str],
    candidate_tokens: &[&str],
) -> bool {
    query_tokens.starts_with(candidate_tokens)
        && query_tokens[candidate_tokens.len()..]
            .iter()
            .all(|token| is_ignorable_lookup_suffix_token(token))
}

fn content_source_priority(source: ContentSource) -> i32 {
    match source {
        ContentSource::Modrinth => 2,
        ContentSource::CurseForge => 1,
    }
}

fn kind_accepts_content_type(kind: InstalledContentKind, content_type: &str) -> bool {
    let normalized_type = normalize_lookup_key(content_type);
    match kind {
        InstalledContentKind::Mods => normalized_type.contains("mod"),
        InstalledContentKind::ResourcePacks => {
            normalized_type.contains("resource pack")
                || normalized_type.contains("resourcepack")
                || normalized_type.contains("texture pack")
        }
        InstalledContentKind::ShaderPacks => normalized_type.contains("shader"),
        InstalledContentKind::DataPacks => {
            normalized_type.contains("data pack") || normalized_type.contains("datapack")
        }
    }
}

fn levenshtein_distance(left: &str, right: &str) -> i32 {
    if left == right {
        return 0;
    }
    if left.is_empty() {
        return right.chars().count() as i32;
    }
    if right.is_empty() {
        return left.chars().count() as i32;
    }

    let right_chars: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_index, left_char) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != *right_char);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution_cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right_chars.len()] as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, source: ContentSource) -> UnifiedContentEntry {
        UnifiedContentEntry {
            id: format!("{}:{name}", source.label().to_ascii_lowercase()),
            name: name.to_owned(),
            summary: String::new(),
            content_type: "mod".to_owned(),
            source,
            project_url: None,
            icon_url: None,
        }
    }

    #[test]
    fn autodetect_prefers_exact_multi_token_name_over_short_prefix() {
        let selected = choose_preferred_content_entry(
            vec![
                entry("Voxy", ContentSource::Modrinth),
                entry("Voxy Worldgen", ContentSource::CurseForge),
            ],
            "mods::voxy worldgen",
            InstalledContentKind::Mods,
        )
        .expect("expected a matching entry");

        assert_eq!(selected.name, "Voxy Worldgen");
    }

    #[test]
    fn autodetect_allows_trailing_loader_noise_in_filename_queries() {
        let selected = choose_preferred_content_entry(
            vec![entry("Sodium", ContentSource::Modrinth)],
            "mods::sodium fabric",
            InstalledContentKind::Mods,
        )
        .expect("expected a matching entry");

        assert_eq!(selected.name, "Sodium");
    }

    #[test]
    fn autodetect_keeps_short_name_when_lookup_is_short_name() {
        let selected = choose_preferred_content_entry(
            vec![
                entry("Voxy", ContentSource::Modrinth),
                entry("Voxy Worldgen", ContentSource::CurseForge),
            ],
            "mods::voxy",
            InstalledContentKind::Mods,
        )
        .expect("expected a matching entry");

        assert_eq!(selected.name, "Voxy");
    }

    #[test]
    fn raw_lookup_query_preserves_full_jar_stem_for_fallback_search() {
        let path = Path::new("mods/foomod-neoforge-mc1.21.1-v1.3.0.jar");

        assert_eq!(
            derive_installed_lookup_query(path, "foomod-neoforge-mc1.21.1-v1.3.0.jar"),
            "foomod neoforge"
        );
        assert_eq!(
            derive_raw_lookup_query(path, "foomod-neoforge-mc1.21.1-v1.3.0.jar"),
            "foomod-neoforge-mc1.21.1-v1.3.0"
        );
    }

    #[test]
    fn levenshtein_distance_prefers_nearest_match() {
        assert!(levenshtein_distance("sodium", "sodium") < levenshtein_distance("sodium", "sod"));
        assert!(
            levenshtein_distance("iris shaders", "iris")
                < levenshtein_distance("iris shaders", "indium")
        );
    }
}
