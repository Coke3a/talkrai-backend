use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::entities::CharacterMemory;
use crate::domain::repositories::{CharacterMemoryRepository, RepoError};
use crate::domain::value_objects::{CharacterId, CharacterMemoryId, MemoryType, UserId};
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::character_memories;

use super::error_mapping::{map_diesel_error, map_pool_error};

#[derive(Queryable, Selectable)]
#[diesel(table_name = character_memories)]
struct CharacterMemoryRow {
    id: Uuid,
    user_id: Uuid,
    character_id: Uuid,
    memory_type: String,
    content: String,
    importance: i16,
    last_recalled_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl CharacterMemoryRow {
    fn into_entity(self) -> CharacterMemory {
        CharacterMemory::from_existing(
            CharacterMemoryId::from_uuid(self.id),
            UserId::from_uuid(self.user_id),
            CharacterId::from_uuid(self.character_id),
            MemoryType::from_str(&self.memory_type).expect("invalid memory_type in DB"),
            self.content,
            self.importance,
            self.last_recalled_at,
            self.created_at,
            self.updated_at,
        )
    }
}

#[derive(Insertable)]
#[diesel(table_name = character_memories)]
struct NewCharacterMemoryRow<'a> {
    id: &'a Uuid,
    user_id: &'a Uuid,
    character_id: &'a Uuid,
    memory_type: &'a str,
    content: &'a str,
    importance: i16,
    last_recalled_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl<'a> NewCharacterMemoryRow<'a> {
    fn from_entity(entity: &'a CharacterMemory) -> Self {
        Self {
            id: entity.id().as_uuid(),
            user_id: entity.user_id().as_uuid(),
            character_id: entity.character_id().as_uuid(),
            memory_type: entity.memory_type().as_str(),
            content: entity.content(),
            importance: entity.importance(),
            last_recalled_at: entity.last_recalled_at().copied(),
            created_at: *entity.created_at(),
            updated_at: *entity.updated_at(),
        }
    }
}

pub struct CharacterMemoryPostgres {
    pool: Arc<PgPool>,
}

impl CharacterMemoryPostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CharacterMemoryRepository for CharacterMemoryPostgres {
    async fn find_by_user_and_character(
        &self,
        user_id: &UserId,
        character_id: &CharacterId,
        limit: i64,
    ) -> Result<Vec<CharacterMemory>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let results = character_memories::table
            .filter(character_memories::user_id.eq(user_id.as_uuid()))
            .filter(character_memories::character_id.eq(character_id.as_uuid()))
            .order(character_memories::importance.desc())
            .limit(limit)
            .load::<CharacterMemoryRow>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("character_memory.find_by_user_and_character", e))?;

        Ok(results.into_iter().map(|row| row.into_entity()).collect())
    }

    async fn create(&self, memory: &CharacterMemory) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let new_row = NewCharacterMemoryRow::from_entity(memory);

        diesel::insert_into(character_memories::table)
            .values(&new_row)
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("character_memory.create", e))?;

        Ok(())
    }

    async fn create_many(&self, memories: &[CharacterMemory]) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let new_rows: Vec<NewCharacterMemoryRow> =
            memories.iter().map(NewCharacterMemoryRow::from_entity).collect();

        diesel::insert_into(character_memories::table)
            .values(&new_rows)
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("character_memory.create_many", e))?;

        Ok(())
    }

    async fn update(&self, memory: &CharacterMemory) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let rows_affected =
            diesel::update(character_memories::table.find(memory.id().as_uuid()))
                .set((
                    character_memories::memory_type.eq(memory.memory_type().as_str()),
                    character_memories::content.eq(memory.content()),
                    character_memories::importance.eq(memory.importance()),
                    character_memories::last_recalled_at
                        .eq(memory.last_recalled_at().copied()),
                    character_memories::updated_at.eq(memory.updated_at()),
                ))
                .execute(&mut conn)
                .await
                .map_err(|e| map_diesel_error("character_memory.update", e))?;

        if rows_affected == 0 {
            return Err(RepoError::NotFound(format!(
                "CharacterMemory {} not found",
                memory.id()
            )));
        }

        Ok(())
    }

    async fn delete_by_user_and_character(
        &self,
        user_id: &UserId,
        character_id: &CharacterId,
    ) -> Result<u64, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let rows_affected = diesel::delete(
            character_memories::table
                .filter(character_memories::user_id.eq(user_id.as_uuid()))
                .filter(character_memories::character_id.eq(character_id.as_uuid())),
        )
        .execute(&mut conn)
        .await
        .map_err(|e| map_diesel_error("character_memory.delete_by_user_and_character", e))?;

        Ok(rows_affected as u64)
    }
}
