use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::entities::Character;
use crate::domain::repositories::{CharacterRepository, RepoError};
use crate::domain::value_objects::{CharacterGender, CharacterId, CharacterName};
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::characters;

use super::error_mapping::{map_diesel_error, map_pool_error};

#[derive(Queryable, Selectable)]
#[diesel(table_name = characters)]
struct CharacterRow {
    id: Uuid,
    name: String,
    personality: String,
    speaking_style: String,
    background: String,
    system_prompt: String,
    avatar_url: Option<String>,
    appearance_prompt: Option<String>,
    genre_tags: Vec<String>,
    gender: String,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl CharacterRow {
    fn into_entity(self) -> Character {
        Character::from_existing(
            CharacterId::from_uuid(self.id),
            CharacterName::from_trusted(self.name),
            self.personality,
            self.speaking_style,
            self.background,
            self.system_prompt,
            self.avatar_url,
            self.appearance_prompt,
            self.genre_tags,
            CharacterGender::from_str(&self.gender).expect("invalid gender in DB"),
            self.is_active,
            self.created_at,
            self.updated_at,
        )
    }
}

pub struct CharacterPostgres {
    pool: Arc<PgPool>,
}

impl CharacterPostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CharacterRepository for CharacterPostgres {
    async fn find_by_id(&self, id: &CharacterId) -> Result<Option<Character>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = characters::table
            .find(id.as_uuid())
            .first::<CharacterRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("character.find_by_id", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn find_active(&self) -> Result<Vec<Character>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let results = characters::table
            .filter(characters::is_active.eq(true))
            .load::<CharacterRow>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("character.find_active", e))?;

        Ok(results.into_iter().map(|row| row.into_entity()).collect())
    }
}
