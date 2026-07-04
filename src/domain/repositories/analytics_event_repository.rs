use async_trait::async_trait;

use crate::domain::entities::AnalyticsEvent;
use crate::domain::repositories::RepoError;

#[async_trait]
pub trait AnalyticsEventRepository: Send + Sync {
    /// Batch-insert events. Rows whose `client_event_id` already exists are
    /// skipped (dedup). Returns the count of newly-inserted rows.
    async fn record_batch(&self, events: &[AnalyticsEvent]) -> Result<u64, RepoError>;
}
