use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Narrator,
    Character,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Narrator => "narrator",
            Self::Character => "character",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, DomainError> {
        match s {
            "user" => Ok(Self::User),
            "narrator" => Ok(Self::Narrator),
            "character" => Ok(Self::Character),
            _ => Err(DomainError::InvalidField {
                field: "message_role",
                reason: "invalid message role value",
            }),
        }
    }
}
