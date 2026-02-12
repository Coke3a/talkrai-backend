use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobMode {
    RoleplayMessage,
    StickerMessage,
    ImageMessage,
    FollowEvent,
    UnfollowEvent,
    PostbackEvent,
}

impl JobMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RoleplayMessage => "roleplay_message",
            Self::StickerMessage => "sticker_message",
            Self::ImageMessage => "image_message",
            Self::FollowEvent => "follow_event",
            Self::UnfollowEvent => "unfollow_event",
            Self::PostbackEvent => "postback_event",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, DomainError> {
        match s {
            "roleplay_message" => Ok(Self::RoleplayMessage),
            "sticker_message" => Ok(Self::StickerMessage),
            "image_message" => Ok(Self::ImageMessage),
            "follow_event" => Ok(Self::FollowEvent),
            "unfollow_event" => Ok(Self::UnfollowEvent),
            "postback_event" => Ok(Self::PostbackEvent),
            _ => Err(DomainError::InvalidField {
                field: "job_mode",
                reason: "invalid job mode value",
            }),
        }
    }
}
