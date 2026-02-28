use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::domain::repositories::{AppConfigRepository, RepoError};

const CACHE_TTL_SECS: u64 = 60;

struct CacheEntry {
    value: Option<String>,
    fetched_at: Instant,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.fetched_at.elapsed().as_secs() >= CACHE_TTL_SECS
    }
}

pub struct CachedAppConfigRepository {
    inner: Arc<dyn AppConfigRepository>,
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

impl CachedAppConfigRepository {
    pub fn new(inner: Arc<dyn AppConfigRepository>) -> Self {
        Self {
            inner,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl AppConfigRepository for CachedAppConfigRepository {
    async fn get(&self, key: &str) -> Result<Option<String>, RepoError> {
        // Check cache (read lock)
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(key) {
                if !entry.is_expired() {
                    return Ok(entry.value.clone());
                }
            }
        }

        // Cache miss or expired → query DB
        let value = self.inner.get(key).await?;

        // Store in cache (write lock)
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                key.to_string(),
                CacheEntry {
                    value: value.clone(),
                    fetched_at: Instant::now(),
                },
            );
        }

        Ok(value)
    }

    async fn get_many(&self, keys: &[&str]) -> Result<HashMap<String, String>, RepoError> {
        let mut result = HashMap::new();
        let mut missing_keys = Vec::new();

        // Check cache for each key (read lock)
        {
            let cache = self.cache.read().await;
            for &key in keys {
                if let Some(entry) = cache.get(key) {
                    if !entry.is_expired() {
                        if let Some(ref v) = entry.value {
                            result.insert(key.to_string(), v.clone());
                        }
                        continue;
                    }
                }
                missing_keys.push(key);
            }
        }

        if missing_keys.is_empty() {
            return Ok(result);
        }

        // Fetch missing keys from DB
        let fetched = self.inner.get_many(&missing_keys).await?;

        // Store fetched values in cache (write lock)
        {
            let mut cache = self.cache.write().await;
            for key in &missing_keys {
                let value = fetched.get(*key).cloned();
                cache.insert(
                    key.to_string(),
                    CacheEntry {
                        value,
                        fetched_at: Instant::now(),
                    },
                );
            }
        }

        result.extend(fetched);
        Ok(result)
    }
}
