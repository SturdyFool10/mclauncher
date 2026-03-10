use std::borrow::Cow;

pub const REDACTED_ACCOUNT_LABEL: &str = "Hidden Account";
pub const REDACTED_IDENTIFIER_LABEL: &str = "[redacted]";

pub fn redact_account_label<'a>(streamer_mode: bool, label: &'a str) -> Cow<'a, str> {
    if streamer_mode {
        Cow::Borrowed(REDACTED_ACCOUNT_LABEL)
    } else {
        Cow::Borrowed(label)
    }
}

pub fn redact_sensitive_text<'a>(streamer_mode: bool, text: &'a str) -> Cow<'a, str> {
    if !streamer_mode {
        return Cow::Borrowed(text);
    }

    let mut changed = false;
    let mut sanitized_parts = Vec::new();
    for part in text.split_whitespace() {
        let redacted = redact_identifier_token(part);
        if redacted != part {
            changed = true;
        }
        sanitized_parts.push(redacted);
    }

    if !changed {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(sanitized_parts.join(" "))
    }
}

fn redact_identifier_token(token: &str) -> String {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return token.to_owned();
    }

    let prefix_len = trimmed
        .find(|ch: char| ch.is_ascii_alphanumeric())
        .unwrap_or(trimmed.len());
    let suffix_len = trimmed
        .chars()
        .rev()
        .take_while(|ch| !ch.is_ascii_alphanumeric())
        .count();
    let core_end = trimmed.len().saturating_sub(suffix_len);
    if prefix_len >= core_end {
        return token.to_owned();
    }

    let prefix = &trimmed[..prefix_len];
    let core = &trimmed[prefix_len..core_end];
    let suffix = &trimmed[core_end..];

    if looks_like_sensitive_identifier(core) {
        format!("{prefix}{REDACTED_IDENTIFIER_LABEL}{suffix}")
    } else {
        token.to_owned()
    }
}

fn looks_like_sensitive_identifier(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("profile_id=")
        || lower.starts_with("player_uuid=")
        || lower.starts_with("xuid=")
        || lower.starts_with("account_key=")
        || lower.starts_with("display_name=")
        || lower.starts_with("player_name=")
    {
        return true;
    }

    is_hex_identifier(&lower) || is_uuid_identifier(&lower)
}

fn is_hex_identifier(value: &str) -> bool {
    value.len() >= 16 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn is_uuid_identifier(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 5 {
        return false;
    }
    let expected = [8, 4, 4, 4, 12];
    parts
        .iter()
        .zip(expected)
        .all(|(part, len)| part.len() == len && part.chars().all(|ch| ch.is_ascii_hexdigit()))
}
