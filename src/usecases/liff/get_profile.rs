use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::domain::repositories::{MessageRepository, RoleplaySessionRepository, UserRepository};
use crate::usecases::UsecaseError;

pub struct GetProfileInput {
    pub line_user_id: String,
}

pub struct GetProfileOutput {
    pub created_at: DateTime<Utc>,
    pub total_sessions: i64,
    pub total_messages: i64,
}

pub struct GetProfileUseCase {
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    message_repo: Arc<dyn MessageRepository>,
}

impl GetProfileUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn RoleplaySessionRepository>,
        message_repo: Arc<dyn MessageRepository>,
    ) -> Self {
        Self {
            user_repo,
            session_repo,
            message_repo,
        }
    }

    pub async fn execute(&self, input: GetProfileInput) -> Result<GetProfileOutput, UsecaseError> {
        let user = self
            .user_repo
            .find_by_line_user_id(&input.line_user_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("User not found".to_string()))?;

        let user_id = user.id().clone();

        let (total_sessions, total_messages) = tokio::try_join!(
            self.session_repo.count_by_user_id(&user_id),
            self.message_repo.count_by_user_id(&user_id),
        )?;

        Ok(GetProfileOutput {
            created_at: *user.created_at(),
            total_sessions,
            total_messages,
        })
    }
}
