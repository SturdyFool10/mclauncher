use std::fs;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

use crate::constants::{ACCOUNT_CACHE_APP_DIR, ACCOUNT_CACHE_FILENAME, LEGACY_ACCOUNT_CACHE_PATH};
use crate::error::AuthError;
use crate::secret_store;
use crate::types::{CachedAccount, CachedAccountsState};

#[track_caller]
fn fs_read_to_string(path: impl AsRef<Path>) -> Result<String, AuthError> {
    let path = path.as_ref();
    tracing::debug!(target: "vertexlauncher/io", op = "read_to_string", path = %path.display());
    Ok(fs::read_to_string(path)?)
}

#[track_caller]
fn fs_remove_file(path: impl AsRef<Path>) -> Result<(), AuthError> {
    let path = path.as_ref();
    tracing::debug!(target: "vertexlauncher/io", op = "remove_file", path = %path.display());
    Ok(fs::remove_file(path)?)
}

pub(crate) fn load_cached_accounts() -> Result<CachedAccountsState, AuthError> {
    let path = account_cache_path();
    migrate_legacy_disk_cache_to_secure_storage(&path)?;

    match secret_store::load_accounts_state()? {
        Some(contents) => parse_cached_accounts_state(contents.as_str()),
        None => {
            tracing::debug!(
                target: "vertexlauncher/auth/cache",
                "secure cached accounts state missing; using empty cache"
            );
            Ok(CachedAccountsState::default())
        }
    }
}

pub(crate) fn save_cached_accounts(state: &CachedAccountsState) -> Result<(), AuthError> {
    let path = account_cache_path();
    migrate_legacy_disk_cache_to_secure_storage(&path)?;
    let previous_profile_ids = load_cached_profile_ids_from_secure_storage()?;
    let mut normalized = state.clone().normalize();
    let current_profile_ids = normalized
        .accounts
        .iter()
        .map(|account| account.minecraft_profile.id.clone())
        .collect::<Vec<_>>();

    for account in &mut normalized.accounts {
        persist_refresh_token(account)?;
        sanitize_cached_profile(account);
    }
    let json = serde_json::to_string(&normalized)?;
    secret_store::store_accounts_state(&json)?;

    if path.exists() {
        let _ = fs_remove_file(&path);
    }

    for profile_id in previous_profile_ids {
        if !current_profile_ids
            .iter()
            .any(|current| current == &profile_id)
        {
            secret_store::delete_refresh_token(&profile_id)?;
        }
    }

    Ok(())
}

pub(crate) fn clear_cached_accounts() -> Result<(), AuthError> {
    let path = account_cache_path();
    let previous_profile_ids = load_cached_profile_ids_from_secure_storage()?;

    if path.exists() {
        let _ = fs_remove_file(path);
    }
    secret_store::delete_accounts_state()?;

    for profile_id in previous_profile_ids {
        secret_store::delete_refresh_token(&profile_id)?;
    }

    Ok(())
}

pub(crate) fn load_cached_account() -> Result<Option<CachedAccount>, AuthError> {
    let state = load_cached_accounts()?;
    Ok(state.active_account().cloned())
}

pub(crate) fn save_cached_account(account: &CachedAccount) -> Result<(), AuthError> {
    let mut state = load_cached_accounts()?;
    state.upsert_and_activate(account.clone());
    save_cached_accounts(&state)
}

pub(crate) fn clear_cached_account() -> Result<(), AuthError> {
    clear_cached_accounts()
}

fn account_cache_path() -> PathBuf {
    std::env::var("VERTEX_ACCOUNT_CACHE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_account_cache_path())
}

fn default_account_cache_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            if !local_app_data.trim().is_empty() {
                return PathBuf::from(local_app_data)
                    .join(ACCOUNT_CACHE_APP_DIR)
                    .join(ACCOUNT_CACHE_FILENAME);
            }
        }

        if let Ok(app_data) = std::env::var("APPDATA") {
            if !app_data.trim().is_empty() {
                return PathBuf::from(app_data)
                    .join(ACCOUNT_CACHE_APP_DIR)
                    .join(ACCOUNT_CACHE_FILENAME);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            if !home.trim().is_empty() {
                return PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join(ACCOUNT_CACHE_APP_DIR)
                    .join(ACCOUNT_CACHE_FILENAME);
            }
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(state_home) = std::env::var("XDG_STATE_HOME") {
            if !state_home.trim().is_empty() {
                return PathBuf::from(state_home)
                    .join(ACCOUNT_CACHE_APP_DIR)
                    .join(ACCOUNT_CACHE_FILENAME);
            }
        }

        if let Ok(home) = std::env::var("HOME") {
            if !home.trim().is_empty() {
                return PathBuf::from(home)
                    .join(".local")
                    .join("state")
                    .join(ACCOUNT_CACHE_APP_DIR)
                    .join(ACCOUNT_CACHE_FILENAME);
            }
        }
    }

    PathBuf::from(ACCOUNT_CACHE_FILENAME)
}

fn migrate_legacy_disk_cache_to_secure_storage(target_path: &Path) -> Result<(), AuthError> {
    if secret_store::load_accounts_state()?.is_some() {
        remove_legacy_account_cache_file(target_path);
        remove_legacy_account_cache_file(Path::new(LEGACY_ACCOUNT_CACHE_PATH));
        return Ok(());
    }

    let Some(state) = load_cached_accounts_state_from_disk(target_path)? else {
        remove_legacy_account_cache_file(Path::new(LEGACY_ACCOUNT_CACHE_PATH));
        return Ok(());
    };

    let mut normalized = state.normalize();
    for account in &mut normalized.accounts {
        persist_refresh_token(account)?;
        sanitize_cached_profile(account);
    }
    let serialized = serde_json::to_string(&normalized)?;
    secret_store::store_accounts_state(&serialized)?;

    remove_legacy_account_cache_file(target_path);
    remove_legacy_account_cache_file(Path::new(LEGACY_ACCOUNT_CACHE_PATH));
    tracing::info!(
        target: "vertexlauncher/auth/cache",
        "migrated cached account metadata from disk into secure storage"
    );
    Ok(())
}

fn load_cached_accounts_state_from_disk(
    path: &Path,
) -> Result<Option<CachedAccountsState>, AuthError> {
    let candidate_paths = if path == Path::new(LEGACY_ACCOUNT_CACHE_PATH) {
        vec![path.to_path_buf()]
    } else {
        vec![path.to_path_buf(), PathBuf::from(LEGACY_ACCOUNT_CACHE_PATH)]
    };

    for candidate in candidate_paths {
        if !candidate.exists() {
            continue;
        }
        let contents = fs_read_to_string(&candidate)?;
        if let Ok(state) = serde_json::from_str::<CachedAccountsState>(&contents) {
            return Ok(Some(state));
        }
        if let Ok(single_account) = serde_json::from_str::<CachedAccount>(&contents) {
            tracing::info!(
                target: "vertexlauncher/auth/cache",
                "migrated single-account cache format into multi-account state"
            );
            let mut state = CachedAccountsState::default();
            state.upsert_and_activate(single_account);
            return Ok(Some(state));
        }
    }

    Ok(None)
}

fn remove_legacy_account_cache_file(path: &Path) {
    if path.exists() {
        let _ = fs_remove_file(path);
    }
}

fn parse_cached_accounts_state(contents: &str) -> Result<CachedAccountsState, AuthError> {
    match serde_json::from_str::<CachedAccountsState>(contents) {
        Ok(state) => finalize_loaded_accounts(state.normalize()),
        Err(state_error) => {
            if let Ok(single_account) = serde_json::from_str::<CachedAccount>(contents) {
                tracing::info!(
                    target: "vertexlauncher/auth/cache",
                    "migrated single-account secure cache format into multi-account state"
                );
                let mut state = CachedAccountsState::default();
                state.upsert_and_activate(single_account);
                return finalize_loaded_accounts(state);
            }

            tracing::warn!(
                target: "vertexlauncher/auth/cache",
                error = %state_error,
                "failed to parse secure cached accounts state"
            );
            Err(AuthError::Json(state_error))
        }
    }
}

fn load_cached_profile_ids_from_secure_storage() -> Result<Vec<String>, AuthError> {
    let Some(contents) = secret_store::load_accounts_state()? else {
        return Ok(Vec::new());
    };
    if let Ok(state) = serde_json::from_str::<CachedAccountsState>(&contents) {
        return Ok(state
            .accounts
            .into_iter()
            .map(|account| account.minecraft_profile.id)
            .filter(|profile_id| !profile_id.trim().is_empty())
            .collect());
    }

    if let Ok(account) = serde_json::from_str::<CachedAccount>(&contents) {
        if account.minecraft_profile.id.trim().is_empty() {
            return Ok(Vec::new());
        }
        return Ok(vec![account.minecraft_profile.id]);
    }

    Ok(Vec::new())
}

fn sanitize_cached_profile(account: &mut CachedAccount) {
    // Runtime access tokens stay in memory only, and refresh tokens are
    // persisted separately in OS-backed secure storage.
    account.minecraft_access_token = None;
    account.microsoft_refresh_token = None;

    // Keep lightweight identity/profile metadata only; avoid stale or heavy
    // texture payloads in the on-disk cache.
    account.minecraft_profile.skins.clear();
    for cape in &mut account.minecraft_profile.capes {
        cape.state.clear();
        cape.texture_png_base64 = None;
    }
}

fn finalize_loaded_accounts(
    mut state: CachedAccountsState,
) -> Result<CachedAccountsState, AuthError> {
    let mut migrated_plaintext_token = false;

    for account in &mut state.accounts {
        let profile_id = account.minecraft_profile.id.trim();
        if profile_id.is_empty() {
            account.microsoft_refresh_token = None;
            continue;
        }

        if let Some(token) = account
            .microsoft_refresh_token
            .as_deref()
            .map(str::trim)
            .filter(|token| !token.is_empty())
        {
            secret_store::store_refresh_token(profile_id, token)?;
            migrated_plaintext_token = true;
        }

        account.microsoft_refresh_token = secret_store::load_refresh_token(profile_id)?
            .map(Zeroizing::new)
            .map(|token| token.to_string());
    }

    if migrated_plaintext_token {
        save_cached_accounts(&state)?;
    }

    Ok(state)
}

fn persist_refresh_token(account: &mut CachedAccount) -> Result<(), AuthError> {
    let profile_id = account.minecraft_profile.id.trim();
    if profile_id.is_empty() {
        account.microsoft_refresh_token = None;
        return Ok(());
    }

    match account
        .microsoft_refresh_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        Some(token) => {
            secret_store::store_refresh_token(profile_id, token)?;
            let stored_token = secret_store::load_refresh_token(profile_id)?;
            let stored = stored_token
                .as_deref()
                .map(str::trim)
                .filter(|stored| !stored.is_empty())
                .ok_or_else(|| {
                    AuthError::SecureStorage(format!(
                        "Refresh token for profile '{profile_id}' was written to secure storage but could not be reloaded."
                    ))
                })?;
            if stored != token {
                return Err(AuthError::SecureStorage(format!(
                    "Refresh token for profile '{profile_id}' did not round-trip correctly through secure storage."
                )));
            }
        }
        None => secret_store::delete_refresh_token(profile_id)?,
    }

    Ok(())
}
