use async_trait::async_trait;
use chrono::NaiveDate;

use crate::domain::entities::User;
use crate::domain::repositories::RepoError;
use crate::domain::value_objects::UserId;

/// A user selected for an evening re-engagement reminder (spec §C.5).
#[derive(Debug, Clone)]
pub struct ReengagementTarget {
    pub user_id: UserId,
    pub line_user_id: String,
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, RepoError>;
    async fn find_by_line_user_id(&self, line_user_id: &str) -> Result<Option<User>, RepoError>;
    async fn upsert(&self, user: &User) -> Result<(), RepoError>;
    async fn update(&self, user: &User) -> Result<(), RepoError>;

    /// Active users who engaged at least once, are idle today, engaged within the window, and have
    /// not been reminded today — ordered most-recently-active first, capped at `batch_cap`.
    /// `today` is the Asia/Bangkok date supplied by the caller (not SQL `CURRENT_DATE`).
    async fn find_reengagement_targets(
        &self,
        today: NaiveDate,
        window_days: i64,
        batch_cap: i64,
    ) -> Result<Vec<ReengagementTarget>, RepoError>;

    /// Mark the given users as reminded on `today` (idempotency anchor for the daily reminder).
    async fn mark_reminded(&self, user_ids: &[UserId], today: NaiveDate) -> Result<(), RepoError>;
}
