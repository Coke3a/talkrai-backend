use std::sync::Arc;

use uuid::Uuid;

use crate::domain::repositories::{RoleplaySessionRepository, UserRepository};
use crate::domain::services::line_client::LineClient;
use crate::usecases::UsecaseError;

pub struct EndSessionInput {
    pub line_user_id: String,
    pub rich_menu_a_id: String,
}

pub struct EndSessionOutput {
    pub session_id: Uuid,
}

pub struct EndSessionUseCase {
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    line_client: Arc<dyn LineClient>,
}

impl EndSessionUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn RoleplaySessionRepository>,
        line_client: Arc<dyn LineClient>,
    ) -> Self {
        Self {
            user_repo,
            session_repo,
            line_client,
        }
    }

    pub async fn execute(&self, input: EndSessionInput) -> Result<EndSessionOutput, UsecaseError> {
        // 1. Find user
        let user = self
            .user_repo
            .find_by_line_user_id(&input.line_user_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("User not found".into()))?;

        // 2. Find active session
        let mut session = self
            .session_repo
            .find_active_by_user_id(user.id())
            .await?
            .ok_or_else(|| UsecaseError::NotFound("No active session found".into()))?;

        // 3. End session (domain state transition: Active → Ended)
        session.end()?;
        self.session_repo.update(&session).await?;

        // 4. Switch rich menu back to A (best-effort)
        if let Err(e) = self
            .line_client
            .link_rich_menu(&input.line_user_id, &input.rich_menu_a_id)
            .await
        {
            tracing::warn!(
                error = %e,
                line_user_id = %input.line_user_id,
                "Failed to link rich menu A after ending session"
            );
        }

        Ok(EndSessionOutput {
            session_id: *session.id().as_uuid(),
        })
    }
}
