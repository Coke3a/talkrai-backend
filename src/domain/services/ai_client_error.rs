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
