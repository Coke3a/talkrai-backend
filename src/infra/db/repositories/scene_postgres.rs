use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use std::str::FromStr;

use crate::domain::entities::Scene;
use crate::domain::repositories::{RepoError, SceneRepository};
use crate::domain::value_objects::{
    CharacterId, CharacterMood, RelationshipLevel, SceneId, SceneName,
};
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::scenes;

use super::error_mapping::{map_diesel_error, map_pool_error};

#[derive(Queryable, Selectable)]
#[diesel(table_name = scenes)]
struct SceneRow {
    id: Uuid,
    character_id: Uuid,
    name: String,
    location: String,
    time_of_day: String,
    atmosphere: String,
    situation_prompt: String,
    opening_narrator: String,
    opening_dialogue: String,
    is_default: bool,
    is_active: bool,
    is_adult_content: bool,
    start_relationship_level: String,
    start_mood: String,
    image_url: Option<String>,
    image_prompt: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    suggested_first_replies: Vec<String>,
}

impl SceneRow {
    fn into_entity(self) -> Scene {
        Scene::from_existing(
            SceneId::from_uuid(self.id),
            CharacterId::from_uuid(self.character_id),
            SceneName::from_trusted(self.name),
            self.location,
            self.time_of_day,
            self.atmosphere,
            self.situation_prompt,
            self.opening_narrator,
            self.opening_dialogue,
            self.is_default,
            self.is_active,
            self.is_adult_content,
            RelationshipLevel::from_str(&self.start_relationship_level)
                .expect("invalid start_relationship_level in DB"),
            CharacterMood::from_str(&self.start_mood).expect("invalid start_mood in DB"),
            self.image_url,
            self.image_prompt,
            self.suggested_first_replies,
            self.created_at,
            self.updated_at,
        )
    }
}

pub struct ScenePostgres {
    pool: Arc<PgPool>,
}

impl ScenePostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SceneRepository for ScenePostgres {
    async fn find_by_id(&self, id: &SceneId) -> Result<Option<Scene>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = scenes::table
            .find(id.as_uuid())
            .first::<SceneRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("scene.find_by_id", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn find_by_character_id(
        &self,
        character_id: &CharacterId,
    ) -> Result<Vec<Scene>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let results = scenes::table
            .filter(scenes::character_id.eq(character_id.as_uuid()))
            .filter(scenes::is_active.eq(true))
            .load::<SceneRow>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("scene.find_by_character_id", e))?;

        Ok(results.into_iter().map(|row| row.into_entity()).collect())
    }

    async fn find_default_by_character_id(
        &self,
        character_id: &CharacterId,
    ) -> Result<Option<Scene>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = scenes::table
            .filter(scenes::character_id.eq(character_id.as_uuid()))
            .filter(scenes::is_default.eq(true))
            .filter(scenes::is_active.eq(true))
            .first::<SceneRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("scene.find_default_by_character_id", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn find_all_active(&self) -> Result<Vec<Scene>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let results = scenes::table
            .filter(scenes::is_active.eq(true))
            .filter(scenes::is_adult_content.eq(false))
            .order(scenes::created_at.desc())
            .load::<SceneRow>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("scene.find_all_active", e))?;

        Ok(results.into_iter().map(|row| row.into_entity()).collect())
    }
}
