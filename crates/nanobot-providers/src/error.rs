//! Provider error types

use thiserror::Error;

/// Provider error types
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("API key not configured for provider: {0}")]
    MissingApiKey(String),

    #[error("API base URL not configured for provider: {0}")]
    MissingApiBase(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Invalid response from provider: {0}")]
    InvalidResponse(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Request timeout")]
    Timeout,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ProviderError {
    /// Check if this error is transient (retry may succeed)
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Http(e) if e.is_timeout() || e.is_connect()
        ) || matches!(
            self,
            Self::RateLimit | ProviderError::Timeout
        )
    }
}

/// Result type alias for provider operations
pub type Result<T> = std::result::Result<T, ProviderError>;
