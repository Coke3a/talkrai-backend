use crate::domain::web::{AuthFlow, Identity, WebError, WebPrincipal, WebRepository};
use crate::infra::db::postgres_connection::PgPool;
use async_trait::async_trait;
use diesel::{
    sql_query,
    sql_types::{Jsonb, Text},
    QueryableByName,
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub struct WebAuthPostgres {
    pool: Arc<PgPool>,
}
impl WebAuthPostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}
#[derive(QueryableByName)]
struct JsonRow {
    #[diesel(sql_type=Jsonb)]
    value: Value,
}
fn internal(error: impl Into<anyhow::Error>) -> WebError {
    WebError::Internal(error.into())
}

#[async_trait]
impl WebRepository for WebAuthPostgres {
    async fn principal(&self, token_hash: &str) -> Result<WebPrincipal, WebError> {
        let mut conn = self.pool.get().await.map_err(internal)?;
        let row = sql_query("UPDATE web_auth_sessions SET last_seen_at=now() WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at>now() AND last_seen_at>now()-interval '7 days' RETURNING jsonb_build_object('user_id',user_id,'csrf_token',csrf_token,'token_hash',token_hash) AS value")
   .bind::<Text,_>(token_hash).get_results::<JsonRow>(&mut conn).await.map_err(internal)?.pop().ok_or(WebError::Rejected("AUTH_EXPIRED"))?;
        serde_json::from_value(row.value).map_err(internal)
    }
    async fn save_flow(&self, flow: &AuthFlow) -> Result<(), WebError> {
        let mut conn = self.pool.get().await.map_err(internal)?;
        sql_query("INSERT INTO auth_flows(acquisition_source,state_hash,browser_hash,provider,nonce,pkce_verifier,return_to,link_user_id,link_session_hash,expires_at) SELECT $1->>'acquisition_source',$1->>'state_hash',$1->>'browser_hash',$1->>'provider',$1->>'nonce',$1->>'pkce_verifier',$1->>'return_to',($1->>'link_user_id')::uuid,$1->>'link_session_hash',now()+interval '10 minutes'")
   .bind::<Jsonb,_>(serde_json::to_value(flow).map_err(internal)?).execute(&mut conn).await.map_err(internal)?;
        Ok(())
    }
    async fn consume_flow(
        &self,
        state_hash: &str,
        browser_hash: &str,
        provider: &str,
    ) -> Result<AuthFlow, WebError> {
        let mut conn = self.pool.get().await.map_err(internal)?;
        let row=sql_query("DELETE FROM auth_flows WHERE state_hash=$1 AND browser_hash=$2 AND provider=$3 AND expires_at>now() RETURNING to_jsonb(auth_flows) AS value")
   .bind::<Text,_>(state_hash).bind::<Text,_>(browser_hash).bind::<Text,_>(provider).get_results::<JsonRow>(&mut conn).await.map_err(internal)?.pop().ok_or(WebError::Rejected("INVALID_AUTH_FLOW"))?;
        serde_json::from_value(row.value).map_err(internal)
    }
    async fn login(
        &self,
        identity: &Identity,
        flow: &AuthFlow,
        token_hash: &str,
        csrf: &str,
        allow_registration: bool,
    ) -> Result<Uuid, WebError> {
        let mut conn = self.pool.get().await.map_err(internal)?;
        let input = json!({"identity":identity,"flow":flow,"token_hash":token_hash,"csrf":csrf,"allow_registration":allow_registration});
        conn.transaction::<Uuid, anyhow::Error, _>(|conn| {
            Box::pin(async move {
                let row = sql_query(include_str!("web_login.sql"))
                    .bind::<Jsonb, _>(&input)
                    .get_result::<JsonRow>(conn)
                    .await?;
                if let Some(code) = row.value.get("error").and_then(Value::as_str) {
                    anyhow::bail!("{}", code);
                }
                Ok(serde_json::from_value(row.value["user_id"].clone())?)
            })
        })
        .await
        .map_err(|e| match e.to_string().as_str() {
            "IDENTITY_ALREADY_LINKED" => WebError::Rejected("IDENTITY_ALREADY_LINKED"),
            "ACCOUNT_SUSPENDED" => WebError::Rejected("ACCOUNT_SUSPENDED"),
            "AUTH_EXPIRED" => WebError::Rejected("AUTH_EXPIRED"),
            "ADMISSIONS_DISABLED" => WebError::Rejected("ADMISSIONS_DISABLED"),
            _ => internal(e),
        })
    }
    async fn logout(&self, token_hash: &str) -> Result<(), WebError> {
        let mut conn = self.pool.get().await.map_err(internal)?;
        sql_query("UPDATE web_auth_sessions SET revoked_at=now() WHERE token_hash=$1")
            .bind::<Text, _>(token_hash)
            .execute(&mut conn)
            .await
            .map_err(internal)?;
        Ok(())
    }
}
