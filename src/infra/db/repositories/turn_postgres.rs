use super::web_data_postgres::{checked, JsonRow};
use crate::{
    domain::web::{TurnRepository, WebError},
    infra::db::postgres_connection::PgPool,
};
use async_trait::async_trait;
use diesel::{
    sql_query,
    sql_types::{Jsonb, Text, Uuid as SqlUuid},
    QueryableByName,
};
use diesel_async::RunQueryDsl;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;
pub struct TurnPostgres {
    pub pool: Arc<PgPool>,
}
#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
}
#[async_trait]
impl TurnRepository for TurnPostgres {
    async fn admit(
        &self,
        owner: Uuid,
        session: Uuid,
        origin: &str,
        kind: &str,
        key: &str,
        input: Value,
    ) -> Result<Value, WebError> {
        let mut conn = self.pool.get().await.map_err(anyhow::Error::from)?;
        let row = sql_query("SELECT admit_turn($1,$2,$3,$4,$5,$6) AS value")
            .bind::<SqlUuid, _>(owner)
            .bind::<SqlUuid, _>(session)
            .bind::<Text, _>(origin)
            .bind::<Text, _>(kind)
            .bind::<Text, _>(key)
            .bind::<Jsonb, _>(input)
            .get_result::<JsonRow>(&mut conn)
            .await
            .map_err(anyhow::Error::from)?;
        checked(row.value)
    }
    async fn pending(&self) -> Result<Vec<Uuid>, WebError> {
        let mut conn = self.pool.get().await.map_err(anyhow::Error::from)?;
        let rows=sql_query("SELECT id FROM jobs WHERE context_version IS NOT NULL AND (status='pending' OR (status='processing' AND lease_expires_at<now())) ORDER BY created_at LIMIT 20").load::<IdRow>(&mut conn).await.map_err(anyhow::Error::from)?;
        Ok(rows.into_iter().map(|r| r.id).collect())
    }
    async fn claim(&self, id: Uuid) -> Result<Option<Value>, WebError> {
        let mut conn = self.pool.get().await.map_err(anyhow::Error::from)?;
        let row=sql_query("UPDATE jobs SET status='processing',lease_token=gen_random_uuid(),lease_expires_at=now()+interval '5 minutes',locked_at=now(),attempts=attempts+1 WHERE id=$1 AND context_version IS NOT NULL AND (status='pending' OR (status='processing' AND lease_expires_at<now())) RETURNING to_jsonb(jobs) AS value").bind::<SqlUuid,_>(id).get_results::<JsonRow>(&mut conn).await.map_err(anyhow::Error::from)?.pop();
        Ok(row.and_then(|r| r.value))
    }
    async fn save_input(&self, id: Uuid, lease: Uuid, input: Value) -> Result<bool, WebError> {
        let mut conn = self.pool.get().await.map_err(anyhow::Error::from)?;
        Ok(sql_query("UPDATE jobs SET input_snapshot=$3 WHERE id=$1 AND lease_token=$2 AND status='processing' AND lease_expires_at>now() AND input_snapshot IS NULL").bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(lease).bind::<Jsonb,_>(input).execute(&mut conn).await.map_err(anyhow::Error::from)?==1)
    }
    async fn settle(&self, id: Uuid, lease: Uuid, response: Value) -> Result<Value, WebError> {
        let mut conn = self.pool.get().await.map_err(anyhow::Error::from)?;
        let row = sql_query("SELECT settle_turn($1,$2,$3) AS value")
            .bind::<SqlUuid, _>(id)
            .bind::<SqlUuid, _>(lease)
            .bind::<Jsonb, _>(response)
            .get_result::<JsonRow>(&mut conn)
            .await
            .map_err(anyhow::Error::from)?;
        checked(row.value)
    }
    async fn fail(&self, id: Uuid, lease: Uuid) -> Result<(), WebError> {
        let mut conn = self.pool.get().await.map_err(anyhow::Error::from)?;
        sql_query("SELECT fail_turn($1,$2)")
            .bind::<SqlUuid, _>(id)
            .bind::<SqlUuid, _>(lease)
            .execute(&mut conn)
            .await
            .map_err(anyhow::Error::from)?;
        Ok(())
    }
}
