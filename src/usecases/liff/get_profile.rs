use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};

use crate::domain::repositories::{
    AppConfigRepository, MessageRepository, RoleplaySessionRepository, UserRepository,
};
use crate::domain::value_objects::CheckInConfig;
use crate::usecases::liff::require_active_user::require_active_user;
use crate::usecases::UsecaseError;

pub struct GetProfileInput {
    pub line_user_id: String,
    pub today: NaiveDate,
}

pub struct GetProfileOutput {
    pub created_at: DateTime<Utc>,
    pub total_sessions: i64,
    pub total_messages: i64,
    pub longest_streak: i32,
    pub current_streak: i32,
    pub checked_in_today: bool,
    pub today_cycle_day: i32,
    pub today_credits: i32,
    pub days_to_chest: i32,
    pub weekly_credits: Vec<i32>,
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

        // Weekly check-in display state as of `today`. A missing/malformed config row falls back to
        // the default weekly table rather than failing the profile load.
        let raw = self.config_repo.get("daily_checkin_weekly_credits").await?;
        let cfg = CheckInConfig::from_config_value(raw.as_deref());
        let status = user.check_in_status(input.today, &cfg);

        Ok(GetProfileOutput {
            created_at: *user.created_at(),
            total_sessions,
            total_messages,
            longest_streak: user.longest_streak(),
            current_streak: status.current_streak,
            checked_in_today: status.checked_in_today,
            today_cycle_day: status.today_cycle_day,
            today_credits: status.today_credits,
            days_to_chest: status.days_to_chest,
            weekly_credits: cfg.weekly_credits,
        })
    }
}
