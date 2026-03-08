pub(crate) const OAUTH_BASE_URL: &str = "https://login.microsoftonline.com";
pub(crate) const LIVE_AUTHORIZE_URL: &str = "https://login.live.com/oauth20_authorize.srf";
pub(crate) const LIVE_TOKEN_URL: &str = "https://login.live.com/oauth20_token.srf";
pub(crate) const LIVE_REDIRECT_URI: &str = "https://login.live.com/oauth20_desktop.srf";
pub(crate) const LIVE_SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL offline_access";
pub(crate) const DEVICE_CODE_SCOPE: &str = "XboxLive.signin offline_access";

pub(crate) const XBOX_USER_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
pub(crate) const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
pub(crate) const MINECRAFT_LOGIN_URL: &str = "https://api.minecraftservices.com/launcher/login";
pub(crate) const MINECRAFT_LOGIN_LEGACY_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
pub(crate) const MINECRAFT_ENTITLEMENTS_URL: &str =
    "https://api.minecraftservices.com/entitlements/mcstore";
pub(crate) const MINECRAFT_PROFILE_URL: &str =
    "https://api.minecraftservices.com/minecraft/profile";
pub(crate) const MINECRAFT_PROFILE_SKINS_URL: &str =
    "https://api.minecraftservices.com/minecraft/profile/skins";
pub(crate) const MINECRAFT_PROFILE_CAPE_ACTIVE_URL: &str =
    "https://api.minecraftservices.com/minecraft/profile/capes/active";

pub(crate) const ACCOUNT_CACHE_FILENAME: &str = "account_cache.json";
pub(crate) const ACCOUNT_CACHE_APP_DIR: &str = "vertex-launcher";
pub(crate) const LEGACY_ACCOUNT_CACHE_PATH: &str = "account_cache.json";

/// Built-in Microsoft OAuth client id used when `VERTEX_MSA_CLIENT_ID` is not set.
/// Leave empty to force env-based configuration.
pub const BUILTIN_MICROSOFT_CLIENT_ID: &str = "00000000402b5328";
/// Built-in OAuth tenant used when `VERTEX_MSA_TENANT` is not set.
pub const BUILTIN_MICROSOFT_TENANT: &str = "consumers";
