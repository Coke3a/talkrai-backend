use async_trait::async_trait;

use crate::domain::entities::User;
use crate::domain::repositories::RepoError;
use crate::domain::value_objects::UserId;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, RepoError>;
    async fn find_by_line_user_id(&self, line_user_id: &str) -> Result<Option<User>, RepoError>;
    async fn upsert(&self, user: &User) -> Result<(), RepoError>;
}
