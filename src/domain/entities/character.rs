use chrono::{DateTime, Utc};

use crate::domain::value_objects::{CharacterGender, CharacterId, CharacterName};

pub struct Character {
    id: CharacterId,
    name: CharacterName,
    personality: String,
    speaking_style: String,
    background: String,
    system_prompt: String,
    avatar_url: Option<String>,
    genre_tags: Vec<String>,
    gender: CharacterGender,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl Character {
    pub fn new(
        name: CharacterName,
        personality: String,
        speaking_style: String,
        background: String,
        system_prompt: String,
        avatar_url: Option<String>,
        genre_tags: Vec<String>,
        gender: CharacterGender,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: CharacterId::new(),
            name,
            personality,
            speaking_style,
            background,
            system_prompt,
            avatar_url,
            genre_tags,
            gender,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_existing(
        id: CharacterId,
        name: CharacterName,
        personality: String,
        speaking_style: String,
        background: String,
        system_prompt: String,
        avatar_url: Option<String>,
        genre_tags: Vec<String>,
        gender: CharacterGender,
        is_active: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            name,
            personality,
            speaking_style,
            background,
            system_prompt,
            avatar_url,
            genre_tags,
            gender,
            is_active,
            created_at,
            updated_at,
        }
    }

    pub fn id(&self) -> &CharacterId {
        &self.id
    }

    pub fn name(&self) -> &CharacterName {
        &self.name
    }

    pub fn personality(&self) -> &str {
        &self.personality
    }

    pub fn speaking_style(&self) -> &str {
        &self.speaking_style
    }

    pub fn background(&self) -> &str {
        &self.background
    }

    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    pub fn avatar_url(&self) -> Option<&str> {
        self.avatar_url.as_deref()
    }

    pub fn genre_tags(&self) -> &[String] {
        &self.genre_tags
    }

    pub fn gender(&self) -> &CharacterGender {
        &self.gender
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}
