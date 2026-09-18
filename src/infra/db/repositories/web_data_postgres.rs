use crate::{
    domain::web::{StoryMutation, WebDataRepository, WebError, WebRead},
    infra::db::postgres_connection::PgPool,
};
use async_trait::async_trait;
use diesel::{
    sql_query,
    sql_types::{Jsonb, Nullable, Text, Uuid as SqlUuid},
    OptionalExtension, QueryableByName,
};
use diesel_async::RunQueryDsl;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;
pub struct WebDataPostgres {
    pub pool: Arc<PgPool>,
}
#[derive(QueryableByName)]
pub struct JsonRow {
    #[diesel(sql_type=Nullable<Jsonb>)]
    pub value: Option<Value>,
}
pub fn checked(value: Option<Value>) -> Result<Value, WebError> {
    let value = value
        .filter(|v| !v.is_null())
        .ok_or(WebError::Rejected("NOT_FOUND"))?;
    if let Some(code) = value
        .get("error")
        .filter(|_| value.get("id").is_none())
        .and_then(Value::as_str)
    {
        return Err(WebError::Rejected(match code {
            "ACCOUNT_SUSPENDED" => "ACCOUNT_SUSPENDED",
            "RATE_LIMITED" => "RATE_LIMITED",
            "TURN_IN_PROGRESS" => "TURN_IN_PROGRESS",
            "STALE_VERSION" => "STALE_VERSION",
            "STALE_LEASE" => "STALE_LEASE",
            "TERMS_REQUIRED" => "TERMS_REQUIRED",
            "SCENE_UNAVAILABLE" => "SCENE_UNAVAILABLE",
            "CHANNEL_MISMATCH" => "CHANNEL_MISMATCH",
            "IDEMPOTENCY_MISMATCH" => "IDEMPOTENCY_MISMATCH",
            "INSUFFICIENT_CREDITS" => "INSUFFICIENT_CREDITS",
            "INVALID_REGENERATION" => "INVALID_REGENERATION",
            _ => "NOT_FOUND",
        }));
    }
    Ok(value)
}
#[async_trait]
impl WebDataRepository for WebDataPostgres {
    async fn read(&self, owner: Option<Uuid>, query: WebRead) -> Result<Value, WebError> {
        let messages = matches!(query, WebRead::Messages { .. });
        let (sql,input)=match query {
   WebRead::Scene(id)=>("SELECT web_scene(($2->>'id')::uuid) AS value",json!({"id":id})),
   WebRead::Catalog{query,tag,cursor,limit}=>("SELECT jsonb_build_object('items',coalesce(jsonb_agg(web_scene(id) ORDER BY id),'[]'::jsonb),'next_cursor',CASE WHEN count(*)=($2->>'limit')::integer THEN max(id::text) END) AS value FROM (SELECT s.id FROM scenes s JOIN characters c ON c.id=s.character_id WHERE s.is_active AND c.is_active AND (nullif($2->>'cursor','') IS NULL OR s.id>($2->>'cursor')::uuid) AND (s.name ILIKE '%'||($2->>'query')||'%' OR c.name ILIKE '%'||($2->>'query')||'%') AND (($2->>'tag')='' OR ($2->>'tag')=ANY(c.personality_tags)) ORDER BY s.id LIMIT ($2->>'limit')::integer) page",json!({"query":query,"tag":tag,"cursor":cursor,"limit":limit})),
   WebRead::Me=>("SELECT jsonb_build_object('id',u.id,'display_name',u.display_name,'terms_accepted',u.terms_accepted_at IS NOT NULL,'terms_version',u.terms_version,'account_status',u.account_status,'linked_providers',coalesce((SELECT jsonb_agg(DISTINCT provider) FROM auth_identities WHERE user_id=u.id),'[]')) AS value FROM users u WHERE u.id=$1",json!({})),
   WebRead::Story(id)=>("SELECT web_story($1,($2->>'id')::uuid) AS value",json!({"id":id})),
   WebRead::Stories{cursor,limit}=>("SELECT jsonb_build_object('items',coalesce(jsonb_agg(web_story($1,id) ORDER BY updated_at DESC,id DESC),'[]'),'next_cursor',CASE WHEN count(*)=($2->>'limit')::integer THEN (array_agg(id ORDER BY updated_at,id))[1] END) AS value FROM (SELECT id,updated_at FROM roleplay_sessions WHERE user_id=$1 AND (($2->>'cursor') IS NULL OR (updated_at,id)<(SELECT updated_at,id FROM roleplay_sessions WHERE id=($2->>'cursor')::uuid AND user_id=$1)) ORDER BY updated_at DESC,id DESC LIMIT ($2->>'limit')::integer) page",json!({"cursor":cursor,"limit":limit})),
   WebRead::Messages{session_id,before,limit}=>("SELECT CASE WHEN EXISTS(SELECT 1 FROM roleplay_sessions WHERE id=($2->>'id')::uuid AND user_id=$1) THEN jsonb_build_object('items',coalesce(jsonb_agg(jsonb_build_object('id',m.id,'turn_id',m.turn_id,'role',m.role,'content',m.content,'created_at',m.created_at,'sequence',m.sequence,'is_regeneratable',m.role='character' AND m.turn_id IS NOT NULL AND m.sequence=(SELECT max(sequence) FROM messages WHERE session_id=m.session_id)) ORDER BY m.sequence),'[]'),'next_cursor',CASE WHEN count(*)=($2->>'limit')::integer THEN min(m.sequence)::text END) END AS value FROM (SELECT m.* FROM messages m JOIN roleplay_sessions s ON s.id=m.session_id WHERE s.user_id=$1 AND m.session_id=($2->>'id')::uuid AND (($2->>'before') IS NULL OR sequence<($2->>'before')::bigint) ORDER BY sequence DESC LIMIT ($2->>'limit')::integer) m",json!({"id":session_id,"before":before,"limit":limit})),
   WebRead::Credits=>("SELECT jsonb_build_object('balance',balance,'reserved',reserved,'available',balance-reserved,'cost_per_turn',2,'cost_per_regeneration',2,'packages',jsonb_build_array(jsonb_build_object('id','basic','credits',50,'amount_thb',29),jsonb_build_object('id','plus','credits',150,'amount_thb',69),jsonb_build_object('id','premium','credits',400,'amount_thb',149))) AS value FROM credit_balances WHERE user_id=$1",json!({})),
   WebRead::Transactions{cursor,limit}=>("SELECT jsonb_build_object('items',coalesce(jsonb_agg(to_jsonb(t) ORDER BY created_at DESC,id DESC),'[]'),'next_cursor',CASE WHEN count(*)=($2->>'limit')::integer THEN (array_agg(id ORDER BY created_at,id))[1] END) AS value FROM (SELECT id,type,amount,balance_after,description,created_at FROM credit_transactions WHERE user_id=$1 AND (($2->>'cursor') IS NULL OR (created_at,id)<(SELECT created_at,id FROM credit_transactions WHERE user_id=$1 AND id=($2->>'cursor')::uuid)) ORDER BY created_at DESC,id DESC LIMIT ($2->>'limit')::integer) t",json!({"cursor":cursor,"limit":limit})),
   WebRead::Job(id)=>("SELECT jsonb_build_object('id',id,'status',status,'kind',kind,'session_id',session_id,'result',result,'error',CASE WHEN status='failed' THEN 'GENERATION_FAILED' END) AS value FROM jobs WHERE user_id=$1 AND id=($2->>'id')::uuid AND mode='roleplay_message'",json!({"id":id})),
   WebRead::Payment(id)=>("SELECT jsonb_build_object('order_id',id,'status',status,'credits',credits_amount,'amount_thb',price_thb,'payment_url',payment_url) AS value FROM payment_orders WHERE user_id=$1 AND id=($2->>'id')::uuid",json!({"id":id})),
  };
        let mut conn = self.pool.get().await.map_err(anyhow::Error::from)?;
        let sql = format!("WITH request_bindings AS (SELECT $1::uuid,$2::jsonb) {sql}");
        let row = sql_query(sql)
            .bind::<Nullable<SqlUuid>, _>(owner)
            .bind::<Jsonb, _>(input)
            .get_result::<JsonRow>(&mut conn)
            .await
            .optional()
            .map_err(anyhow::Error::from)?;
        let mut value = checked(row.and_then(|row| row.value))?;
        if messages {
            if let Some(items) = value["items"].as_array_mut() {
                for message in items {
                    let content = message["content"].as_str().unwrap_or_default();
                    let content =
                        crate::domain::services::roleplay_text::wrap_character_content(content);
                    message["blocks"] = json!(crate::domain::services::roleplay_text::parse_text_into_blocks(&content).iter().map(|block| json!({"type": match block.block_type { crate::domain::services::ai_client::BlockType::Narration => "narration", _ => "dialogue" }, "text": block.text})).collect::<Vec<_>>());
                }
            }
        }
        Ok(value)
    }
    async fn mutate_story(
        &self,
        owner: Uuid,
        operation: &str,
        input: StoryMutation,
        key: &str,
    ) -> Result<Value, WebError> {
        let mut conn = self.pool.get().await.map_err(anyhow::Error::from)?;
        let row = sql_query("SELECT web_mutate_story($1,$2,$3,$4) AS value")
            .bind::<SqlUuid, _>(owner)
            .bind::<Text, _>(operation)
            .bind::<Jsonb, _>(serde_json::to_value(input).map_err(anyhow::Error::from)?)
            .bind::<Text, _>(key)
            .get_result::<JsonRow>(&mut conn)
            .await
            .optional()
            .map_err(anyhow::Error::from)?;
        checked(row.and_then(|row| row.value))
    }
    async fn accept_terms(&self, owner: Uuid, version: &str) -> Result<Value, WebError> {
        let mut conn = self.pool.get().await.map_err(anyhow::Error::from)?;
        let row=sql_query("UPDATE users SET terms_accepted_at=now(),terms_version=$2 WHERE id=$1 AND account_status='active' RETURNING jsonb_build_object('version',terms_version,'accepted_at',terms_accepted_at) AS value").bind::<SqlUuid,_>(owner).bind::<Text,_>(version).get_result::<JsonRow>(&mut conn).await.map_err(anyhow::Error::from)?;
        checked(row.value)
    }
}
