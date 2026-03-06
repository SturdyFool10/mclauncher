use auth::{CachedAccount, DeviceCodeLoginFlow, DeviceCodePrompt, LoginEvent};
use std::time::Duration;

use super::tokio_runtime;

pub const REPAINT_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Clone, Debug)]
pub enum AuthUiStatus {
    Idle,
    Starting,
    AwaitingCode,
    WaitingForAuthorization,
    Success(String),
    Error(String),
}

impl AuthUiStatus {
    fn status_message(&self) -> Option<&str> {
        match self {
            AuthUiStatus::Idle => None,
            AuthUiStatus::Starting => Some("Requesting Microsoft device code..."),
            AuthUiStatus::AwaitingCode => Some("Waiting for you to finish Microsoft sign-in..."),
            AuthUiStatus::WaitingForAuthorization => {
                Some("Waiting for Microsoft authorization confirmation...")
            }
            AuthUiStatus::Success(message) | AuthUiStatus::Error(message) => Some(message.as_str()),
        }
    }
}

pub struct AuthState {
    account: Option<CachedAccount>,
    avatar_png: Option<Vec<u8>>,
    flow: Option<DeviceCodeLoginFlow>,
    status: AuthUiStatus,
    device_prompt: Option<DeviceCodePrompt>,
}

impl AuthState {
    pub fn load() -> Self {
        let (account, status) = match auth::load_cached_account() {
            Ok(account) => (account, AuthUiStatus::Idle),
            Err(err) => (
                None,
                AuthUiStatus::Error(format!("Failed to load cached account state: {err}")),
            ),
        };

        Self {
            avatar_png: account.as_ref().and_then(CachedAccount::avatar_png_bytes),
            account,
            flow: None,
            status,
            device_prompt: None,
        }
    }

    pub fn poll(&mut self) {
        let mut flow_finished = false;

        if let Some(flow) = self.flow.as_mut() {
            for event in flow.poll_events() {
                match event {
                    LoginEvent::DeviceCode(prompt) => {
                        self.device_prompt = Some(prompt);
                        self.status = AuthUiStatus::AwaitingCode;
                    }
                    LoginEvent::WaitingForAuthorization => {
                        self.status = AuthUiStatus::WaitingForAuthorization;
                    }
                    LoginEvent::Completed(account) => {
                        self.avatar_png = account.avatar_png_bytes();
                        self.status = AuthUiStatus::Success(format!(
                            "Signed in as {}",
                            account.minecraft_profile.name
                        ));
                        self.account = Some(account.clone());

                        if let Err(err) = auth::save_cached_account(&account) {
                            self.status = AuthUiStatus::Error(format!(
                                "Sign-in succeeded, but failed to cache account state: {err}",
                            ));
                        }

                        self.device_prompt = None;
                        flow_finished = true;
                    }
                    LoginEvent::Failed(err) => {
                        self.status = AuthUiStatus::Error(err);
                        self.device_prompt = None;
                        flow_finished = true;
                    }
                }
            }

            if flow.is_finished() {
                flow_finished = true;
            }
        }

        if flow_finished {
            self.flow = None;
        }
    }

    pub fn start_sign_in(&mut self) {
        if self.flow.is_some() {
            return;
        }

        let client_id = match microsoft_client_id() {
            Ok(client_id) => client_id,
            Err(err) => {
                self.status = AuthUiStatus::Error(err);
                return;
            }
        };

        self.device_prompt = None;
        self.status = AuthUiStatus::Starting;
        self.flow = Some(auth::start_device_code_login_with_handle(
            client_id,
            tokio_runtime::handle(),
        ));
    }

    pub fn sign_out(&mut self) {
        self.flow = None;
        self.account = None;
        self.avatar_png = None;
        self.device_prompt = None;
        self.status = AuthUiStatus::Idle;

        if let Err(err) = auth::clear_cached_account() {
            self.status = AuthUiStatus::Error(format!(
                "Signed out in memory, but failed to clear cached account state: {err}",
            ));
        }
    }

    pub fn should_request_repaint(&self) -> bool {
        self.flow.is_some()
    }

    pub fn sign_in_in_progress(&self) -> bool {
        self.flow.is_some()
    }

    pub fn display_name(&self) -> Option<&str> {
        self.account
            .as_ref()
            .map(|account| account.minecraft_profile.name.as_str())
    }

    pub fn avatar_png(&self) -> Option<&[u8]> {
        self.avatar_png.as_deref()
    }

    pub fn status_message(&self) -> Option<&str> {
        self.status.status_message()
    }

    pub fn device_prompt(&self) -> Option<&DeviceCodePrompt> {
        self.device_prompt.as_ref()
    }
}

fn microsoft_client_id() -> Result<String, String> {
    let client_id = std::env::var("VERTEX_MSA_CLIENT_ID")
        .ok()
        .map(|raw| raw.trim().to_owned())
        .filter(|raw| !raw.is_empty())
        .or_else(|| auth::builtin_client_id().map(str::to_owned))
        .ok_or_else(|| {
            "Microsoft OAuth client ID is not configured. Set VERTEX_MSA_CLIENT_ID or set \
auth::BUILTIN_MICROSOFT_CLIENT_ID in crates/auth/src/lib.rs."
                .to_owned()
        })?;

    if is_valid_microsoft_client_id(&client_id) {
        Ok(client_id)
    } else {
        Err(format!(
            "Invalid Microsoft client id '{client_id}'. Set VERTEX_MSA_CLIENT_ID to a valid \
16-character hex id or GUID application id.",
        ))
    }
}

fn is_valid_microsoft_client_id(value: &str) -> bool {
    is_hex_client_id(value) || is_guid_client_id(value)
}

fn is_hex_client_id(value: &str) -> bool {
    value.len() == 16 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn is_guid_client_id(value: &str) -> bool {
    if value.len() != 36 {
        return false;
    }

    for (index, ch) in value.chars().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if ch != '-' {
                return false;
            }
            continue;
        }

        if !ch.is_ascii_hexdigit() {
            return false;
        }
    }

    true
}
