use chrono::{DateTime, Utc};

use crate::domain::error::DomainError;
use crate::domain::value_objects::{
    CharacterId, CharacterMood, RelationshipLevel, SceneId, SessionId, SessionStatus, UserId,
};

pub struct RoleplaySession {
    id: SessionId,
    user_id: UserId,
    character_id: CharacterId,
    scene_id: SceneId,
    status: SessionStatus,
    mood: CharacterMood,
    relationship_level: RelationshipLevel,
    message_count: i32,
    current_location: Option<String>,
    scene_time: Option<String>,
    scene_summary: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl RoleplaySession {
    pub fn new(
        user_id: UserId,
        character_id: CharacterId,
        scene_id: SceneId,
        start_relationship_level: RelationshipLevel,
        start_mood: CharacterMood,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: SessionId::new(),
            user_id,
            character_id,
            scene_id,
            status: SessionStatus::Active,
            mood: start_mood,
            relationship_level: start_relationship_level,
            message_count: 0,
            current_location: None,
            scene_time: None,
            scene_summary: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_existing(
        id: SessionId,
        user_id: UserId,
        character_id: CharacterId,
        scene_id: SceneId,
        status: SessionStatus,
        mood: CharacterMood,
        relationship_level: RelationshipLevel,
        message_count: i32,
        current_location: Option<String>,
        scene_time: Option<String>,
        scene_summary: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            user_id,
            character_id,
            scene_id,
            status,
            mood,
            relationship_level,
            message_count,
            current_location,
            scene_time,
            scene_summary,
            created_at,
            updated_at,
        }
    }

    pub fn id(&self) -> &SessionId {
        &self.id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn character_id(&self) -> &CharacterId {
        &self.character_id
    }

    pub fn scene_id(&self) -> &SceneId {
        &self.scene_id
    }

    pub fn status(&self) -> &SessionStatus {
        &self.status
    }

    pub fn mood(&self) -> &CharacterMood {
        &self.mood
    }

    pub fn relationship_level(&self) -> &RelationshipLevel {
        &self.relationship_level
    }

    pub fn message_count(&self) -> i32 {
        self.message_count
    }

    pub fn current_location(&self) -> Option<&str> {
        self.current_location.as_deref()
    }

    pub fn scene_time(&self) -> Option<&str> {
        self.scene_time.as_deref()
    }

    pub fn scene_summary(&self) -> Option<&str> {
        self.scene_summary.as_deref()
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }

    pub fn pause(&mut self) -> Result<(), DomainError> {
        self.status.transition_to(&SessionStatus::Paused)?;
        self.status = SessionStatus::Paused;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), DomainError> {
        self.status.transition_to(&SessionStatus::Active)?;
        self.status = SessionStatus::Active;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn end(&mut self) -> Result<(), DomainError> {
        self.status.transition_to(&SessionStatus::Ended)?;
        self.status = SessionStatus::Ended;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn update_mood(&mut self, mood: CharacterMood) {
        self.mood = mood;
        self.updated_at = Utc::now();
    }

    /// Increment message count and check for relationship level up
    pub fn increment_messages(&mut self) -> Option<RelationshipLevel> {
        self.message_count += 1;
        self.updated_at = Utc::now();

        if let Some(threshold) = self.relationship_level.messages_threshold() {
            if self.message_count as u32 >= threshold {
                if let Some(next_level) = self.relationship_level.next() {
                    self.relationship_level = next_level.clone();
                    return Some(next_level);
                }
            }
        }
        None
    }

    pub fn update_scene_summary(&mut self, summary: String) {
        self.scene_summary = Some(summary);
        self.updated_at = Utc::now();
    }

    pub fn update_scene_context(
        &mut self,
        location: Option<String>,
        time: Option<String>,
        summary: Option<String>,
    ) {
        self.current_location = location;
        self.scene_time = time;
        if let Some(s) = summary {
            self.scene_summary = Some(s);
        }
        self.updated_at = Utc::now();
    }
}
