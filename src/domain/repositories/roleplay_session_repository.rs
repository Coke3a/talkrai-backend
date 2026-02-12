use async_trait::async_trait;

use crate::domain::entities::RoleplaySession;
use crate::domain::repositories::RepoError;
use crate::domain::value_objects::{SessionId, UserId};

#[async_trait]
pub trait RoleplaySessionRepository: Send + Sync {
    async fn find_by_id(&self, id: &SessionId) -> Result<Option<RoleplaySession>, RepoError>;
    async fn find_active_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<RoleplaySession>, RepoError>;
    async fn create(&self, session: &RoleplaySession) -> Result<(), RepoError>;
    async fn update(&self, session: &RoleplaySession) -> Result<(), RepoError>;
}
