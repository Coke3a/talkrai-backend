use chrono::{DateTime, Utc};

use crate::domain::value_objects::{
    CharacterId, CharacterMood, RelationshipLevel, SceneId, SceneName,
};

#[derive(Clone)]
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
    is_adult_content: bool,
    start_relationship_level: RelationshipLevel,
    start_mood: CharacterMood,
    image_url: Option<String>,
    image_prompt: Option<String>,
    suggested_first_replies: Vec<String>,
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
        is_adult_content: bool,
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
            is_adult_content,
            start_relationship_level,
            start_mood,
            image_url: None,
            image_prompt: None,
            suggested_first_replies: Vec::new(),
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
        is_adult_content: bool,
        start_relationship_level: RelationshipLevel,
        start_mood: CharacterMood,
        image_url: Option<String>,
        image_prompt: Option<String>,
        suggested_first_replies: Vec<String>,
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
            is_adult_content,
            start_relationship_level,
            start_mood,
            image_url,
            image_prompt,
            suggested_first_replies,
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

    pub fn is_adult_content(&self) -> bool {
        self.is_adult_content
    }

    pub fn start_relationship_level(&self) -> &RelationshipLevel {
        &self.start_relationship_level
    }

    pub fn start_mood(&self) -> &CharacterMood {
        &self.start_mood
    }

    pub fn image_url(&self) -> Option<&str> {
        self.image_url.as_deref()
    }

    pub fn image_prompt(&self) -> Option<&str> {
        self.image_prompt.as_deref()
    }

    pub fn suggested_first_replies(&self) -> &[String] {
        &self.suggested_first_replies
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_with_replies(replies: Vec<String>) -> Scene {
        Scene::from_existing(
            SceneId::new(),
            CharacterId::new(),
            SceneName::from_trusted("ฉากทดสอบ".to_string()),
            "location".to_string(),
            "evening".to_string(),
            "atmosphere".to_string(),
            "situation".to_string(),
            "*narrator*".to_string(),
            "dialogue".to_string(),
            false,
            true,
            false,
            RelationshipLevel::Stranger,
            CharacterMood::Neutral,
            None,
            None,
            replies,
            Utc::now(),
            Utc::now(),
        )
    }

    #[test]
    fn returns_suggested_first_replies() {
        let scene = scene_with_replies(vec!["สวัสดี".to_string(), "เป็นไงบ้าง".to_string()]);
        assert_eq!(scene.suggested_first_replies(), &["สวัสดี", "เป็นไงบ้าง"]);
    }

    #[test]
    fn new_scene_defaults_to_no_suggested_replies() {
        let scene = Scene::new(
            CharacterId::new(),
            SceneName::from_trusted("ฉาก".to_string()),
            "location".to_string(),
            "evening".to_string(),
            "atmosphere".to_string(),
            "situation".to_string(),
            "*narrator*".to_string(),
            "dialogue".to_string(),
            false,
            false,
            RelationshipLevel::Stranger,
            CharacterMood::Neutral,
        );
        assert!(scene.suggested_first_replies().is_empty());
    }
}
