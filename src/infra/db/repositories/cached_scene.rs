use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::domain::entities::Scene;
use crate::domain::repositories::{RepoError, SceneRepository};
use crate::domain::value_objects::{CharacterId, SceneId};

use super::ttl_cache::TtlCache;

/// Caches `find_by_id` (the per-message hot-path read) with a TTL; listing lookups
/// are served fresh. See `CachedCharacterRepository` for rationale.
pub struct CachedSceneRepository {
    inner: Arc<dyn SceneRepository>,
    cache: TtlCache<SceneId, Scene>,
}

impl CachedSceneRepository {
    pub fn new(inner: Arc<dyn SceneRepository>, ttl: Duration) -> Self {
        Self {
            inner,
            cache: TtlCache::new(ttl),
        }
    }
}

#[async_trait]
impl SceneRepository for CachedSceneRepository {
    async fn find_by_id(&self, id: &SceneId) -> Result<Option<Scene>, RepoError> {
        if let Some(cached) = self.cache.get(id).await {
            return Ok(Some(cached));
        }
        let result = self.inner.find_by_id(id).await?;
        if let Some(scene) = &result {
            self.cache.insert(id.clone(), scene.clone()).await;
        }
        Ok(result)
    }

    async fn find_by_character_id(
        &self,
        character_id: &CharacterId,
    ) -> Result<Vec<Scene>, RepoError> {
        self.inner.find_by_character_id(character_id).await
    }

    async fn find_default_by_character_id(
        &self,
        character_id: &CharacterId,
    ) -> Result<Option<Scene>, RepoError> {
        self.inner.find_default_by_character_id(character_id).await
    }

    async fn find_all_active(&self) -> Result<Vec<Scene>, RepoError> {
        self.inner.find_all_active().await
    }
}
