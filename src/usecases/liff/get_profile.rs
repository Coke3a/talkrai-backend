use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::domain::repositories::{
    AppConfigRepository, MessageRepository, RoleplaySessionRepository, UserRepository,
};
use crate::domain::value_objects::CheckInConfig;
use crate::usecases::liff::require_active_user::require_active_user;
use crate::usecases::UsecaseError;

pub struct GetProfileInput {
    pub line_user_id: String,
}

pub struct GetProfileOutput {
    pub created_at: DateTime<Utc>,
    pub total_sessions: i64,
    pub total_messages: i64,
    pub check_in_streak: i32,
    pub longest_streak: i32,
    /// Days until the next milestone bonus, or `None` if no milestone lies ahead.
    pub next_milestone_in: Option<i32>,
}

pub struct GetProfileUseCase {
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    message_repo: Arc<dyn MessageRepository>,
    config_repo: Arc<dyn AppConfigRepository>,
}

impl GetProfileUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn RoleplaySessionRepository>,
        message_repo: Arc<dyn MessageRepository>,
        config_repo: Arc<dyn AppConfigRepository>,
    ) -> Self {
        Self {
            user_repo,
            session_repo,
            message_repo,
            config_repo,
        }
    }

    pub async fn execute(&self, input: GetProfileInput) -> Result<GetProfileOutput, UsecaseError> {
        let user = require_active_user(&*self.user_repo, &input.line_user_id).await?;

        let user_id = user.id().clone();

        let (total_sessions, total_messages) = tokio::try_join!(
            self.session_repo.count_by_user_id(&user_id),
            self.message_repo.count_by_user_id(&user_id),
        )?;

        // Days to the next milestone bonus. Optional UI hint, so a missing/empty config simply
        // yields `None` (no hint) rather than failing the profile load.
        let streak = user.check_in_streak();
        let milestones = self
            .config_repo
            .get("daily_checkin_milestone_bonuses")
            .await?
            .map(|raw| CheckInConfig::parse_milestones(&raw))
            .unwrap_or_default();
        let next_milestone_in = milestones
            .iter()
            .map(|(day, _)| *day)
            .filter(|day| *day > streak)
            .min()
            .map(|day| day - streak);

        Ok(GetProfileOutput {
            created_at: *user.created_at(),
            total_sessions,
            total_messages,
            check_in_streak: streak,
            longest_streak: user.longest_streak(),
            next_milestone_in,
        })
    }
}
