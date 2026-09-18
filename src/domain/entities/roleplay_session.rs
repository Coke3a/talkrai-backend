use chrono::{DateTime, Utc};

use crate::domain::error::DomainError;
use crate::domain::value_objects::{
    CharacterId, CharacterMood, RelationshipLevel, RelationshipThresholds, SceneId, SessionId,
    SessionStatus, UserId,
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
    context_version: i64,
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
            context_version: 0,
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
            context_version: 0,
            current_location,
            scene_time,
            scene_summary,
            created_at,
            updated_at,
        }
    }

    pub fn with_context_version(mut self, version: i64) -> Self {
        self.context_version = version;
        self
    }
    pub fn context_version(&self) -> i64 {
        self.context_version
    }
    pub fn restore_relationship_context(&mut self, level: RelationshipLevel, count: i32) {
        self.relationship_level = level;
        self.message_count = count;
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
    pub fn increment_messages(
        &mut self,
        thresholds: &RelationshipThresholds,
    ) -> Option<RelationshipLevel> {
        self.message_count += 1;
        self.updated_at = Utc::now();

        let threshold = match self.relationship_level {
            RelationshipLevel::Stranger => Some(thresholds.acquaintance_at()),
            RelationshipLevel::Acquaintance => Some(thresholds.friend_at()),
            RelationshipLevel::Friend => Some(thresholds.close_friend_at()),
            RelationshipLevel::CloseFriend => None,
        };

        if let Some(threshold) = threshold {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{CharacterId, SceneId, SessionId, UserId};

    fn default_thresholds() -> RelationshipThresholds {
        RelationshipThresholds::new(20, 50, 100).unwrap()
    }

    fn session_at(level: RelationshipLevel, message_count: i32) -> RoleplaySession {
        RoleplaySession::from_existing(
            SessionId::new(),
            UserId::new(),
            CharacterId::new(),
            SceneId::new(),
            SessionStatus::Active,
            CharacterMood::Neutral,
            level,
            message_count,
            None,
            None,
            None,
            Utc::now(),
            Utc::now(),
        )
    }

    #[test]
    fn no_level_up_before_threshold() {
        let thresholds = default_thresholds();
        let mut session = session_at(RelationshipLevel::Stranger, 18);
        assert!(session.increment_messages(&thresholds).is_none());
        assert_eq!(session.message_count(), 19);
    }

    #[test]
    fn level_up_at_threshold() {
        let thresholds = default_thresholds();
        let mut session = session_at(RelationshipLevel::Stranger, 19);
        let result = session.increment_messages(&thresholds);
        assert_eq!(result, Some(RelationshipLevel::Acquaintance));
        assert_eq!(
            *session.relationship_level(),
            RelationshipLevel::Acquaintance
        );
    }

    #[test]
    fn acquaintance_to_friend_at_threshold() {
        let thresholds = default_thresholds();
        let mut session = session_at(RelationshipLevel::Acquaintance, 49);
        let result = session.increment_messages(&thresholds);
        assert_eq!(result, Some(RelationshipLevel::Friend));
    }

    #[test]
    fn friend_to_close_friend_at_threshold() {
        let thresholds = default_thresholds();
        let mut session = session_at(RelationshipLevel::Friend, 99);
        let result = session.increment_messages(&thresholds);
        assert_eq!(result, Some(RelationshipLevel::CloseFriend));
    }

    #[test]
    fn close_friend_never_levels_up() {
        let thresholds = default_thresholds();
        let mut session = session_at(RelationshipLevel::CloseFriend, 200);
        assert!(session.increment_messages(&thresholds).is_none());
        assert_eq!(session.message_count(), 201);
    }

    #[test]
    fn custom_thresholds_work() {
        let thresholds = RelationshipThresholds::new(5, 10, 15).unwrap();
        let mut session = session_at(RelationshipLevel::Stranger, 4);
        let result = session.increment_messages(&thresholds);
        assert_eq!(result, Some(RelationshipLevel::Acquaintance));
    }
}
