use chrono::{DateTime, Utc};

use crate::domain::value_objects::{MessageId, MessageRole, MessageType, SessionId};

pub struct Message {
    id: MessageId,
    session_id: SessionId,
    role: MessageRole,
    message_type: MessageType,
    content: String,
    created_at: DateTime<Utc>,
}

impl Message {
    pub fn new(
        session_id: SessionId,
        role: MessageRole,
        message_type: MessageType,
        content: String,
    ) -> Self {
        Self {
            id: MessageId::new(),
            session_id,
            role,
            message_type,
            content,
            created_at: Utc::now(),
        }
    }

    pub fn from_existing(
        id: MessageId,
        session_id: SessionId,
        role: MessageRole,
        message_type: MessageType,
        content: String,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            session_id,
            role,
            message_type,
            content,
            created_at,
        }
    }

    pub fn id(&self) -> &MessageId {
        &self.id
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn role(&self) -> &MessageRole {
        &self.role
    }

    pub fn message_type(&self) -> &MessageType {
        &self.message_type
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }
}
