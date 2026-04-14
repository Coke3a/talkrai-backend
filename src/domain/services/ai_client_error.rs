use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiClientError {
    #[error("AI API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("AI response parse error: {0}")]
    ParseError(String),

    #[error("AI rate limited")]
    RateLimited,

    #[error("Network error: {0}")]
    NetworkError(#[source] anyhow::Error),
}

impl AiClientError {
    /// Whether this error is transient and worth retrying.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::NetworkError(_) | Self::RateLimited => true,
            Self::ApiError { status, .. } => matches!(status, 408 | 500 | 502 | 503 | 529),
            Self::ParseError(_) => false,
        }
    }
}
