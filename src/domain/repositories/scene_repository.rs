use async_trait::async_trait;

use crate::domain::entities::Scene;
use crate::domain::repositories::RepoError;
use crate::domain::value_objects::{CharacterId, SceneId};

#[async_trait]
pub trait SceneRepository: Send + Sync {
    async fn find_by_id(&self, id: &SceneId) -> Result<Option<Scene>, RepoError>;
    async fn find_by_character_id(
        &self,
        character_id: &CharacterId,
    ) -> Result<Vec<Scene>, RepoError>;
    async fn find_default_by_character_id(
        &self,
        character_id: &CharacterId,
    ) -> Result<Option<Scene>, RepoError>;
    async fn find_all_active(&self) -> Result<Vec<Scene>, RepoError>;
}
