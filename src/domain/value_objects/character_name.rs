use crate::domain::error::DomainError;

#[derive(Debug, Clone)]
pub struct CharacterName(String);

impl CharacterName {
    pub fn new(value: String) -> Result<Self, DomainError> {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            return Err(DomainError::InvalidField {
                field: "character_name",
                reason: "must not be empty",
            });
        }
        if trimmed.len() > 100 {
            return Err(DomainError::InvalidField {
                field: "character_name",
                reason: "must not exceed 100 characters",
            });
        }
        Ok(Self(trimmed))
    }

    pub fn from_trusted(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
