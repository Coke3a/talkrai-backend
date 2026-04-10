use thiserror::Error;

use crate::domain::error::DomainError;
use crate::domain::repositories::RepoError;
use crate::domain::services::ai_client_error::AiClientError;
use crate::domain::services::beam_client_error::BeamClientError;
use crate::domain::services::line_client_error::LineClientError;

#[derive(Debug, Error)]
pub enum UsecaseError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Insufficient credits")]
    InsufficientCredits,

    #[error("Already checked in today")]
    AlreadyCheckedIn,

    #[error("LINE API error: {0}")]
    LineError(String),

    #[error("AI processing error: {0}")]
    AiError(String),

    #[error("AI response invalid after all retries")]
    AiResponseInvalid,

    #[error("Payment error: {0}")]
    PaymentError(String),

    #[error("Infrastructure error")]
    Infra(#[source] anyhow::Error),
}

impl From<DomainError> for UsecaseError {
    fn from(err: DomainError) -> Self {
        match err {
            DomainError::NotFound(msg) => UsecaseError::NotFound(msg),
            DomainError::InsufficientCredits => UsecaseError::InsufficientCredits,
            other => UsecaseError::Validation(other.to_string()),
        }
    }
}

impl From<RepoError> for UsecaseError {
    fn from(err: RepoError) -> Self {
        match err {
            RepoError::NotFound(msg) => UsecaseError::NotFound(msg),
            other => UsecaseError::Infra(anyhow::Error::new(other)),
        }
    }
}

impl From<LineClientError> for UsecaseError {
    fn from(err: LineClientError) -> Self {
        UsecaseError::LineError(err.to_string())
    }
}

impl From<AiClientError> for UsecaseError {
    fn from(err: AiClientError) -> Self {
        UsecaseError::AiError(err.to_string())
    }
}

impl From<BeamClientError> for UsecaseError {
    fn from(err: BeamClientError) -> Self {
        UsecaseError::PaymentError(err.to_string())
    }
}
