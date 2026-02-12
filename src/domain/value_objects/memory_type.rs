use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryType {
    Fact,
    Event,
    Preference,
    Relationship,
}

impl MemoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Event => "event",
            Self::Preference => "preference",
            Self::Relationship => "relationship",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, DomainError> {
        match s {
            "fact" => Ok(Self::Fact),
            "event" => Ok(Self::Event),
            "preference" => Ok(Self::Preference),
            "relationship" => Ok(Self::Relationship),
            _ => Err(DomainError::InvalidField {
                field: "memory_type",
                reason: "invalid memory type value",
            }),
        }
    }
}
