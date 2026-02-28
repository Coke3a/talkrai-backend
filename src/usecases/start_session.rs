use std::sync::Arc;

use uuid::Uuid;

use crate::domain::entities::{Message, RoleplaySession};
use crate::domain::repositories::{
    CharacterRepository, MessageRepository, RoleplaySessionRepository, SceneRepository,
    UserRepository,
};
use crate::domain::services::line_client::{LineClient, LineMessage};
use crate::domain::value_objects::MessageRole;
use crate::usecases::UsecaseError;

pub struct StartSessionInput {
    pub line_user_id: String,
    pub scene_id: Uuid,
    pub rich_menu_b_id: String,
}

pub struct StartSessionOutput {
    pub session_id: Uuid,
}

pub struct StartSessionUseCase {
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    scene_repo: Arc<dyn SceneRepository>,
    character_repo: Arc<dyn CharacterRepository>,
    message_repo: Arc<dyn MessageRepository>,
    line_client: Arc<dyn LineClient>,
}

impl StartSessionUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn RoleplaySessionRepository>,
        scene_repo: Arc<dyn SceneRepository>,
        character_repo: Arc<dyn CharacterRepository>,
        message_repo: Arc<dyn MessageRepository>,
        line_client: Arc<dyn LineClient>,
    ) -> Self {
        Self {
            user_repo,
            session_repo,
            scene_repo,
            character_repo,
            message_repo,
            line_client,
        }
    }

    pub async fn execute(
        &self,
        input: StartSessionInput,
    ) -> Result<StartSessionOutput, UsecaseError> {
        // 1. Find user
        let user = self
            .user_repo
            .find_by_line_user_id(&input.line_user_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("User not found".into()))?;

        // 2. Auto-accept terms if not yet accepted
        let mut user = user;
        if !user.has_accepted_terms() {
            user.accept_terms()
                .map_err(|e| UsecaseError::Validation(format!("Failed to accept terms: {}", e)))?;
            self.user_repo.update(&user).await?;
            tracing::info!(
                user_id = %user.id().as_uuid(),
                "Auto-accepted terms during session start"
            );
        }

        // 3. Fetch scene
        let scene_id = crate::domain::value_objects::SceneId::from_uuid(input.scene_id);
        let scene = self
            .scene_repo
            .find_by_id(&scene_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Scene not found".into()))?;

        if !scene.is_active() {
            return Err(UsecaseError::Validation("Scene is not active".into()));
        }

        // 4. Fetch character
        let character = self
            .character_repo
            .find_by_id(scene.character_id())
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Character not found".into()))?;

        // 5. End existing active session if any
        if let Some(mut active_session) =
            self.session_repo.find_active_by_user_id(user.id()).await?
        {
            active_session.end()?;
            self.session_repo.update(&active_session).await?;
            tracing::info!(
                session_id = %active_session.id().as_uuid(),
                "Ended previous active session before starting new one"
            );
        }

        // 6. Create new session
        let session = RoleplaySession::new(
            user.id().clone(),
            character.id().clone(),
            scene.id().clone(),
            scene.start_relationship_level().clone(),
            scene.start_mood().clone(),
        );
        self.session_repo.create(&session).await?;

        // 7. Create opening messages (narrator + character dialogue)
        let narrator_message = Message::new(
            session.id().clone(),
            MessageRole::Character,
            scene.opening_narrator().to_string(),
            None,
        );
        let dialogue_message = Message::new(
            session.id().clone(),
            MessageRole::Character,
            scene.opening_dialogue().to_string(),
            None,
        );
        self.message_repo
            .create_many(&[narrator_message, dialogue_message])
            .await?;

        // 8. Push opening messages to LINE
        let narrator_sender_name = "ผู้บรรยาย".to_string();
        let narrator_sender_icon = String::new();
        let character_sender_name = character.name().as_str().to_string();
        let character_sender_icon = character.avatar_url().unwrap_or_default().to_string();

        let line_messages = vec![
            LineMessage::Text {
                text: scene.opening_narrator().to_string(),
                sender_name: narrator_sender_name,
                sender_icon_url: narrator_sender_icon,
            },
            LineMessage::Text {
                text: scene.opening_dialogue().to_string(),
                sender_name: character_sender_name,
                sender_icon_url: character_sender_icon,
            },
        ];

        if let Err(e) = self
            .line_client
            .push_messages(&input.line_user_id, line_messages)
            .await
        {
            tracing::warn!(
                error = %e,
                line_user_id = %input.line_user_id,
                "Failed to push opening messages"
            );
        }

        // 9. Switch rich menu to B (best-effort)
        if let Err(e) = self
            .line_client
            .link_rich_menu(&input.line_user_id, &input.rich_menu_b_id)
            .await
        {
            tracing::warn!(
                error = %e,
                line_user_id = %input.line_user_id,
                "Failed to link rich menu B after starting session"
            );
        }

        Ok(StartSessionOutput {
            session_id: *session.id().as_uuid(),
        })
    }
}
