use std::sync::Arc;

use uuid::Uuid;

use crate::domain::repositories::{
    CharacterRepository, JobRepository, RoleplaySessionRepository, SceneRepository, UserRepository,
};
use crate::domain::services::line_client::{LineClient, LineMessage};
use crate::infra::line::flex_messages;
use crate::usecases::liff::require_active_user::require_active_user;
use crate::usecases::UsecaseError;

pub struct EndSessionInput {
    pub line_user_id: String,
    pub rich_menu_a_id: String,
    pub liff_base_url: String,
}

pub struct EndSessionOutput {
    pub session_id: Uuid,
}

pub struct EndSessionUseCase {
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    job_repo: Arc<dyn JobRepository>,
    scene_repo: Arc<dyn SceneRepository>,
    character_repo: Arc<dyn CharacterRepository>,
    line_client: Arc<dyn LineClient>,
}

impl EndSessionUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn RoleplaySessionRepository>,
        job_repo: Arc<dyn JobRepository>,
        scene_repo: Arc<dyn SceneRepository>,
        character_repo: Arc<dyn CharacterRepository>,
        line_client: Arc<dyn LineClient>,
    ) -> Self {
        Self {
            user_repo,
            session_repo,
            job_repo,
            scene_repo,
            character_repo,
            line_client,
        }
    }

    pub async fn execute(&self, input: EndSessionInput) -> Result<EndSessionOutput, UsecaseError> {
        // 1. Find user
        let user = require_active_user(&*self.user_repo, &input.line_user_id).await?;

        // 2. Find active session
        let mut session = self
            .session_repo
            .find_active_by_user_id(user.id())
            .await?
            .ok_or_else(|| UsecaseError::NotFound("No active session found".into()))?;

        // A reply (web or LINE) may be in flight on this story; ending it now would trip the DB guard.
        if self
            .job_repo
            .has_active_job_for_session(session.id())
            .await?
        {
            return Err(UsecaseError::Validation(
                super::REPLY_IN_FLIGHT_MESSAGE.into(),
            ));
        }

        // 3. End session (domain state transition: Active → Ended)
        session.end()?;
        self.session_repo
            .update(&session)
            .await
            .map_err(super::story_update_error)?;

        // 4. Fetch character + scene for Flex notification (best-effort)
        let flex_result: Option<serde_json::Value> = async {
            let character = self
                .character_repo
                .find_by_id(session.character_id())
                .await
                .ok()??;
            let scene = self
                .scene_repo
                .find_by_id(session.scene_id())
                .await
                .ok()??;
            let scenes_url = format!("{}/scenes", input.liff_base_url);
            Some(flex_messages::build_session_ended_flex(
                character.name().as_str(),
                scene.name().as_str(),
                session.message_count(),
                &scenes_url,
            ))
        }
        .await;

        // 5. Push "Session Ended" Flex (best-effort)
        if let Some(flex_contents) = flex_result {
            if let Err(e) = self
                .line_client
                .push_messages(
                    &input.line_user_id,
                    vec![LineMessage::Flex {
                        alt_text: "เรื่องราวจบลงแล้ว".into(),
                        contents: flex_contents,
                        sender_name: "TalkRai".into(),
                        sender_icon_url: String::new(),
                        quick_reply: None,
                    }],
                )
                .await
            {
                tracing::warn!(
                    error = %e,
                    line_user_id = %input.line_user_id,
                    "Failed to push session ended flex"
                );
            }
        } else {
            tracing::warn!(
                line_user_id = %input.line_user_id,
                "Could not fetch character/scene for session ended flex, skipping"
            );
        }

        // 6. Switch rich menu back to A (best-effort)
        if let Err(e) = self
            .line_client
            .link_rich_menu(&input.line_user_id, &input.rich_menu_a_id)
            .await
        {
            tracing::warn!(
                error = %e,
                line_user_id = %input.line_user_id,
                "Failed to link rich menu A after ending session"
            );
        }

        Ok(EndSessionOutput {
            session_id: *session.id().as_uuid(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{Character, Job, RoleplaySession, Scene, User};
    use crate::domain::repositories::RepoError;
    use crate::domain::services::line_client::LineProfile;
    use crate::domain::services::LineClientError;
    use crate::domain::value_objects::{
        CharacterId, CharacterMood, JobId, RelationshipLevel, SceneId, SessionId, UserId,
        UserStatus,
    };
    use async_trait::async_trait;
    use chrono::Utc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    struct MockUserRepo;
    #[async_trait]
    impl UserRepository for MockUserRepo {
        async fn find_by_id(&self, _: &UserId) -> Result<Option<User>, RepoError> {
            Ok(None)
        }
        async fn find_by_line_user_id(&self, _: &str) -> Result<Option<User>, RepoError> {
            Ok(Some(User::from_existing(
                UserId::new(),
                "U_test".to_string(),
                "Tester".to_string(),
                None,
                "th".to_string(),
                UserStatus::Active,
                Some(Utc::now()),
                Utc::now(),
                Utc::now(),
                0,
                0,
                None,
                None,
            )))
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

    struct MockSessionRepo {
        active: Mutex<Option<RoleplaySession>>,
        updates: AtomicUsize,
    }
    #[async_trait]
    impl RoleplaySessionRepository for MockSessionRepo {
        async fn find_by_id(&self, _: &SessionId) -> Result<Option<RoleplaySession>, RepoError> {
            Ok(None)
        }
        async fn find_active_by_user_id(
            &self,
            _: &UserId,
        ) -> Result<Option<RoleplaySession>, RepoError> {
            Ok(self.active.lock().unwrap().take())
        }
        async fn count_by_user_id(&self, _: &UserId) -> Result<i64, RepoError> {
            Ok(0)
        }
        async fn create(&self, _: &RoleplaySession) -> Result<(), RepoError> {
            unimplemented!()
        }
        async fn update(&self, _: &RoleplaySession) -> Result<(), RepoError> {
            self.updates.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct MockJobRepo {
        active: bool,
    }
    #[async_trait]
    impl JobRepository for MockJobRepo {
        async fn create(&self, _: &Job) -> Result<(), RepoError> {
            unimplemented!()
        }
        async fn create_many(&self, _: &[Job]) -> Result<(), RepoError> {
            unimplemented!()
        }
        async fn find_by_id(&self, _: &JobId) -> Result<Option<Job>, RepoError> {
            unimplemented!()
        }
        async fn lock_pending_job(&self, _: &JobId) -> Result<Option<Job>, RepoError> {
            unimplemented!()
        }
        async fn mark_processing_if_pending(&self, _: &Job) -> Result<bool, RepoError> {
            unimplemented!()
        }
        async fn find_and_lock_pending_jobs(&self, _: i64) -> Result<Vec<Job>, RepoError> {
            unimplemented!()
        }
        async fn find_stale_processing_jobs(&self, _: i64) -> Result<Vec<Job>, RepoError> {
            unimplemented!()
        }
        async fn has_active_job_for_session(&self, _: &SessionId) -> Result<bool, RepoError> {
            Ok(self.active)
        }
        async fn update(&self, _: &Job) -> Result<(), RepoError> {
            unimplemented!()
        }
    }

    struct MockSceneRepo;
    #[async_trait]
    impl SceneRepository for MockSceneRepo {
        async fn find_by_id(&self, _: &SceneId) -> Result<Option<Scene>, RepoError> {
            Ok(None)
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
            Ok(None)
        }
        async fn find_active(&self) -> Result<Vec<Character>, RepoError> {
            Ok(vec![])
        }
    }

    struct MockLineClient;
    #[async_trait]
    impl LineClient for MockLineClient {
        fn verify_signature(&self, _: &[u8], _: &str) -> Result<bool, LineClientError> {
            Ok(true)
        }
        async fn push_messages(&self, _: &str, _: Vec<LineMessage>) -> Result<(), LineClientError> {
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

    fn run(job_active: bool) -> (Arc<MockSessionRepo>, EndSessionUseCase) {
        let sessions = Arc::new(MockSessionRepo {
            active: Mutex::new(Some(RoleplaySession::new(
                UserId::new(),
                CharacterId::new(),
                SceneId::new(),
                RelationshipLevel::Stranger,
                CharacterMood::Neutral,
            ))),
            updates: AtomicUsize::new(0),
        });
        let usecase = EndSessionUseCase::new(
            Arc::new(MockUserRepo),
            sessions.clone(),
            Arc::new(MockJobRepo { active: job_active }),
            Arc::new(MockSceneRepo),
            Arc::new(MockCharacterRepo),
            Arc::new(MockLineClient),
        );
        (sessions, usecase)
    }

    fn input() -> EndSessionInput {
        EndSessionInput {
            line_user_id: "U_test".to_string(),
            rich_menu_a_id: "richmenu-a".to_string(),
            liff_base_url: "https://liff.test".to_string(),
        }
    }

    #[tokio::test]
    async fn rejects_when_story_has_job_in_flight() {
        let (sessions, usecase) = run(true);

        let result = usecase.execute(input()).await;

        assert!(matches!(result, Err(UsecaseError::Validation(_))));
        assert_eq!(
            sessions.updates.load(Ordering::SeqCst),
            0,
            "story must not be ended"
        );
    }

    #[tokio::test]
    async fn ends_story_when_no_job_in_flight() {
        let (sessions, usecase) = run(false);

        usecase.execute(input()).await.expect("end should succeed");

        assert_eq!(sessions.updates.load(Ordering::SeqCst), 1);
    }
}
