use async_trait::async_trait;

use crate::domain::entities::CharacterMemory;
use crate::domain::repositories::RepoError;
use crate::domain::value_objects::{CharacterId, UserId};

#[async_trait]
pub trait CharacterMemoryRepository: Send + Sync {
    async fn find_by_user_and_character(
        &self,
        user_id: &UserId,
        character_id: &CharacterId,
        limit: i64,
    ) -> Result<Vec<CharacterMemory>, RepoError>;

    async fn create(&self, memory: &CharacterMemory) -> Result<(), RepoError>;
    async fn create_many(&self, memories: &[CharacterMemory]) -> Result<(), RepoError>;
    async fn update(&self, memory: &CharacterMemory) -> Result<(), RepoError>;

    /// Delete all memories for a user-character pair
    async fn delete_by_user_and_character(
        &self,
        user_id: &UserId,
        character_id: &CharacterId,
    ) -> Result<u64, RepoError>;
}
