use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::repositories::{
    AppConfigRepository, CharacterRepository, RoleplaySessionRepository, SceneRepository,
    UserRepository,
};
use crate::domain::value_objects::{RelationshipProgress, RelationshipThresholds};
use crate::usecases::liff::require_active_user::require_active_user;
use crate::usecases::UsecaseError;

pub struct GetCurrentSessionInput {
    pub line_user_id: String,
}

pub struct CurrentSessionData {
    pub id: Uuid,
    pub character_name: String,
    pub character_avatar_url: Option<String>,
    pub scene_name: String,
    pub scene_image_url: Option<String>,
    pub current_location: Option<String>,
    pub scene_time: Option<String>,
    pub mood: String,
    pub relationship_level: String,
    pub message_count: i32,
    pub scene_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Progress within the current relationship segment toward the next level (0.0..=1.0).
    pub relationship_progress: f32,
    /// Thai label of the next level, or `None` at the top level.
    pub next_level_label: Option<String>,
    /// Messages remaining to the next level, or `None` at the top level.
    pub messages_to_next: Option<i32>,
}

pub struct GetCurrentSessionOutput {
    pub session: Option<CurrentSessionData>,
}

pub struct GetCurrentSessionUseCase {
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    character_repo: Arc<dyn CharacterRepository>,
    scene_repo: Arc<dyn SceneRepository>,
    config_repo: Arc<dyn AppConfigRepository>,
}

impl GetCurrentSessionUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn RoleplaySessionRepository>,
        character_repo: Arc<dyn CharacterRepository>,
        scene_repo: Arc<dyn SceneRepository>,
        config_repo: Arc<dyn AppConfigRepository>,
    ) -> Self {
        Self {
            user_repo,
            session_repo,
            character_repo,
            scene_repo,
            config_repo,
        }
    }

    /// Reads the relationship thresholds from `app_config`. Unlike the message pipeline (where a
    /// missing threshold means the game is broken and erroring is correct), this is a read-only
    /// status display: the meter is an enhancement, so a config gap degrades to the seeded
    /// defaults (8/24/70) rather than 500-ing the whole status page.
    async fn load_thresholds(&self) -> Result<RelationshipThresholds, UsecaseError> {
        const DEFAULTS: (u32, u32, u32) = (8, 24, 70);
        let map = self
            .config_repo
            .get_many(&[
                "relationship_threshold_acquaintance",
                "relationship_threshold_friend",
                "relationship_threshold_close_friend",
            ])
            .await?;
        let read = |key: &str, default: u32| -> u32 {
            match map.get(key).and_then(|v| v.parse::<u32>().ok()) {
                Some(v) => v,
                None => {
                    tracing::warn!(key, "Missing/invalid relationship threshold; using default");
                    default
                }
            }
        };
        let (a, f, cf) = (
            read("relationship_threshold_acquaintance", DEFAULTS.0),
            read("relationship_threshold_friend", DEFAULTS.1),
            read("relationship_threshold_close_friend", DEFAULTS.2),
        );
        // `new` only rejects non-monotonic thresholds (possible from a partial misconfig); fall
        // back to the known-valid defaults rather than failing the request.
        match RelationshipThresholds::new(a, f, cf) {
            Ok(t) => Ok(t),
            Err(_) => {
                tracing::warn!(
                    "Non-monotonic relationship thresholds in app_config; using defaults"
                );
                RelationshipThresholds::new(DEFAULTS.0, DEFAULTS.1, DEFAULTS.2).map_err(|e| {
                    UsecaseError::Infra(anyhow::anyhow!("default thresholds invalid: {e}"))
                })
            }
        }
    }

    pub async fn execute(
        &self,
        input: GetCurrentSessionInput,
    ) -> Result<GetCurrentSessionOutput, UsecaseError> {
        let user = require_active_user(&*self.user_repo, &input.line_user_id).await?;

        let session = self.session_repo.find_active_by_user_id(user.id()).await?;

        let session = match session {
            Some(s) => s,
            None => return Ok(GetCurrentSessionOutput { session: None }),
        };

        let character = self
            .character_repo
            .find_by_id(session.character_id())
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Character not found".to_string()))?;

        let scene = self
            .scene_repo
            .find_by_id(session.scene_id())
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Scene not found".to_string()))?;

        let thresholds = self.load_thresholds().await?;
        let progress = RelationshipProgress::compute(
            session.relationship_level(),
            session.message_count(),
            &thresholds,
        );

        Ok(GetCurrentSessionOutput {
            session: Some(CurrentSessionData {
                id: *session.id().as_uuid(),
                character_name: character.name().as_str().to_string(),
                character_avatar_url: character.avatar_url().map(|s| s.to_string()),
                scene_name: scene.name().as_str().to_string(),
                scene_image_url: scene.image_url().map(|s| s.to_string()),
                current_location: session.current_location().map(|s| s.to_string()),
                scene_time: session.scene_time().map(|s| s.to_string()),
                mood: session.mood().as_str().to_string(),
                relationship_level: session.relationship_level().as_str().to_string(),
                message_count: session.message_count(),
                scene_summary: session.scene_summary().map(|s| s.to_string()),
                created_at: *session.created_at(),
                relationship_progress: progress.fraction,
                next_level_label: progress.next_level_label,
                messages_to_next: progress.messages_to_next,
            }),
        })
    }
}
