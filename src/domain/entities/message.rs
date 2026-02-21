use chrono::{DateTime, Utc};

use crate::domain::value_objects::{MessageId, MessageRole, SessionId};

pub struct Message {
    id: MessageId,
    session_id: SessionId,
    role: MessageRole,
    content: String,
    mood: Option<String>,
    created_at: DateTime<Utc>,
}

impl Message {
    pub fn new(
        session_id: SessionId,
        role: MessageRole,
        content: String,
        mood: Option<String>,
    ) -> Self {
        Self {
            id: MessageId::new(),
            session_id,
            role,
            content,
            mood,
            created_at: Utc::now(),
        }
    }

    pub fn from_existing(
        id: MessageId,
        session_id: SessionId,
        role: MessageRole,
        content: String,
        mood: Option<String>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            session_id,
            role,
            content,
            mood,
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

    pub fn mood(&self) -> Option<&str> {
        self.mood.as_deref()
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }
}
