use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use crate::domain::repositories::{AppConfigRepository, RepoError};
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::app_config;

use super::error_mapping::{map_diesel_error, map_pool_error};

pub struct AppConfigPostgres {
    pool: Arc<PgPool>,
}

impl AppConfigPostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AppConfigRepository for AppConfigPostgres {
    async fn get(&self, key: &str) -> Result<Option<String>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = app_config::table
            .find(key)
            .select(app_config::value)
            .first::<String>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("app_config.get", e))?;

        Ok(result)
    }

    async fn get_many(&self, keys: &[&str]) -> Result<HashMap<String, String>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let results: Vec<(String, String)> = app_config::table
            .filter(app_config::key.eq_any(keys))
            .select((app_config::key, app_config::value))
            .load::<(String, String)>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("app_config.get_many", e))?;

        Ok(results.into_iter().collect())
    }
}
