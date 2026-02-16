use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageType {
    Dialogue,
    Action,
    Mixed,
    Narration,
    System,
}

impl MessageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dialogue => "dialogue",
            Self::Action => "action",
            Self::Mixed => "mixed",
            Self::Narration => "narration",
            Self::System => "system",
        }
    }

    /// Classify user input text into message type
    pub fn classify_user_input(text: &str) -> Self {
        let trimmed = text.trim();
        let has_action = trimmed.contains('*');
        let has_dialogue = !trimmed.replace('*', "").trim().is_empty() && !trimmed.starts_with('*')
            || !trimmed.ends_with('*');

        match (has_dialogue, has_action) {
            (true, true) => Self::Mixed,
            (false, true) => Self::Action,
            _ => Self::Dialogue,
        }
    }
}

impl std::str::FromStr for MessageType {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dialogue" => Ok(Self::Dialogue),
            "action" => Ok(Self::Action),
            "mixed" => Ok(Self::Mixed),
            "narration" => Ok(Self::Narration),
            "system" => Ok(Self::System),
            _ => Err(DomainError::InvalidField {
                field: "message_type",
                reason: "invalid message type value",
            }),
        }
    }
}
