use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Invalid field '{field}': {reason}")]
    InvalidField {
        field: &'static str,
        reason: &'static str,
    },

    #[error("Business rule violation: {0}")]
    BusinessRuleViolation(String),

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Invalid state transition: {0}")]
    InvalidStateTransition(String),

    #[error("Insufficient credits")]
    InsufficientCredits,
}
