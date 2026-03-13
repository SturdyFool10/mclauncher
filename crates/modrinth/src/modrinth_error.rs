/// Errors returned by Modrinth API requests.
#[derive(Debug, thiserror::Error)]
pub enum ModrinthError {
    #[error("HTTP status {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("HTTP transport error: {0}")]
    Transport(String),
    #[error("Response read error: {0}")]
    Read(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid Modrinth hash algorithm: {0}")]
    InvalidHashAlgorithm(String),
}
