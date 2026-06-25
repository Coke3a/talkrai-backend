use std::sync::Arc;

use uuid::Uuid;

use crate::domain::entities::{Message, RoleplaySession};
use crate::domain::repositories::{
    CharacterRepository, MessageRepository, RoleplaySessionRepository, SceneRepository,
    UserRepository,
};
use crate::domain::services::line_client::{LineClient, LineMessage};
use crate::domain::value_objects::MessageRole;
use crate::infra::ai::response::parse_text_into_blocks;
use crate::infra::line::{flex_messages, roleplay_flex};
use crate::usecases::liff::require_active_user::require_active_user;
use crate::usecases::UsecaseError;

/// Derive the opening-message quick reply from a scene's suggested first replies.
/// Empty slice => `None` (fallback: push the opening with no quick reply, no error).
fn derive_opening_quick_reply(suggested: &[String]) -> Option<Vec<String>> {
    if suggested.is_empty() {
        None
    } else {
        Some(suggested.to_vec())
    }
}

pub struct StartSessionInput {
    pub line_user_id: String,
    pub scene_id: Uuid,
    pub rich_menu_b_id: String,
}

pub struct StartSessionOutput {
    pub session_id: Uuid,
}

pub struct StartSessionUseCase {
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    scene_repo: Arc<dyn SceneRepository>,
    character_repo: Arc<dyn CharacterRepository>,
    message_repo: Arc<dyn MessageRepository>,
    line_client: Arc<dyn LineClient>,
}

impl StartSessionUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn RoleplaySessionRepository>,
        scene_repo: Arc<dyn SceneRepository>,
        character_repo: Arc<dyn CharacterRepository>,
        message_repo: Arc<dyn MessageRepository>,
        line_client: Arc<dyn LineClient>,
    ) -> Self {
        Self {
            user_repo,
            session_repo,
            scene_repo,
            character_repo,
            message_repo,
            line_client,
        }
    }

    pub async fn execute(
        &self,
        input: StartSessionInput,
    ) -> Result<StartSessionOutput, UsecaseError> {
        // 1. Find user
        let user = require_active_user(&*self.user_repo, &input.line_user_id).await?;

        // 2. Auto-accept terms if not yet accepted
        let mut user = user;
        if !user.has_accepted_terms() {
            user.accept_terms()
                .map_err(|e| UsecaseError::Validation(format!("Failed to accept terms: {}", e)))?;
            self.user_repo.update(&user).await?;
            tracing::info!(
                user_id = %user.id().as_uuid(),
                "Auto-accepted terms during session start"
            );
        }

        // 3. Fetch scene
        let scene_id = crate::domain::value_objects::SceneId::from_uuid(input.scene_id);
        let scene = self
            .scene_repo
            .find_by_id(&scene_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Scene not found".into()))?;

        if !scene.is_active() {
            return Err(UsecaseError::Validation("Scene is not active".into()));
        }

        // 4. Fetch character
        let character = self
            .character_repo
            .find_by_id(scene.character_id())
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Character not found".into()))?;

        // 5. End existing active session if any
        if let Some(mut active_session) =
            self.session_repo.find_active_by_user_id(user.id()).await?
        {
            active_session.end()?;
            self.session_repo.update(&active_session).await?;
            tracing::info!(
                session_id = %active_session.id().as_uuid(),
                "Ended previous active session before starting new one"
            );
        }

        // 6. Create new session
        let session = RoleplaySession::new(
            user.id().clone(),
            character.id().clone(),
            scene.id().clone(),
            scene.start_relationship_level().clone(),
            scene.start_mood().clone(),
        );
        self.session_repo.create(&session).await?;

        // 7. Create opening messages (narrator + character dialogue)
        let narrator_message = Message::new(
            session.id().clone(),
            MessageRole::Character,
            scene.opening_narrator().to_string(),
            None,
        );
        let dialogue_message = Message::new(
            session.id().clone(),
            MessageRole::Character,
            scene.opening_dialogue().to_string(),
            None,
        );
        self.message_repo
            .create_many(&[narrator_message, dialogue_message])
            .await?;

        // 8a. Push scene opening card (hero image + narrator)
        let color_tone = roleplay_flex::extract_color_tone(scene.atmosphere());
        let session_flex = flex_messages::build_session_opening_flex(
            character.name().as_str(),
            scene.name().as_str(),
            scene.image_url(),
            scene.opening_narrator(),
            &color_tone,
        );

        if let Err(e) = self
            .line_client
            .push_messages(
                &input.line_user_id,
                vec![LineMessage::Flex {
                    alt_text: format!("เรื่องราวเริ่มต้นแล้ว - {}", scene.name().as_str()),
                    contents: session_flex,
                    sender_name: "TalkRai".into(),
                    sender_icon_url: String::new(),
                    // Opening card is the FIRST of two pushes — no quick reply here:
                    // LINE drops quick replies when a newer message enters the room,
                    // so they ride the opening dialogue (the last message) below.
                    quick_reply: None,
                }],
            )
            .await
        {
            tracing::warn!(
                error = %e,
                line_user_id = %input.line_user_id,
                "Failed to push session opening card"
            );
        }

        // 8b. Push opening dialogue (character message with avatar + action spans)
        let blocks = parse_text_into_blocks(scene.opening_dialogue());
        let bubble = roleplay_flex::build_roleplay_blocks_bubble(
            &blocks,
            scene.location(),
            scene.time_of_day(),
            &color_tone,
        );
        let alt_text = roleplay_flex::truncate_alt_text(scene.opening_dialogue());

        // Attach the scene's suggested first replies as a LINE quick reply on the
        // LAST opening message (this dialogue bubble). Empty column => None =>
        // normal push with no quick reply (fallback). The push boundary validates
        // each item against the LINE quick-reply spec before sending.
        let opening_quick_reply = derive_opening_quick_reply(scene.suggested_first_replies());

        if let Err(e) = self
            .line_client
            .push_messages(
                &input.line_user_id,
                vec![LineMessage::Flex {
                    alt_text,
                    contents: bubble,
                    sender_name: character.name().as_str().to_string(),
                    sender_icon_url: character.avatar_url().unwrap_or_default().to_string(),
                    quick_reply: opening_quick_reply,
                }],
            )
            .await
        {
            tracing::warn!(
                error = %e,
                line_user_id = %input.line_user_id,
                "Failed to push opening dialogue message"
            );
        }

        // 9. Switch rich menu to B (best-effort)
        if let Err(e) = self
            .line_client
            .link_rich_menu(&input.line_user_id, &input.rich_menu_b_id)
            .await
        {
            tracing::warn!(
                error = %e,
                line_user_id = %input.line_user_id,
                "Failed to link rich menu B after starting session"
            );
        }

        Ok(StartSessionOutput {
            session_id: *session.id().as_uuid(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{Character, RoleplaySession, Scene, User};
    use crate::domain::repositories::RepoError;
    use crate::domain::services::line_client::LineProfile;
    use crate::domain::services::LineClientError;
    use crate::domain::value_objects::{
        CharacterGender, CharacterId, CharacterMood, CharacterName, RelationshipLevel, SceneId,
        SceneName, SessionId, UserId, UserStatus,
    };
    use async_trait::async_trait;
    use chrono::Utc;
    use std::sync::Mutex;

    // ── derive_opening_quick_reply (pure) ──────────────────────

    #[test]
    fn derive_opening_quick_reply_empty_is_none() {
        assert_eq!(derive_opening_quick_reply(&[]), None);
    }

    #[test]
    fn derive_opening_quick_reply_nonempty_is_some() {
        let replies = vec!["สวัสดี".to_string(), "ทักทาย".to_string()];
        assert_eq!(derive_opening_quick_reply(&replies), Some(replies.clone()));
    }

    // ── Builders ───────────────────────────────────────────────

    fn active_user() -> User {
        User::from_existing(
            UserId::new(),
            "U_test".to_string(),
            "Tester".to_string(),
            None,
            "th".to_string(),
            UserStatus::Active,
            Some(Utc::now()), // terms already accepted -> skip update path
            Utc::now(),
            Utc::now(),
            0,
            0,
            None,
            None,
        )
    }

    fn test_character() -> Character {
        Character::from_existing(
            CharacterId::new(),
            CharacterName::from_trusted("มินะ".to_string()),
            "personality".to_string(),
            "speaking_style".to_string(),
            "background".to_string(),
            "system_prompt".to_string(),
            Some("https://avatar.test/img.webp".to_string()),
            None,
            vec!["cool".to_string()],
            vec!["tsundere".to_string()],
            CharacterGender::Female,
            true,
            Utc::now(),
            Utc::now(),
        )
    }

    fn test_scene(replies: Vec<String>) -> Scene {
        Scene::from_existing(
            SceneId::new(),
            CharacterId::new(),
            SceneName::from_trusted("ร้านกาแฟ".to_string()),
            "ร้านกาแฟริมทาง".to_string(),
            "เย็น".to_string(),
            "อบอุ่น สบายๆ".to_string(),
            "situation".to_string(),
            "*บรรยากาศร้านกาแฟยามเย็น*".to_string(),
            "สวัสดี วันนี้อยากดื่มอะไรดี".to_string(),
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

    // ── Mocks ──────────────────────────────────────────────────

    struct MockUserRepo;
    #[async_trait]
    impl UserRepository for MockUserRepo {
        async fn find_by_id(&self, _: &UserId) -> Result<Option<User>, RepoError> {
            Ok(None)
        }
        async fn find_by_line_user_id(&self, _: &str) -> Result<Option<User>, RepoError> {
            Ok(Some(active_user()))
        }
        async fn upsert(&self, _: &User) -> Result<(), RepoError> {
            Ok(())
        }
        async fn update(&self, _: &User) -> Result<(), RepoError> {
            Ok(())
        }
        async fn find_reengagement_targets(
            &self,
            _: chrono::NaiveDate,
            _: i64,
            _: i64,
        ) -> Result<Vec<crate::domain::repositories::ReengagementTarget>, RepoError> {
            Ok(vec![])
        }
        async fn mark_reminded(&self, _: &[UserId], _: chrono::NaiveDate) -> Result<(), RepoError> {
            Ok(())
        }
    }

    struct MockSessionRepo;
    #[async_trait]
    impl RoleplaySessionRepository for MockSessionRepo {
        async fn find_by_id(&self, _: &SessionId) -> Result<Option<RoleplaySession>, RepoError> {
            Ok(None)
        }
        async fn find_active_by_user_id(
            &self,
            _: &UserId,
        ) -> Result<Option<RoleplaySession>, RepoError> {
            Ok(None)
        }
        async fn count_by_user_id(&self, _: &UserId) -> Result<i64, RepoError> {
            Ok(0)
        }
        async fn create(&self, _: &RoleplaySession) -> Result<(), RepoError> {
            Ok(())
        }
        async fn update(&self, _: &RoleplaySession) -> Result<(), RepoError> {
            Ok(())
        }
    }

    struct MockSceneRepo {
        scene: Mutex<Option<Scene>>,
    }
    #[async_trait]
    impl SceneRepository for MockSceneRepo {
        async fn find_by_id(&self, _: &SceneId) -> Result<Option<Scene>, RepoError> {
            Ok(self.scene.lock().unwrap().take())
        }
        async fn find_by_character_id(&self, _: &CharacterId) -> Result<Vec<Scene>, RepoError> {
            Ok(vec![])
        }
        async fn find_default_by_character_id(
            &self,
            _: &CharacterId,
        ) -> Result<Option<Scene>, RepoError> {
            Ok(None)
        }
        async fn find_all_active(&self) -> Result<Vec<Scene>, RepoError> {
            Ok(vec![])
        }
    }

    struct MockCharacterRepo;
    #[async_trait]
    impl CharacterRepository for MockCharacterRepo {
        async fn find_by_id(&self, _: &CharacterId) -> Result<Option<Character>, RepoError> {
            Ok(Some(test_character()))
        }
        async fn find_active(&self) -> Result<Vec<Character>, RepoError> {
            Ok(vec![])
        }
    }

    struct MockMessageRepo;
    #[async_trait]
    impl MessageRepository for MockMessageRepo {
        async fn create(&self, _: &Message) -> Result<(), RepoError> {
            Ok(())
        }
        async fn create_many(&self, _: &[Message]) -> Result<(), RepoError> {
            Ok(())
        }
        async fn find_by_session_id(
            &self,
            _: &SessionId,
            _: i64,
        ) -> Result<Vec<Message>, RepoError> {
            Ok(vec![])
        }
        async fn count_by_user_id(&self, _: &UserId) -> Result<i64, RepoError> {
            Ok(0)
        }
    }

    /// Records the `quick_reply` field of every pushed message, in push order.
    struct RecordingLineClient {
        pushed: Mutex<Vec<Option<Vec<String>>>>,
    }
    impl RecordingLineClient {
        fn new() -> Self {
            Self {
                pushed: Mutex::new(Vec::new()),
            }
        }
    }
    #[async_trait]
    impl LineClient for RecordingLineClient {
        fn verify_signature(&self, _: &[u8], _: &str) -> Result<bool, LineClientError> {
            Ok(true)
        }
        async fn push_messages(
            &self,
            _: &str,
            messages: Vec<LineMessage>,
        ) -> Result<(), LineClientError> {
            let mut pushed = self.pushed.lock().unwrap();
            for m in &messages {
                match m {
                    LineMessage::Flex { quick_reply, .. } => pushed.push(quick_reply.clone()),
                    LineMessage::Text { .. } => pushed.push(None),
                }
            }
            Ok(())
        }
        async fn get_profile(&self, _: &str) -> Result<LineProfile, LineClientError> {
            unimplemented!()
        }
        async fn link_rich_menu(&self, _: &str, _: &str) -> Result<(), LineClientError> {
            Ok(())
        }
        async fn unlink_rich_menu(&self, _: &str) -> Result<(), LineClientError> {
            Ok(())
        }
        async fn show_loading_animation(
            &self,
            _: &str,
            _: Option<u32>,
        ) -> Result<(), LineClientError> {
            Ok(())
        }
        async fn verify_liff_token(&self, _: &str) -> Result<LineProfile, LineClientError> {
            unimplemented!()
        }
        async fn reply_messages(
            &self,
            _: &str,
            _: Vec<LineMessage>,
        ) -> Result<(), LineClientError> {
            Ok(())
        }
    }

    fn usecase(scene: Scene, line: Arc<RecordingLineClient>) -> StartSessionUseCase {
        StartSessionUseCase::new(
            Arc::new(MockUserRepo),
            Arc::new(MockSessionRepo),
            Arc::new(MockSceneRepo {
                scene: Mutex::new(Some(scene)),
            }),
            Arc::new(MockCharacterRepo),
            Arc::new(MockMessageRepo),
            line,
        )
    }

    fn input() -> StartSessionInput {
        StartSessionInput {
            line_user_id: "U_test".to_string(),
            scene_id: Uuid::new_v4(),
            rich_menu_b_id: "richmenu-b".to_string(),
        }
    }

    // ── Placement integration tests ────────────────────────────

    #[tokio::test]
    async fn attaches_quick_reply_to_last_opening_message_only() {
        let replies = vec!["อยากกาแฟ".to_string(), "ขอชาเย็น".to_string()];
        let scene = test_scene(replies.clone());
        let line = Arc::new(RecordingLineClient::new());

        usecase(scene, Arc::clone(&line))
            .execute(input())
            .await
            .expect("start session should succeed");

        let pushed = line.pushed.lock().unwrap();
        // Two opening pushes: [0] = opening card, [1] = opening dialogue (last).
        assert_eq!(pushed.len(), 2);
        assert_eq!(pushed[0], None, "opening card must NOT carry a quick reply");
        assert_eq!(
            pushed[1],
            Some(replies),
            "opening dialogue (last message) carries the quick reply"
        );
    }

    #[tokio::test]
    async fn empty_suggestions_pushes_with_no_quick_reply_and_no_error() {
        let scene = test_scene(vec![]);
        let line = Arc::new(RecordingLineClient::new());

        let result = usecase(scene, Arc::clone(&line)).execute(input()).await;

        assert!(result.is_ok(), "fallback path must not error");
        let pushed = line.pushed.lock().unwrap();
        assert_eq!(pushed.len(), 2);
        assert!(
            pushed.iter().all(|qr| qr.is_none()),
            "no message carries a quick reply when the column is empty"
        );
    }
}
