use thiserror::Error;

#[derive(Debug, Error)]
pub enum BeamClientError {
    #[error("Beam API request failed: {0}")]
    RequestFailed(String),

    #[error("Beam API returned error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Failed to parse Beam API response: {0}")]
    ParseError(String),
}
