use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::entities::AnalyticsEvent;
use crate::domain::repositories::{AnalyticsEventRepository, RepoError};
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::analytics_events;

use super::error_mapping::{map_diesel_error, map_pool_error};

#[derive(Insertable)]
#[diesel(table_name = analytics_events)]
struct NewAnalyticsEventRow {
    user_id: Uuid,
    event_name: String,
    page: Option<String>,
    properties: Option<serde_json::Value>,
    client_event_id: Uuid,
    occurred_at: DateTime<Utc>,
}

impl NewAnalyticsEventRow {
    fn from_entity(e: &AnalyticsEvent) -> Self {
        Self {
            user_id: *e.user_id().as_uuid(),
            event_name: e.event_name().to_string(),
            page: e.page().map(|p| p.as_str().to_string()),
            properties: e.properties().cloned(),
            client_event_id: *e.client_event_id(),
            occurred_at: *e.occurred_at(),
        }
    }
}

pub struct AnalyticsEventPostgres {
    pool: Arc<PgPool>,
}

impl AnalyticsEventPostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AnalyticsEventRepository for AnalyticsEventPostgres {
    async fn record_batch(&self, events: &[AnalyticsEvent]) -> Result<u64, RepoError> {
        if events.is_empty() {
            return Ok(0); // nothing to insert; avoids an empty-VALUES query
        }

        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let rows: Vec<NewAnalyticsEventRow> = events
            .iter()
            .map(NewAnalyticsEventRow::from_entity)
            .collect();

        let inserted = diesel::insert_into(analytics_events::table)
            .values(&rows)
            .on_conflict(analytics_events::client_event_id)
            .do_nothing()
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("analytics_event.record_batch", e))?;

        Ok(inserted as u64)
    }
}
