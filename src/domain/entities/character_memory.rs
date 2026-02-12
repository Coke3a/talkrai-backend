use chrono::{DateTime, Utc};

use crate::domain::error::DomainError;
use crate::domain::value_objects::{CharacterId, CharacterMemoryId, MemoryType, UserId};

pub struct CharacterMemory {
    id: CharacterMemoryId,
    user_id: UserId,
    character_id: CharacterId,
    memory_type: MemoryType,
    content: String,
    importance: i16,
    last_recalled_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl CharacterMemory {
    pub fn new(
        user_id: UserId,
        character_id: CharacterId,
        memory_type: MemoryType,
        content: String,
        importance: i16,
    ) -> Result<Self, DomainError> {
        if !(1..=10).contains(&importance) {
            return Err(DomainError::InvalidField {
                field: "importance",
                reason: "must be between 1 and 10",
            });
        }
        let now = Utc::now();
        Ok(Self {
            id: CharacterMemoryId::new(),
            user_id,
            character_id,
            memory_type,
            content,
            importance,
            last_recalled_at: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn from_existing(
        id: CharacterMemoryId,
        user_id: UserId,
        character_id: CharacterId,
        memory_type: MemoryType,
        content: String,
        importance: i16,
        last_recalled_at: Option<DateTime<Utc>>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            user_id,
            character_id,
            memory_type,
            content,
            importance,
            last_recalled_at,
            created_at,
            updated_at,
        }
    }

    pub fn id(&self) -> &CharacterMemoryId {
        &self.id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn character_id(&self) -> &CharacterId {
        &self.character_id
    }

    pub fn memory_type(&self) -> &MemoryType {
        &self.memory_type
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn importance(&self) -> i16 {
        self.importance
    }

    pub fn last_recalled_at(&self) -> Option<&DateTime<Utc>> {
        self.last_recalled_at.as_ref()
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }

    pub fn mark_recalled(&mut self) {
        self.last_recalled_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    pub fn update_importance(&mut self, importance: i16) -> Result<(), DomainError> {
        if !(1..=10).contains(&importance) {
            return Err(DomainError::InvalidField {
                field: "importance",
                reason: "must be between 1 and 10",
            });
        }
        self.importance = importance;
        self.updated_at = Utc::now();
        Ok(())
    }
}
