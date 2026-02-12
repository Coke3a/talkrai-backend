use thiserror::Error;

#[derive(Debug, Error)]
pub enum LineClientError {
    #[error("Invalid signature")]
    InvalidSignature,

    #[error("LINE API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Network error: {0}")]
    NetworkError(#[source] anyhow::Error),
}
