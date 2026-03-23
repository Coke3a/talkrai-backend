use std::sync::Arc;

use crate::domain::entities::tag_definition::TagCategory;
use crate::domain::repositories::TagDefinitionRepository;
use crate::usecases::UsecaseError;

pub struct TagItem {
    pub key: String,
    pub display_name: String,
    pub description: Option<String>,
}

pub struct GetTagsOutput {
    pub appearance: Vec<TagItem>,
    pub personality: Vec<TagItem>,
}

pub struct GetTagsUseCase {
    tag_def_repo: Arc<dyn TagDefinitionRepository>,
}

impl GetTagsUseCase {
    pub fn new(tag_def_repo: Arc<dyn TagDefinitionRepository>) -> Self {
        Self { tag_def_repo }
    }

    pub async fn execute(&self) -> Result<GetTagsOutput, UsecaseError> {
        let all_tags = self.tag_def_repo.find_active().await?;

        let mut appearance = Vec::new();
        let mut personality = Vec::new();

        for tag in all_tags {
            let item = TagItem {
                key: tag.key().to_string(),
                display_name: tag.display_name().to_string(),
                description: tag.description().map(String::from),
            };

            match tag.category() {
                TagCategory::Appearance => appearance.push(item),
                TagCategory::Personality => personality.push(item),
            }
        }

        Ok(GetTagsOutput {
            appearance,
            personality,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::TagDefinition;
    use crate::domain::repositories::RepoError;
    use async_trait::async_trait;

    struct MockTagDefRepo {
        tags: Vec<TagDefinition>,
    }

    #[async_trait]
    impl TagDefinitionRepository for MockTagDefRepo {
        async fn find_active(&self) -> Result<Vec<TagDefinition>, RepoError> {
            Ok(self.tags.clone())
        }

        async fn find_active_by_category(
            &self,
            category: &TagCategory,
        ) -> Result<Vec<TagDefinition>, RepoError> {
            Ok(self
                .tags
                .iter()
                .filter(|t| t.category() == category)
                .cloned()
                .collect())
        }

        async fn validate_keys(
            &self,
            category: &TagCategory,
            keys: &[String],
        ) -> Result<bool, RepoError> {
            let valid: std::collections::HashSet<&str> = self
                .tags
                .iter()
                .filter(|t| t.category() == category && t.is_active())
                .map(|t| t.key())
                .collect();
            Ok(keys.iter().all(|k| valid.contains(k.as_str())))
        }
    }

    fn make_tag(
        category: &str,
        key: &str,
        display_name: &str,
        desc: Option<&str>,
    ) -> TagDefinition {
        TagDefinition::from_existing(
            uuid::Uuid::new_v4(),
            category.parse().unwrap(),
            key.to_string(),
            display_name.to_string(),
            desc.map(String::from),
            0,
            true,
        )
    }

    #[tokio::test]
    async fn execute_groups_tags_by_category() {
        let repo = MockTagDefRepo {
            tags: vec![
                make_tag("appearance", "cute", "น่ารัก", Some("หน้าอ่อนหวาน")),
                make_tag("personality", "tsundere", "ซึนเดเระ", Some("ภายนอกเย็นชา")),
                make_tag("appearance", "cool", "เท่", None),
                make_tag("personality", "cheerful", "ร่าเริง", Some("สดใส")),
            ],
        };

        let usecase = GetTagsUseCase::new(Arc::new(repo));
        let output = usecase.execute().await.unwrap();

        assert_eq!(output.appearance.len(), 2);
        assert_eq!(output.personality.len(), 2);

        assert_eq!(output.appearance[0].key, "cute");
        assert_eq!(output.appearance[0].display_name, "น่ารัก");
        assert_eq!(
            output.appearance[0].description.as_deref(),
            Some("หน้าอ่อนหวาน")
        );

        assert_eq!(output.appearance[1].key, "cool");
        assert_eq!(output.appearance[1].description, None);

        assert_eq!(output.personality[0].key, "tsundere");
        assert_eq!(output.personality[1].key, "cheerful");
    }

    #[tokio::test]
    async fn execute_empty_repo_returns_empty_lists() {
        let repo = MockTagDefRepo { tags: vec![] };
        let usecase = GetTagsUseCase::new(Arc::new(repo));
        let output = usecase.execute().await.unwrap();

        assert!(output.appearance.is_empty());
        assert!(output.personality.is_empty());
    }

    #[tokio::test]
    async fn execute_only_appearance_tags() {
        let repo = MockTagDefRepo {
            tags: vec![make_tag("appearance", "elegant", "สง่างาม", None)],
        };

        let usecase = GetTagsUseCase::new(Arc::new(repo));
        let output = usecase.execute().await.unwrap();

        assert_eq!(output.appearance.len(), 1);
        assert!(output.personality.is_empty());
    }

    #[tokio::test]
    async fn execute_only_personality_tags() {
        let repo = MockTagDefRepo {
            tags: vec![make_tag("personality", "shy", "ขี้อาย", Some("เขินง่าย"))],
        };

        let usecase = GetTagsUseCase::new(Arc::new(repo));
        let output = usecase.execute().await.unwrap();

        assert!(output.appearance.is_empty());
        assert_eq!(output.personality.len(), 1);
        assert_eq!(output.personality[0].key, "shy");
    }
}
