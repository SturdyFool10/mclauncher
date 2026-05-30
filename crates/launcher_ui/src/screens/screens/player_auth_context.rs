use std::collections::HashMap;

use super::LaunchAuthContext;

/// Bundles all active-player/account state the render layer needs.
///
/// Replaces five separate parameters (`active_username`, `active_launch_auth`,
/// `active_account_owns_minecraft`, `token_refresh_in_progress`,
/// `account_avatars_by_key`) that were previously threaded independently
/// through every screen function and its sub-functions.
pub struct PlayerAuthContext<'a> {
    /// Full launch credentials.  `None` when no account is selected or the
    /// Minecraft profile is incomplete (e.g. player UUID not yet fetched).
    pub launch_auth: Option<LaunchAuthContext>,
    /// `true` while a Microsoft token renewal is in flight.  Launch buttons
    /// are disabled until this clears to avoid online-server failures caused
    /// by a stale access token.
    pub token_refresh_in_progress: bool,
    /// Per-account avatar PNG bytes keyed by lowercase player UUID.
    pub account_avatars: &'a HashMap<String, Vec<u8>>,
}

impl<'a> PlayerAuthContext<'a> {
    /// Display name from the active account profile.  Returns `None` when no
    /// account is signed in or the profile name is empty.
    pub fn display_name(&self) -> Option<&str> {
        self.launch_auth
            .as_ref()
            .map(|a| a.player_name.as_str())
            .filter(|s| !s.trim().is_empty())
    }

    /// `true` when the active account has a complete Minecraft profile (both
    /// player name and UUID are present).  Equivalent to `launch_auth.is_some()`.
    pub fn owns_minecraft(&self) -> bool {
        self.launch_auth.is_some()
    }
}
