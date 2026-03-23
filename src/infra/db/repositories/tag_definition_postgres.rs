use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::entities::tag_definition::TagCategory;
use crate::domain::entities::TagDefinition;
use crate::domain::repositories::{RepoError, TagDefinitionRepository};
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::tag_definitions;

use super::error_mapping::{map_diesel_error, map_pool_error};

#[allow(dead_code)]
#[derive(Queryable, Selectable)]
#[diesel(table_name = tag_definitions)]
struct TagDefinitionRow {
    id: Uuid,
    category: String,
    key: String,
    display_name: String,
    description: Option<String>,
    sort_order: i32,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TagDefinitionRow {
    fn into_entity(self) -> TagDefinition {
        TagDefinition::from_existing(
            self.id,
            TagCategory::from_str(&self.category).expect("invalid tag_category in DB"),
            self.key,
            self.display_name,
            self.description,
            self.sort_order,
            self.is_active,
        )
    }
}

pub struct TagDefinitionPostgres {
    pool: Arc<PgPool>,
}

impl TagDefinitionPostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TagDefinitionRepository for TagDefinitionPostgres {
    async fn find_active(&self) -> Result<Vec<TagDefinition>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let results = tag_definitions::table
            .filter(tag_definitions::is_active.eq(true))
            .order((
                tag_definitions::category.asc(),
                tag_definitions::sort_order.asc(),
            ))
            .load::<TagDefinitionRow>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("tag_definition.find_active", e))?;

        Ok(results.into_iter().map(|row| row.into_entity()).collect())
    }

    async fn find_active_by_category(
        &self,
        category: &TagCategory,
    ) -> Result<Vec<TagDefinition>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let results = tag_definitions::table
            .filter(tag_definitions::is_active.eq(true))
            .filter(tag_definitions::category.eq(category.as_str()))
            .order(tag_definitions::sort_order.asc())
            .load::<TagDefinitionRow>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("tag_definition.find_active_by_category", e))?;

        Ok(results.into_iter().map(|row| row.into_entity()).collect())
    }

    async fn validate_keys(
        &self,
        category: &TagCategory,
        keys: &[String],
    ) -> Result<bool, RepoError> {
        if keys.is_empty() {
            return Ok(true);
        }

        let unique_keys: Vec<&str> = {
            let mut seen = std::collections::HashSet::new();
            keys.iter()
                .filter(|k| seen.insert(k.as_str()))
                .map(|k| k.as_str())
                .collect()
        };

        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let count: i64 = tag_definitions::table
            .filter(tag_definitions::is_active.eq(true))
            .filter(tag_definitions::category.eq(category.as_str()))
            .filter(tag_definitions::key.eq_any(&unique_keys))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(|e| map_diesel_error("tag_definition.validate_keys", e))?;

        Ok(count as usize == unique_keys.len())
    }
}
