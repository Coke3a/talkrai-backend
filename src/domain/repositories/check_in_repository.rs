use async_trait::async_trait;

use crate::domain::entities::{DailyCheckIn, UserStreak};
use crate::domain::repositories::RepoError;
use crate::domain::value_objects::UserId;

#[async_trait]
pub trait CheckInRepository: Send + Sync {
    async fn find_streak_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<UserStreak>, RepoError>;
    async fn upsert_streak(&self, streak: &UserStreak) -> Result<(), RepoError>;
    async fn create_check_in(&self, check_in: &DailyCheckIn) -> Result<(), RepoError>;
    async fn find_check_ins_by_user_id(
        &self,
        user_id: &UserId,
        limit: i64,
    ) -> Result<Vec<DailyCheckIn>, RepoError>;
}
