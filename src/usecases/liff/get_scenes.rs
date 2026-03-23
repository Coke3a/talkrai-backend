use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::repositories::{CharacterRepository, SceneRepository};
use crate::usecases::UsecaseError;

pub struct SceneCharacterItem {
    pub id: Uuid,
    pub name: String,
    pub gender: String,
    pub avatar_url: Option<String>,
    pub appearance_tags: Vec<String>,
    pub personality_tags: Vec<String>,
}

pub struct SceneItem {
    pub id: Uuid,
    pub name: String,
    pub location: String,
    pub time_of_day: String,
    pub atmosphere_summary: String,
    pub opening_narrator: String,
    pub opening_dialogue: String,
    pub start_mood: String,
    pub image_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub character: SceneCharacterItem,
}

pub struct GetScenesOutput {
    pub scenes: Vec<SceneItem>,
}

pub struct GetScenesUseCase {
    scene_repo: Arc<dyn SceneRepository>,
    character_repo: Arc<dyn CharacterRepository>,
}

impl GetScenesUseCase {
    pub fn new(
        scene_repo: Arc<dyn SceneRepository>,
        character_repo: Arc<dyn CharacterRepository>,
    ) -> Self {
        Self {
            scene_repo,
            character_repo,
        }
    }

    pub async fn execute(&self) -> Result<GetScenesOutput, UsecaseError> {
        let scenes = self.scene_repo.find_all_active().await?;
        let characters = self.character_repo.find_active().await?;

        let char_map: HashMap<Uuid, _> =
            characters.iter().map(|c| (*c.id().as_uuid(), c)).collect();

        let mut items = Vec::with_capacity(scenes.len());

        for scene in scenes {
            let Some(character) = char_map.get(scene.character_id().as_uuid()) else {
                continue;
            };

            items.push(SceneItem {
                id: *scene.id().as_uuid(),
                name: scene.name().as_str().to_string(),
                location: scene.location().to_string(),
                time_of_day: scene.time_of_day().to_string(),
                atmosphere_summary: parse_atmosphere_summary(scene.atmosphere()),
                opening_narrator: scene.opening_narrator().to_string(),
                opening_dialogue: scene.opening_dialogue().to_string(),
                start_mood: scene.start_mood().as_str().to_string(),
                image_url: scene.image_url().map(String::from),
                created_at: *scene.created_at(),
                character: SceneCharacterItem {
                    id: *character.id().as_uuid(),
                    name: character.name().as_str().to_string(),
                    gender: character.gender().as_str().to_string(),
                    avatar_url: character.avatar_url().map(String::from),
                    appearance_tags: character.appearance_tags().to_vec(),
                    personality_tags: character.personality_tags().to_vec(),
                },
            });
        }

        Ok(GetScenesOutput { scenes: items })
    }
}

fn parse_atmosphere_summary(atmosphere: &str) -> String {
    #[derive(serde::Deserialize)]
    struct AtmosphereJson {
        mood: Option<String>,
        tags: Option<Vec<String>>,
    }

    match serde_json::from_str::<AtmosphereJson>(atmosphere) {
        Ok(parsed) => {
            let mood = parsed.mood.unwrap_or_default();
            let tags = parsed.tags.unwrap_or_default();
            if tags.is_empty() {
                mood
            } else {
                format!("{} — {}", mood, tags.join(", "))
            }
        }
        Err(_) => atmosphere.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{Character, Scene};
    use crate::domain::repositories::RepoError;
    use crate::domain::value_objects::{
        CharacterGender, CharacterId, CharacterMood, CharacterName, RelationshipLevel, SceneId,
        SceneName,
    };
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct MockSceneRepo {
        scenes: Mutex<Vec<Scene>>,
    }

    impl MockSceneRepo {
        fn new(scenes: Vec<Scene>) -> Self {
            Self {
                scenes: Mutex::new(scenes),
            }
        }
    }

    #[async_trait]
    impl SceneRepository for MockSceneRepo {
        async fn find_by_id(&self, _id: &SceneId) -> Result<Option<Scene>, RepoError> {
            Ok(None)
        }
        async fn find_by_character_id(
            &self,
            _character_id: &CharacterId,
        ) -> Result<Vec<Scene>, RepoError> {
            Ok(vec![])
        }
        async fn find_default_by_character_id(
            &self,
            _character_id: &CharacterId,
        ) -> Result<Option<Scene>, RepoError> {
            Ok(None)
        }
        async fn find_all_active(&self) -> Result<Vec<Scene>, RepoError> {
            Ok(self.scenes.lock().unwrap().drain(..).collect())
        }
    }

    struct MockCharacterRepo {
        characters: Mutex<Vec<Character>>,
    }

    impl MockCharacterRepo {
        fn new(characters: Vec<Character>) -> Self {
            Self {
                characters: Mutex::new(characters),
            }
        }
    }

    #[async_trait]
    impl CharacterRepository for MockCharacterRepo {
        async fn find_by_id(&self, _id: &CharacterId) -> Result<Option<Character>, RepoError> {
            Ok(None)
        }
        async fn find_active(&self) -> Result<Vec<Character>, RepoError> {
            Ok(self.characters.lock().unwrap().drain(..).collect())
        }
    }

    fn make_character(id: Uuid, name: &str, gender: CharacterGender) -> Character {
        Character::from_existing(
            CharacterId::from_uuid(id),
            CharacterName::from_trusted(name.to_string()),
            "personality".to_string(),
            "speaking_style".to_string(),
            "background".to_string(),
            "system_prompt".to_string(),
            Some("https://avatar.test/img.webp".to_string()),
            None,
            vec!["cool".to_string()],
            vec!["tsundere".to_string()],
            gender,
            true,
            Utc::now(),
            Utc::now(),
        )
    }

    fn make_scene(id: Uuid, character_id: Uuid, name: &str, atmosphere: &str) -> Scene {
        Scene::from_existing(
            SceneId::from_uuid(id),
            CharacterId::from_uuid(character_id),
            SceneName::from_trusted(name.to_string()),
            "location".to_string(),
            "evening".to_string(),
            atmosphere.to_string(),
            "situation_prompt".to_string(),
            "*narrator text*".to_string(),
            "dialogue text".to_string(),
            false,
            true,
            false,
            RelationshipLevel::Stranger,
            CharacterMood::Neutral,
            Some("https://img.test/scene.webp".to_string()),
            None,
            Utc::now(),
            Utc::now(),
        )
    }

    #[tokio::test]
    async fn execute_joins_scenes_with_characters() {
        let char_id = Uuid::new_v4();
        let scene_id = Uuid::new_v4();

        let usecase = GetScenesUseCase::new(
            Arc::new(MockSceneRepo::new(vec![make_scene(
                scene_id,
                char_id,
                "Test Scene",
                r#"{"mood":"romantic","tags":["อบอุ่น","เงียบสงบ"]}"#,
            )])),
            Arc::new(MockCharacterRepo::new(vec![make_character(
                char_id,
                "Kira",
                CharacterGender::Male,
            )])),
        );

        let output = usecase.execute().await.unwrap();
        assert_eq!(output.scenes.len(), 1);

        let item = &output.scenes[0];
        assert_eq!(item.id, scene_id);
        assert_eq!(item.name, "Test Scene");
        assert_eq!(item.character.id, char_id);
        assert_eq!(item.character.name, "Kira");
        assert_eq!(item.character.gender, "male");
        assert_eq!(item.atmosphere_summary, "romantic — อบอุ่น, เงียบสงบ");
        assert_eq!(item.start_mood, "neutral");
    }

    #[tokio::test]
    async fn execute_skips_scenes_with_missing_characters() {
        let orphan_char_id = Uuid::new_v4();

        let usecase = GetScenesUseCase::new(
            Arc::new(MockSceneRepo::new(vec![make_scene(
                Uuid::new_v4(),
                orphan_char_id,
                "Orphan Scene",
                "plain text atmosphere",
            )])),
            Arc::new(MockCharacterRepo::new(vec![])),
        );

        let output = usecase.execute().await.unwrap();
        assert!(output.scenes.is_empty());
    }

    #[tokio::test]
    async fn execute_empty_repos_return_empty_output() {
        let usecase = GetScenesUseCase::new(
            Arc::new(MockSceneRepo::new(vec![])),
            Arc::new(MockCharacterRepo::new(vec![])),
        );

        let output = usecase.execute().await.unwrap();
        assert!(output.scenes.is_empty());
    }

    #[tokio::test]
    async fn atmosphere_json_parsed_correctly() {
        let summary = parse_atmosphere_summary(r#"{"mood":"tense","tags":["ใกล้ชิด","ตึงเครียด"]}"#);
        assert_eq!(summary, "tense — ใกล้ชิด, ตึงเครียด");
    }

    #[tokio::test]
    async fn atmosphere_plain_text_fallback() {
        let summary = parse_atmosphere_summary("just a plain string");
        assert_eq!(summary, "just a plain string");
    }

    #[tokio::test]
    async fn atmosphere_mood_only_no_tags() {
        let summary = parse_atmosphere_summary(r#"{"mood":"romantic"}"#);
        assert_eq!(summary, "romantic");
    }
}
