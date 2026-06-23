use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

/// Process-local TTL cache for read-heavy, rarely-changing catalog data
/// (characters, scenes). Mirrors the approach of `CachedAppConfigRepository` but
/// is generic over key/value. A stale entry yields `None`, so the caller refetches
/// and refreshes it. The key space is the catalog size (small), so entries are not
/// evicted beyond expiry.
pub struct TtlCache<K, V> {
    ttl: Duration,
    entries: RwLock<HashMap<K, (V, Instant)>>,
}

impl<K: Eq + Hash + Clone, V: Clone> TtlCache<K, V> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Return a fresh cached value, or `None` when absent or expired.
    pub async fn get(&self, key: &K) -> Option<V> {
        let entries = self.entries.read().await;
        let (value, fetched_at) = entries.get(key)?;
        (fetched_at.elapsed() < self.ttl).then(|| value.clone())
    }

    pub async fn insert(&self, key: K, value: V) {
        self.entries
            .write()
            .await
            .insert(key, (value, Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_value_within_ttl() {
        let cache: TtlCache<u32, String> = TtlCache::new(Duration::from_secs(60));
        cache.insert(1, "a".to_string()).await;
        assert_eq!(cache.get(&1).await, Some("a".to_string()));
    }

    #[tokio::test]
    async fn misses_unknown_key() {
        let cache: TtlCache<u32, String> = TtlCache::new(Duration::from_secs(60));
        assert_eq!(cache.get(&99).await, None);
    }

    #[tokio::test]
    async fn expires_after_ttl() {
        let cache: TtlCache<u32, String> = TtlCache::new(Duration::from_millis(0));
        cache.insert(1, "a".to_string()).await;
        // TTL of 0 → any elapsed time is already >= ttl → treated as expired.
        assert_eq!(cache.get(&1).await, None);
    }
}
