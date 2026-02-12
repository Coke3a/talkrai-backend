use async_trait::async_trait;

use crate::domain::entities::Character;
use crate::domain::repositories::RepoError;
use crate::domain::value_objects::CharacterId;

#[async_trait]
pub trait CharacterRepository: Send + Sync {
    async fn find_by_id(&self, id: &CharacterId) -> Result<Option<Character>, RepoError>;
    async fn find_active(&self) -> Result<Vec<Character>, RepoError>;
}
