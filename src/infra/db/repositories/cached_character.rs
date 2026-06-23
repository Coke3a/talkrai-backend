use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::domain::entities::Character;
use crate::domain::repositories::{CharacterRepository, RepoError};
use crate::domain::value_objects::CharacterId;

use super::ttl_cache::TtlCache;

/// Caches `find_by_id` (the per-message hot-path read) with a TTL. Characters are
/// part of a rarely-changing catalog, so serving a slightly stale copy is safe and
/// removes a DB round-trip — and a pool connection checkout — from every roleplay
/// turn. Edits to a character take effect within one TTL.
pub struct CachedCharacterRepository {
    inner: Arc<dyn CharacterRepository>,
    cache: TtlCache<CharacterId, Character>,
}

impl CachedCharacterRepository {
    pub fn new(inner: Arc<dyn CharacterRepository>, ttl: Duration) -> Self {
        Self {
            inner,
            cache: TtlCache::new(ttl),
        }
    }
}

#[async_trait]
impl CharacterRepository for CachedCharacterRepository {
    async fn find_by_id(&self, id: &CharacterId) -> Result<Option<Character>, RepoError> {
        if let Some(cached) = self.cache.get(id).await {
            return Ok(Some(cached));
        }
        let result = self.inner.find_by_id(id).await?;
        if let Some(character) = &result {
            self.cache.insert(id.clone(), character.clone()).await;
        }
        Ok(result)
    }

    async fn find_active(&self) -> Result<Vec<Character>, RepoError> {
        // Listing is not on the hot path; always serve fresh.
        self.inner.find_active().await
    }
}
