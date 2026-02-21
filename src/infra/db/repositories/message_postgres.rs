use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use std::str::FromStr;

use crate::domain::entities::Message;
use crate::domain::repositories::{MessageRepository, RepoError};
use crate::domain::value_objects::{MessageId, MessageRole, SessionId};
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::messages;

use super::error_mapping::{map_diesel_error, map_pool_error};

#[derive(Queryable, Selectable)]
#[diesel(table_name = messages)]
struct MessageRow {
    id: Uuid,
    session_id: Uuid,
    role: String,
    content: String,
    mood: Option<String>,
    created_at: DateTime<Utc>,
}

impl MessageRow {
    fn into_entity(self) -> Message {
        Message::from_existing(
            MessageId::from_uuid(self.id),
            SessionId::from_uuid(self.session_id),
            MessageRole::from_str(&self.role).expect("invalid message_role in DB"),
            self.content,
            self.mood,
            self.created_at,
        )
    }
}

#[derive(Insertable)]
#[diesel(table_name = messages)]
struct NewMessageRow<'a> {
    id: &'a Uuid,
    session_id: &'a Uuid,
    role: &'a str,
    content: &'a str,
    mood: Option<&'a str>,
    created_at: DateTime<Utc>,
}

impl<'a> NewMessageRow<'a> {
    fn from_entity(entity: &'a Message) -> Self {
        Self {
            id: entity.id().as_uuid(),
            session_id: entity.session_id().as_uuid(),
            role: entity.role().as_str(),
            content: entity.content(),
            mood: entity.mood(),
            created_at: *entity.created_at(),
        }
    }
}

pub struct MessagePostgres {
    pool: Arc<PgPool>,
}

impl MessagePostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MessageRepository for MessagePostgres {
    async fn create(&self, message: &Message) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let new_row = NewMessageRow::from_entity(message);

        diesel::insert_into(messages::table)
            .values(&new_row)
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("message.create", e))?;

        Ok(())
    }

    async fn create_many(&self, msgs: &[Message]) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let new_rows: Vec<NewMessageRow> = msgs.iter().map(NewMessageRow::from_entity).collect();

        diesel::insert_into(messages::table)
            .values(&new_rows)
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("message.create_many", e))?;

        Ok(())
    }

    async fn find_by_session_id(
        &self,
        session_id: &SessionId,
        limit: i64,
    ) -> Result<Vec<Message>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let mut results = messages::table
            .filter(messages::session_id.eq(session_id.as_uuid()))
            .order(messages::created_at.desc())
            .limit(limit)
            .load::<MessageRow>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("message.find_by_session_id", e))?;

        results.reverse();

        Ok(results.into_iter().map(|row| row.into_entity()).collect())
    }
}
