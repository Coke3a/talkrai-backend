use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagCategory {
    Appearance,
    Personality,
}

impl TagCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Appearance => "appearance",
            Self::Personality => "personality",
        }
    }
}

impl std::str::FromStr for TagCategory {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "appearance" => Ok(Self::Appearance),
            "personality" => Ok(Self::Personality),
            _ => Err(DomainError::InvalidField {
                field: "tag_category",
                reason: "must be 'appearance' or 'personality'",
            }),
        }
    }
}

#[derive(Clone)]
pub struct TagDefinition {
    id: uuid::Uuid,
    category: TagCategory,
    key: String,
    display_name: String,
    description: Option<String>,
    sort_order: i32,
    is_active: bool,
}

impl TagDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn from_existing(
        id: uuid::Uuid,
        category: TagCategory,
        key: String,
        display_name: String,
        description: Option<String>,
        sort_order: i32,
        is_active: bool,
    ) -> Self {
        Self {
            id,
            category,
            key,
            display_name,
            description,
            sort_order,
            is_active,
        }
    }

    pub fn id(&self) -> &uuid::Uuid {
        &self.id
    }

    pub fn category(&self) -> &TagCategory {
        &self.category
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn sort_order(&self) -> i32 {
        self.sort_order
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }
}
