use super::web_data_postgres::{checked, JsonRow};
use crate::{
    domain::web::{WebError, WebPaymentRepository},
    infra::db::postgres_connection::PgPool,
};
use async_trait::async_trait;
use diesel::{
    sql_query,
    sql_types::{Text, Uuid as SqlUuid},
};
use diesel_async::RunQueryDsl;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;
pub struct WebPaymentPostgres {
    pub pool: Arc<PgPool>,
}
#[async_trait]
impl WebPaymentRepository for WebPaymentPostgres {
    async fn create(&self, owner: Uuid, key: &str, package: &str) -> Result<Value, WebError> {
        let mut conn = self.pool.get().await.map_err(anyhow::Error::from)?;
        checked(
            sql_query("SELECT web_payment_order($1,$2,$3) AS value")
                .bind::<SqlUuid, _>(owner)
                .bind::<Text, _>(key)
                .bind::<Text, _>(package)
                .get_result::<JsonRow>(&mut conn)
                .await
                .map_err(anyhow::Error::from)?
                .value,
        )
    }
    async fn attach_link(&self, id: Uuid, link_id: &str, url: &str) -> Result<(), WebError> {
        let mut conn = self.pool.get().await.map_err(anyhow::Error::from)?;
        sql_query("UPDATE payment_orders SET beam_payment_link_id=$2,payment_url=$3,updated_at=now() WHERE id=$1 AND beam_payment_link_id=''").bind::<SqlUuid,_>(id).bind::<Text,_>(link_id).bind::<Text,_>(url).execute(&mut conn).await.map_err(anyhow::Error::from)?;
        Ok(())
    }
}
