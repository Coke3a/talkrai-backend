use std::str::FromStr;
use std::sync::Arc;

use serde_json::json;

use crate::domain::entities::{Character, Job, Message, RoleplaySession, Scene};
use crate::domain::repositories::{
    AppConfigRepository, CharacterRepository, CreditRepository, JobRepository, MessageRepository,
    RoleplaySessionRepository, SceneRepository,
};
use crate::domain::services::ai_client::{
    AiClient, AiMessage, AiRoleplayRequest, AiSummaryRequest,
};
use crate::domain::services::line_client::{LineClient, LineMessage};
use crate::domain::value_objects::{CharacterMood, JobId, MessageRole, MessageType};
use crate::usecases::UsecaseError;

pub fn build_system_prompt(
    character: &Character,
    scene: &Scene,
    session: &RoleplaySession,
) -> String {
    let location = session
        .current_location()
        .unwrap_or_else(|| scene.location());
    let time = session.scene_time().unwrap_or_else(|| scene.time_of_day());

    let mut prompt = format!(
        r#"You are "{}".

{}

Personality: {}
Speaking style (voice anchor): {}
Background: {}

## Scene
{}
Atmosphere: {}
Location: {} | Time: {}"#,
        character.name().as_str(),
        character.system_prompt(),
        character.personality(),
        character.speaking_style(),
        character.background(),
        scene.situation_prompt(),
        scene.atmosphere(),
        location,
        time,
    );

    if let Some(summary) = session.scene_summary() {
        prompt.push_str(&format!("\nRecent events: {}", summary));
    }

    prompt.push_str(&format!(
        "\n\nMood: {} (shift naturally with events)\nRelationship [{}]: {}",
        session.mood().as_str(),
        session.relationship_level().as_str(),
        session.relationship_level().relationship_prompt_modifier(),
    ));

    prompt.push_str(
        r#"

## Output — valid JSON only, no other text
- narrator_text: start with "📍 location • 🕒 time\n", then 3rd-person scene narration
- character_text: in-character Thai dialogue matching your voice, mood & relationship
- mood: neutral|happy|sad|excited|angry|shy|playful|serious|worried
- scene_update: {"location":"…","time":"…","summary":"…"} or null

{"narrator_text":"","character_text":"","mood":"","scene_update":null}"#,
    );

    prompt
}

pub fn build_ai_messages(
    recent_messages: &[Message],
    current_user_message: &str,
) -> Vec<AiMessage> {
    let mut ai_messages = Vec::new();
    let mut i = 0;

    while i < recent_messages.len() {
        match recent_messages[i].role() {
            MessageRole::User => {
                ai_messages.push(AiMessage {
                    role: "user".to_string(),
                    content: recent_messages[i].content().to_string(),
                });
                i += 1;
            }
            MessageRole::Narrator => {
                let narrator_text = recent_messages[i].content().to_string();
                let character_text = if i + 1 < recent_messages.len()
                    && *recent_messages[i + 1].role() == MessageRole::Character
                {
                    i += 1;
                    recent_messages[i].content().to_string()
                } else {
                    String::new()
                };

                ai_messages.push(AiMessage {
                    role: "assistant".to_string(),
                    content: json!({
                        "narrator_text": narrator_text,
                        "character_text": character_text
                    })
                    .to_string(),
                });
                i += 1;
            }
            MessageRole::Character => {
                // Orphan character message (shouldn't happen normally)
                ai_messages.push(AiMessage {
                    role: "assistant".to_string(),
                    content: json!({
                        "narrator_text": "",
                        "character_text": recent_messages[i].content().to_string()
                    })
                    .to_string(),
                });
                i += 1;
            }
        }
    }

    // Append current user message
    ai_messages.push(AiMessage {
        role: "user".to_string(),
        content: current_user_message.to_string(),
    });

    ai_messages
}

// ---------------------------------------------------------------------------
// ProcessRoleplayMessageUseCase — execution engine for the roleplay pipeline
// ---------------------------------------------------------------------------

pub struct ProcessRoleplayMessageUseCase {
    job_repo: Arc<dyn JobRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    character_repo: Arc<dyn CharacterRepository>,
    scene_repo: Arc<dyn SceneRepository>,
    message_repo: Arc<dyn MessageRepository>,
    credit_repo: Arc<dyn CreditRepository>,
    ai_client: Arc<dyn AiClient>,
    line_client: Arc<dyn LineClient>,
    config_repo: Arc<dyn AppConfigRepository>,
}

struct ResolvedConfig {
    narrator_display_name: String,
    narrator_avatar_url: String,
    ai_max_tokens: u32,
    summarize_interval: Option<u32>,
}

pub struct ProcessRoleplayMessageInput {
    pub job_id: JobId,
}

impl ProcessRoleplayMessageUseCase {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        job_repo: Arc<dyn JobRepository>,
        session_repo: Arc<dyn RoleplaySessionRepository>,
        character_repo: Arc<dyn CharacterRepository>,
        scene_repo: Arc<dyn SceneRepository>,
        message_repo: Arc<dyn MessageRepository>,
        credit_repo: Arc<dyn CreditRepository>,
        ai_client: Arc<dyn AiClient>,
        line_client: Arc<dyn LineClient>,
        config_repo: Arc<dyn AppConfigRepository>,
    ) -> Self {
        Self {
            job_repo,
            session_repo,
            character_repo,
            scene_repo,
            message_repo,
            credit_repo,
            ai_client,
            line_client,
            config_repo,
        }
    }

    async fn resolve_config(&self) -> Result<ResolvedConfig, UsecaseError> {
        let keys = &[
            "narrator_display_name",
            "narrator_avatar_url",
            "ai_max_tokens",
            "summarize_interval",
        ];
        let map = self.config_repo.get_many(keys).await?;

        let summarize_interval = map
            .get("summarize_interval")
            .and_then(|v| v.parse::<u32>().ok());

        Ok(ResolvedConfig {
            narrator_display_name: map.get("narrator_display_name").cloned().ok_or_else(|| {
                UsecaseError::Infra(anyhow::anyhow!("Missing app_config: narrator_display_name"))
            })?,
            narrator_avatar_url: map.get("narrator_avatar_url").cloned().ok_or_else(|| {
                UsecaseError::Infra(anyhow::anyhow!("Missing app_config: narrator_avatar_url"))
            })?,
            ai_max_tokens: map
                .get("ai_max_tokens")
                .ok_or_else(|| {
                    UsecaseError::Infra(anyhow::anyhow!("Missing app_config: ai_max_tokens"))
                })?
                .parse::<u32>()
                .map_err(|e| {
                    UsecaseError::Infra(anyhow::anyhow!("Invalid app_config ai_max_tokens: {}", e))
                })?,
            summarize_interval,
        })
    }

    /// Wrapper: lock job, delegate to process_job, mark failed on error.
    pub async fn execute(&self, input: ProcessRoleplayMessageInput) -> Result<(), UsecaseError> {
        // 1. Lock job (SELECT FOR UPDATE SKIP LOCKED)
        let mut job = self
            .job_repo
            .lock_pending_job(&input.job_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Job not found or already locked".into()))?;

        job.lock()?;
        self.job_repo.update(&job).await?;

        // Delegate to inner pipeline — on error, mark job as failed
        match self.process_job(&mut job).await {
            Ok(()) => Ok(()),
            Err(e) => {
                tracing::error!(job_id = %job.id().as_uuid(), error = %e, "Job processing failed");
                let _ = job.fail(e.to_string());
                let _ = self.job_repo.update(&job).await;
                Err(e)
            }
        }
    }

    /// Inner pipeline — steps 2-12.
    async fn process_job(&self, job: &mut Job) -> Result<(), UsecaseError> {
        // 2. Load context
        let session_id = job
            .session_id()
            .ok_or_else(|| UsecaseError::Validation("Job has no session_id".into()))?;

        let session = self
            .session_repo
            .find_by_id(session_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Session not found".into()))?;

        // Parallel fetch: character, scene, messages (all depend on session but not each other)
        let (character_opt, scene_opt, recent_messages) = tokio::try_join!(
            async {
                self.character_repo
                    .find_by_id(session.character_id())
                    .await
                    .map_err(UsecaseError::from)
            },
            async {
                self.scene_repo
                    .find_by_id(session.scene_id())
                    .await
                    .map_err(UsecaseError::from)
            },
            async {
                self.message_repo
                    .find_by_session_id(session.id(), 20)
                    .await
                    .map_err(UsecaseError::from)
            },
        )?;

        let character =
            character_opt.ok_or_else(|| UsecaseError::NotFound("Character not found".into()))?;
        let scene = scene_opt.ok_or_else(|| UsecaseError::NotFound("Scene not found".into()))?;

        // 3. Check credits
        let credit_balance = self
            .credit_repo
            .find_balance_by_user_id(session.user_id())
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Credit balance not found".into()))?;

        if !credit_balance.has_sufficient_credits(1) {
            return Err(UsecaseError::InsufficientCredits);
        }

        // 4. Classify user input
        let user_message_type = MessageType::classify_user_input(job.user_message());

        // 5. Resolve config from DB
        let cfg = self.resolve_config().await?;

        // 6. Build prompt (call free functions)
        let system_prompt = build_system_prompt(&character, &scene, &session);
        let ai_messages = build_ai_messages(&recent_messages, job.user_message());

        // 7. Call AI
        let ai_response = self
            .ai_client
            .generate_roleplay_response(AiRoleplayRequest {
                system_prompt,
                messages: ai_messages,
                max_tokens: cfg.ai_max_tokens,
            })
            .await?;

        // 8. Save 3 messages: user, narrator, character
        let user_msg = Message::new(
            session.id().clone(),
            MessageRole::User,
            user_message_type,
            job.user_message().to_string(),
        );
        let narrator_msg = Message::new(
            session.id().clone(),
            MessageRole::Narrator,
            MessageType::Narration,
            ai_response.narrator_text.clone(),
        );
        let character_msg = Message::new(
            session.id().clone(),
            MessageRole::Character,
            MessageType::Dialogue,
            ai_response.character_text.clone(),
        );
        self.message_repo
            .create_many(&[user_msg, narrator_msg, character_msg])
            .await?;

        // 9. Deduct credit
        let mut balance = credit_balance;
        let transaction = balance.deduct(1, Some(*job.id().as_uuid()))?;
        self.credit_repo
            .deduct_and_log(session.user_id(), 1, &transaction)
            .await?;

        // 10. Update session
        let mut session = session;
        if let Some(mood_str) = &ai_response.mood {
            if let Ok(mood) = CharacterMood::from_str(mood_str) {
                session.update_mood(mood);
            }
        }
        if let Some(scene_update) = &ai_response.scene_update {
            session.update_scene_context(
                scene_update.location.clone(),
                scene_update.time.clone(),
                scene_update.summary.clone(),
            );
        }
        let level_up = session.increment_messages();
        self.session_repo.update(&session).await?;

        if let Some(new_level) = level_up {
            tracing::info!(
                session_id = %session.id().as_uuid(),
                new_level = new_level.as_str(),
                "Relationship level up"
            );
        }

        // 11. Push 2 LINE messages with Sender Override
        let narrator_line_msg = LineMessage {
            text: ai_response.narrator_text,
            sender_name: cfg.narrator_display_name,
            sender_icon_url: cfg.narrator_avatar_url,
        };
        let character_line_msg = LineMessage {
            text: ai_response.character_text,
            sender_name: character.name().as_str().to_string(),
            sender_icon_url: character.avatar_url().unwrap_or_default().to_string(),
        };
        self.line_client
            .push_messages(
                job.line_user_id(),
                vec![narrator_line_msg, character_line_msg],
            )
            .await?;

        // 12. Mark job completed
        job.complete()?;
        self.job_repo.update(job).await?;

        tracing::info!(
            job_id = %job.id().as_uuid(),
            session_id = %session.id().as_uuid(),
            "Roleplay message processed"
        );

        // 13. Background summarization (best-effort, after user gets response)
        if let Some(interval) = cfg.summarize_interval.filter(|&v| v > 0) {
            self.maybe_summarize(&session, interval).await;
        }

        Ok(())
    }

    /// Best-effort summarization of messages that are about to fall off the context window.
    async fn maybe_summarize(&self, session: &RoleplaySession, summarize_interval: u32) {
        let message_count = session.message_count() as u32;

        // Only trigger at interval boundaries
        if !message_count.is_multiple_of(summarize_interval) {
            return;
        }

        // No point summarizing if we haven't exceeded the context window yet
        if message_count * 3 <= 20 {
            return;
        }

        tracing::info!(
            session_id = %session.id().as_uuid(),
            message_count = message_count,
            "Triggering conversation summarization"
        );

        // Fetch 40 most recent messages
        let all_messages = match self.message_repo.find_by_session_id(session.id(), 40).await {
            Ok(msgs) => msgs,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to fetch messages for summarization");
                return;
            }
        };

        if all_messages.len() <= 20 {
            return;
        }

        // Messages that are "falling off" — older ones not in the recent 20
        let falling_off = &all_messages[..all_messages.len() - 20];

        // Convert to AiMessage format
        let ai_messages: Vec<AiMessage> = falling_off
            .iter()
            .map(|m| AiMessage {
                role: match m.role() {
                    MessageRole::User => "user".to_string(),
                    MessageRole::Narrator | MessageRole::Character => "assistant".to_string(),
                },
                content: m.content().to_string(),
            })
            .collect();

        let summary_request = AiSummaryRequest {
            existing_summary: session.scene_summary().map(|s| s.to_string()),
            messages_to_summarize: ai_messages,
            max_tokens: 300,
        };

        let new_summary = match self.ai_client.generate_summary(summary_request).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to generate summary");
                return;
            }
        };

        // Re-fetch session to avoid stale data
        let mut fresh_session = match self.session_repo.find_by_id(session.id()).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                tracing::warn!("Session not found during summarization");
                return;
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to re-fetch session for summarization");
                return;
            }
        };

        fresh_session.update_scene_summary(new_summary);

        if let Err(e) = self.session_repo.update(&fresh_session).await {
            tracing::warn!(error = %e, "Failed to save summary to session");
        } else {
            tracing::info!(
                session_id = %session.id().as_uuid(),
                "Conversation summary updated"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{Character, Message, RoleplaySession, Scene};
    use crate::domain::value_objects::*;

    fn test_character() -> Character {
        Character::from_existing(
            CharacterId::new(),
            CharacterName::from_trusted("มิโกะ".to_string()),
            "ร่าเริง สดใส พูดจาน่ารัก".to_string(),
            "พูดลงท้ายด้วย ~นะ ใช้คำน่ารัก".to_string(),
            "เด็กสาวขายขนมปังในหมู่บ้านเล็กๆ".to_string(),
            "คุณเป็นเด็กสาวขายขนมปัง ชื่อมิโกะ อายุ 18 ปี".to_string(),
            Some("https://example.com/miko.png".to_string()),
            vec!["slice-of-life".to_string()],
            true,
            chrono::Utc::now(),
            chrono::Utc::now(),
        )
    }

    fn test_scene() -> Scene {
        Scene::from_existing(
            SceneId::new(),
            CharacterId::new(),
            SceneName::from_trusted("ร้านขนมปัง".to_string()),
            "ร้านขนมปังเล็กๆ ริมถนน".to_string(),
            "เช้า".to_string(),
            "อบอุ่น หอมกลิ่นขนมปัง".to_string(),
            "คุณเดินเข้ามาในร้านขนมปัง".to_string(),
            "📍 ร้านขนมปังเล็กๆ • 🕒 เช้าตรู่\nกลิ่นขนมปังอบใหม่ลอยมา".to_string(),
            "สวัสดีค่า~ ยินดีต้อนรับนะคะ".to_string(),
            true,
            true,
            RelationshipLevel::Stranger,
            CharacterMood::Neutral,
            chrono::Utc::now(),
            chrono::Utc::now(),
        )
    }

    fn test_session() -> RoleplaySession {
        RoleplaySession::from_existing(
            SessionId::new(),
            UserId::new(),
            CharacterId::new(),
            SceneId::new(),
            SessionStatus::Active,
            CharacterMood::Happy,
            RelationshipLevel::Acquaintance,
            25,
            None,
            None,
            None,
            chrono::Utc::now(),
            chrono::Utc::now(),
        )
    }

    #[test]
    fn build_system_prompt_includes_all_fields() {
        let character = test_character();
        let scene = test_scene();
        let session = test_session();

        let prompt = build_system_prompt(&character, &scene, &session);

        assert!(prompt.contains("มิโกะ"));
        assert!(prompt.contains("คุณเป็นเด็กสาวขายขนมปัง"));
        assert!(prompt.contains("ร่าเริง สดใส"));
        assert!(prompt.contains("พูดลงท้ายด้วย ~นะ"));
        assert!(prompt.contains("เด็กสาวขายขนมปังในหมู่บ้านเล็กๆ"));
        assert!(prompt.contains("คุณเดินเข้ามาในร้านขนมปัง"));
        assert!(prompt.contains("อบอุ่น หอมกลิ่นขนมปัง"));
        assert!(prompt.contains("happy"));
        assert!(prompt.contains("acquaintance"));
        assert!(prompt.contains("เริ่มแซว ตั้งชื่อเล่น"));
    }

    #[test]
    fn build_system_prompt_uses_session_location_over_scene() {
        let character = test_character();
        let scene = test_scene();
        let session = RoleplaySession::from_existing(
            SessionId::new(),
            UserId::new(),
            CharacterId::new(),
            SceneId::new(),
            SessionStatus::Active,
            CharacterMood::Neutral,
            RelationshipLevel::Stranger,
            5,
            Some("สวนสาธารณะ".to_string()),
            Some("เย็น".to_string()),
            Some("เดินออกจากร้าน".to_string()),
            chrono::Utc::now(),
            chrono::Utc::now(),
        );

        let prompt = build_system_prompt(&character, &scene, &session);

        assert!(prompt.contains("Location: สวนสาธารณะ"));
        assert!(prompt.contains("Time: เย็น"));
        assert!(prompt.contains("Recent events: เดินออกจากร้าน"));
        assert!(!prompt.contains("ร้านขนมปังเล็กๆ ริมถนน"));
    }

    #[test]
    fn build_system_prompt_falls_back_to_scene_location() {
        let character = test_character();
        let scene = test_scene();
        let session = test_session(); // no current_location/scene_time

        let prompt = build_system_prompt(&character, &scene, &session);

        assert!(prompt.contains("Location: ร้านขนมปังเล็กๆ ริมถนน"));
        assert!(prompt.contains("Time: เช้า"));
    }

    #[test]
    fn build_system_prompt_has_no_memory_section() {
        let character = test_character();
        let scene = test_scene();
        let session = test_session();

        let prompt = build_system_prompt(&character, &scene, &session);

        assert!(!prompt.contains("## Memories"));
    }

    #[test]
    fn build_ai_messages_maps_user_role() {
        let messages = vec![Message::from_existing(
            MessageId::new(),
            SessionId::new(),
            MessageRole::User,
            MessageType::Dialogue,
            "สวัสดี".to_string(),
            chrono::Utc::now(),
        )];

        let result = build_ai_messages(&messages, "ขอขนมปังหน่อย");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].role, "user");
        assert_eq!(result[0].content, "สวัสดี");
        assert_eq!(result[1].role, "user");
        assert_eq!(result[1].content, "ขอขนมปังหน่อย");
    }

    #[test]
    fn build_ai_messages_combines_narrator_and_character() {
        let session_id = SessionId::new();
        let messages = vec![
            Message::from_existing(
                MessageId::new(),
                session_id.clone(),
                MessageRole::Narrator,
                MessageType::Dialogue,
                "📍 ร้านกาแฟ • 🕒 บ่าย".to_string(),
                chrono::Utc::now(),
            ),
            Message::from_existing(
                MessageId::new(),
                session_id,
                MessageRole::Character,
                MessageType::Dialogue,
                "สวัสดีค่า~".to_string(),
                chrono::Utc::now(),
            ),
        ];

        let result = build_ai_messages(&messages, "ว่าไง");

        assert_eq!(result.len(), 2); // 1 combined assistant + 1 current user
        assert_eq!(result[0].role, "assistant");

        let parsed: serde_json::Value = serde_json::from_str(&result[0].content).unwrap();
        assert_eq!(parsed["narrator_text"], "📍 ร้านกาแฟ • 🕒 บ่าย");
        assert_eq!(parsed["character_text"], "สวัสดีค่า~");
    }

    #[test]
    fn build_ai_messages_handles_orphan_narrator() {
        let messages = vec![Message::from_existing(
            MessageId::new(),
            SessionId::new(),
            MessageRole::Narrator,
            MessageType::Dialogue,
            "📍 ร้านกาแฟ".to_string(),
            chrono::Utc::now(),
        )];

        let result = build_ai_messages(&messages, "test");

        assert_eq!(result[0].role, "assistant");
        let parsed: serde_json::Value = serde_json::from_str(&result[0].content).unwrap();
        assert_eq!(parsed["narrator_text"], "📍 ร้านกาแฟ");
        assert_eq!(parsed["character_text"], "");
    }

    #[test]
    fn build_ai_messages_handles_orphan_character() {
        let messages = vec![Message::from_existing(
            MessageId::new(),
            SessionId::new(),
            MessageRole::Character,
            MessageType::Dialogue,
            "สวัสดีค่า~".to_string(),
            chrono::Utc::now(),
        )];

        let result = build_ai_messages(&messages, "test");

        assert_eq!(result[0].role, "assistant");
        let parsed: serde_json::Value = serde_json::from_str(&result[0].content).unwrap();
        assert_eq!(parsed["narrator_text"], "");
        assert_eq!(parsed["character_text"], "สวัสดีค่า~");
    }

    #[test]
    fn build_ai_messages_full_conversation_flow() {
        let session_id = SessionId::new();
        let base_time = chrono::Utc::now();
        let messages = vec![
            Message::from_existing(
                MessageId::new(),
                session_id.clone(),
                MessageRole::User,
                MessageType::Dialogue,
                "สวัสดี".to_string(),
                base_time,
            ),
            Message::from_existing(
                MessageId::new(),
                session_id.clone(),
                MessageRole::Narrator,
                MessageType::Dialogue,
                "📍 ร้านขนมปัง • 🕒 เช้า".to_string(),
                base_time,
            ),
            Message::from_existing(
                MessageId::new(),
                session_id.clone(),
                MessageRole::Character,
                MessageType::Dialogue,
                "สวัสดีค่า~ ยินดีต้อนรับนะคะ".to_string(),
                base_time,
            ),
            Message::from_existing(
                MessageId::new(),
                session_id,
                MessageRole::User,
                MessageType::Dialogue,
                "มีขนมปังอะไรบ้าง".to_string(),
                base_time,
            ),
        ];

        let result = build_ai_messages(&messages, "ขอครัวซองต์");

        assert_eq!(result.len(), 4);
        assert_eq!(result[0].role, "user");
        assert_eq!(result[0].content, "สวัสดี");
        assert_eq!(result[1].role, "assistant");
        assert_eq!(result[2].role, "user");
        assert_eq!(result[2].content, "มีขนมปังอะไรบ้าง");
        assert_eq!(result[3].role, "user");
        assert_eq!(result[3].content, "ขอครัวซองต์");
    }

    #[test]
    fn build_system_prompt_includes_json_format() {
        let character = test_character();
        let scene = test_scene();
        let session = test_session();

        let prompt = build_system_prompt(&character, &scene, &session);

        assert!(prompt.contains("narrator_text"));
        assert!(prompt.contains("character_text"));
        assert!(prompt.contains("scene_update"));
        assert!(prompt.contains("valid JSON only"));
    }
}
