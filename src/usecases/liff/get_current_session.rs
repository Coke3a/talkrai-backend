use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::repositories::{
    CharacterRepository, RoleplaySessionRepository, SceneRepository, UserRepository,
};
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
}

pub struct GetCurrentSessionOutput {
    pub session: Option<CurrentSessionData>,
}

pub struct GetCurrentSessionUseCase {
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    character_repo: Arc<dyn CharacterRepository>,
    scene_repo: Arc<dyn SceneRepository>,
}

impl GetCurrentSessionUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn RoleplaySessionRepository>,
        character_repo: Arc<dyn CharacterRepository>,
        scene_repo: Arc<dyn SceneRepository>,
    ) -> Self {
        Self {
            user_repo,
            session_repo,
            character_repo,
            scene_repo,
        }
    }

    pub async fn execute(
        &self,
        input: GetCurrentSessionInput,
    ) -> Result<GetCurrentSessionOutput, UsecaseError> {
        let user = self
            .user_repo
            .find_by_line_user_id(&input.line_user_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("User not found".to_string()))?;

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
            }),
        })
    }
}
