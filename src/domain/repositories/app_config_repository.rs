use std::collections::HashMap;

use async_trait::async_trait;

use super::error::RepoError;

#[async_trait]
pub trait AppConfigRepository: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<String>, RepoError>;
    async fn get_many(&self, keys: &[&str]) -> Result<HashMap<String, String>, RepoError>;
}
