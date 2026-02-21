use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Character,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Character => "character",
        }
    }
}

impl std::str::FromStr for MessageRole {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(Self::User),
            "character" => Ok(Self::Character),
            _ => Err(DomainError::InvalidField {
                field: "message_role",
                reason: "invalid message role value",
            }),
        }
    }
}
