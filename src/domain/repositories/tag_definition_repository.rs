use async_trait::async_trait;

use crate::domain::entities::tag_definition::TagCategory;
use crate::domain::entities::TagDefinition;
use crate::domain::repositories::RepoError;

#[async_trait]
pub trait TagDefinitionRepository: Send + Sync {
    async fn find_active(&self) -> Result<Vec<TagDefinition>, RepoError>;
    async fn find_active_by_category(
        &self,
        category: &TagCategory,
    ) -> Result<Vec<TagDefinition>, RepoError>;
    async fn validate_keys(
        &self,
        category: &TagCategory,
        keys: &[String],
    ) -> Result<bool, RepoError>;
}
