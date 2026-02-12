use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::entities::RoleplaySession;
use crate::domain::repositories::{RepoError, RoleplaySessionRepository};
use crate::domain::value_objects::{
    CharacterId, CharacterMood, RelationshipLevel, SceneId, SessionId, SessionStatus, UserId,
};
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::roleplay_sessions;

use super::error_mapping::{map_diesel_error, map_pool_error};

#[derive(Queryable, Selectable)]
#[diesel(table_name = roleplay_sessions)]
struct RoleplaySessionRow {
    id: Uuid,
    user_id: Uuid,
    character_id: Uuid,
    scene_id: Uuid,
    status: String,
    mood: String,
    relationship_level: String,
    message_count: i32,
    current_location: Option<String>,
    current_time: Option<String>,
    scene_summary: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl RoleplaySessionRow {
    fn into_entity(self) -> RoleplaySession {
        RoleplaySession::from_existing(
            SessionId::from_uuid(self.id),
            UserId::from_uuid(self.user_id),
            CharacterId::from_uuid(self.character_id),
            SceneId::from_uuid(self.scene_id),
            SessionStatus::from_str(&self.status).expect("invalid session_status in DB"),
            CharacterMood::from_str(&self.mood).expect("invalid character_mood in DB"),
            RelationshipLevel::from_str(&self.relationship_level)
                .expect("invalid relationship_level in DB"),
            self.message_count,
            self.current_location,
            self.current_time,
            self.scene_summary,
            self.created_at,
            self.updated_at,
        )
    }
}

#[derive(Insertable)]
#[diesel(table_name = roleplay_sessions)]
struct NewRoleplaySessionRow<'a> {
    id: &'a Uuid,
    user_id: &'a Uuid,
    character_id: &'a Uuid,
    scene_id: &'a Uuid,
    status: &'a str,
    mood: &'a str,
    relationship_level: &'a str,
    message_count: i32,
    current_location: Option<&'a str>,
    current_time: Option<&'a str>,
    scene_summary: Option<&'a str>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl<'a> NewRoleplaySessionRow<'a> {
    fn from_entity(entity: &'a RoleplaySession) -> Self {
        Self {
            id: entity.id().as_uuid(),
            user_id: entity.user_id().as_uuid(),
            character_id: entity.character_id().as_uuid(),
            scene_id: entity.scene_id().as_uuid(),
            status: entity.status().as_str(),
            mood: entity.mood().as_str(),
            relationship_level: entity.relationship_level().as_str(),
            message_count: entity.message_count(),
            current_location: entity.current_location(),
            current_time: entity.current_time(),
            scene_summary: entity.scene_summary(),
            created_at: *entity.created_at(),
            updated_at: *entity.updated_at(),
        }
    }
}

pub struct RoleplaySessionPostgres {
    pool: Arc<PgPool>,
}

impl RoleplaySessionPostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RoleplaySessionRepository for RoleplaySessionPostgres {
    async fn find_by_id(&self, id: &SessionId) -> Result<Option<RoleplaySession>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = roleplay_sessions::table
            .find(id.as_uuid())
            .first::<RoleplaySessionRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("roleplay_session.find_by_id", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn find_active_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<RoleplaySession>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = roleplay_sessions::table
            .filter(roleplay_sessions::user_id.eq(user_id.as_uuid()))
            .filter(roleplay_sessions::status.eq("active"))
            .first::<RoleplaySessionRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("roleplay_session.find_active_by_user_id", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn create(&self, session: &RoleplaySession) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let new_row = NewRoleplaySessionRow::from_entity(session);

        diesel::insert_into(roleplay_sessions::table)
            .values(&new_row)
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("roleplay_session.create", e))?;

        Ok(())
    }

    async fn update(&self, session: &RoleplaySession) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let rows_affected =
            diesel::update(roleplay_sessions::table.find(session.id().as_uuid()))
                .set((
                    roleplay_sessions::status.eq(session.status().as_str()),
                    roleplay_sessions::mood.eq(session.mood().as_str()),
                    roleplay_sessions::relationship_level
                        .eq(session.relationship_level().as_str()),
                    roleplay_sessions::message_count.eq(session.message_count()),
                    roleplay_sessions::current_location.eq(session.current_location()),
                    roleplay_sessions::current_time.eq(session.current_time()),
                    roleplay_sessions::scene_summary.eq(session.scene_summary()),
                    roleplay_sessions::updated_at.eq(session.updated_at()),
                ))
                .execute(&mut conn)
                .await
                .map_err(|e| map_diesel_error("roleplay_session.update", e))?;

        if rows_affected == 0 {
            return Err(RepoError::NotFound(format!(
                "RoleplaySession {} not found",
                session.id()
            )));
        }

        Ok(())
    }
}
