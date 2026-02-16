use chrono::{DateTime, Utc};

use crate::domain::value_objects::{
    CharacterId, CharacterMood, RelationshipLevel, SceneId, SceneName,
};

pub struct Scene {
    id: SceneId,
    character_id: CharacterId,
    name: SceneName,
    location: String,
    time_of_day: String,
    atmosphere: String,
    situation_prompt: String,
    opening_narrator: String,
    opening_dialogue: String,
    is_default: bool,
    is_active: bool,
    start_relationship_level: RelationshipLevel,
    start_mood: CharacterMood,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl Scene {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        character_id: CharacterId,
        name: SceneName,
        location: String,
        time_of_day: String,
        atmosphere: String,
        situation_prompt: String,
        opening_narrator: String,
        opening_dialogue: String,
        is_default: bool,
        start_relationship_level: RelationshipLevel,
        start_mood: CharacterMood,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: SceneId::new(),
            character_id,
            name,
            location,
            time_of_day,
            atmosphere,
            situation_prompt,
            opening_narrator,
            opening_dialogue,
            is_default,
            is_active: true,
            start_relationship_level,
            start_mood,
            created_at: now,
            updated_at: now,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_existing(
        id: SceneId,
        character_id: CharacterId,
        name: SceneName,
        location: String,
        time_of_day: String,
        atmosphere: String,
        situation_prompt: String,
        opening_narrator: String,
        opening_dialogue: String,
        is_default: bool,
        is_active: bool,
        start_relationship_level: RelationshipLevel,
        start_mood: CharacterMood,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            character_id,
            name,
            location,
            time_of_day,
            atmosphere,
            situation_prompt,
            opening_narrator,
            opening_dialogue,
            is_default,
            is_active,
            start_relationship_level,
            start_mood,
            created_at,
            updated_at,
        }
    }

    pub fn id(&self) -> &SceneId {
        &self.id
    }

    pub fn character_id(&self) -> &CharacterId {
        &self.character_id
    }

    pub fn name(&self) -> &SceneName {
        &self.name
    }

    pub fn location(&self) -> &str {
        &self.location
    }

    pub fn time_of_day(&self) -> &str {
        &self.time_of_day
    }

    pub fn atmosphere(&self) -> &str {
        &self.atmosphere
    }

    pub fn situation_prompt(&self) -> &str {
        &self.situation_prompt
    }

    pub fn opening_narrator(&self) -> &str {
        &self.opening_narrator
    }

    pub fn opening_dialogue(&self) -> &str {
        &self.opening_dialogue
    }

    pub fn is_default(&self) -> bool {
        self.is_default
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn start_relationship_level(&self) -> &RelationshipLevel {
        &self.start_relationship_level
    }

    pub fn start_mood(&self) -> &CharacterMood {
        &self.start_mood
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}
