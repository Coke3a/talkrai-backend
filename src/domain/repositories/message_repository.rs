use async_trait::async_trait;

use crate::domain::entities::Message;
use crate::domain::repositories::RepoError;
use crate::domain::value_objects::SessionId;

#[async_trait]
pub trait MessageRepository: Send + Sync {
    async fn create(&self, message: &Message) -> Result<(), RepoError>;
    async fn create_many(&self, messages: &[Message]) -> Result<(), RepoError>;
    async fn find_by_session_id(
        &self,
        session_id: &SessionId,
        limit: i64,
    ) -> Result<Vec<Message>, RepoError>;
}
