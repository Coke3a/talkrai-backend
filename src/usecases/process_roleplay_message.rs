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
use crate::domain::value_objects::{CharacterMood, JobId, MessageRole, RelationshipThresholds};
use crate::infra::line::roleplay_flex;
use crate::usecases::UsecaseError;

/// Convert JSON atmosphere blob to compact readable text.
/// Plain text passes through unchanged.
pub fn compact_atmosphere(atmosphere: &str) -> String {
    let trimmed = atmosphere.trim();
    if !trimmed.starts_with('{') {
        return atmosphere.to_string();
    }

    let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return atmosphere.to_string();
    };

    let mut parts: Vec<String> = Vec::new();

    // mood
    if let Some(mood) = val.get("mood").and_then(|v| v.as_str()) {
        parts.push(mood.to_string());
    }

    // tags
    if let Some(tags) = val.get("tags").and_then(|v| v.as_array()) {
        let tag_strs: Vec<&str> = tags.iter().filter_map(|t| t.as_str()).collect();
        if !tag_strs.is_empty() {
            parts.push(tag_strs.join(", "));
        }
    }

    // sensory values
    if let Some(sensory) = val.get("sensory").and_then(|v| v.as_object()) {
        for (_key, v) in sensory {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    parts.push(s.to_string());
                }
            }
        }
    }

    if parts.is_empty() {
        atmosphere.to_string()
    } else {
        parts.join(" | ")
    }
}

/// Extract `color_tone` field from a JSON atmosphere string. Falls back to "neutral".
fn extract_color_tone(atmosphere: &str) -> String {
    let trimmed = atmosphere.trim();
    if !trimmed.starts_with('{') {
        return "neutral".to_string();
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|v| v.get("color_tone")?.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "neutral".to_string())
}

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
        "You are \"{}\" ({}).\n\n{}",
        character.name().as_str(),
        character.gender().as_str(),
        character.system_prompt(),
    );

    prompt.push_str(&format!(
        "\n\nPersonality: {}\nSpeaking style: {}\nBackground: {}",
        character.personality(),
        character.speaking_style(),
        character.background(),
    ));

    let atmosphere = compact_atmosphere(scene.atmosphere());
    prompt.push_str(&format!(
        "\n\n[Scene] {}\n{} | {} | {}",
        scene.situation_prompt(),
        location,
        time,
        atmosphere,
    ));

    if let Some(summary) = session.scene_summary() {
        prompt.push_str(&format!("\nRecent: {}", summary));
    }

    prompt.push_str(&format!(
        "\n\nMood: {} | Relationship: {} — {}",
        session.mood().as_str(),
        session.relationship_level().as_str(),
        session.relationship_level().relationship_prompt_modifier(),
    ));

    prompt.push_str(
        r#"

## Writing Style
- เขียนแบบนิยายไทย สลับบรรยายกับบทพูดได้อิสระตามธรรมชาติ
- narration: ร้อยแก้วบรรยายฉาก การกระทำ อารมณ์ ใช้ sensory details (แสง สี เสียง กลิ่น สัมผัส)
- dialogue: บทพูดตัวละครในเครื่องหมายคำพูด ("...") สะท้อน speaking_style
- ใช้หลัก Show Don't Tell — บรรยายผ่านร่างกาย (มือสั่น หายใจหนัก หัวใจเต้น) แทนบอกตรงๆ
- ใช้อุปมาอุปไมย ("ราวกับ..." "ดุจ...")
- narration block: 2-6 ประโยค, dialogue block: 1-3 ประโยค
- จบด้วย dialogue หรือ narration ที่ค้างไว้ ให้ user อยากตอบ

## ความยาว
- ฉากเข้มข้น/ดราม่า: 4-8 blocks, 200-400 คำ
- ฉากโรแมนซ์: 3-6 blocks, 150-300 คำ
- ฉากสบายๆ: 2-4 blocks, 80-150 คำ

## Response Format
Reply with ONLY a JSON object using blocks array:
- mood is REQUIRED — always include mood in every response
- min 2 blocks, min 1 narration, start with narration
- no 3+ consecutive dialogue blocks
- vary block pattern creatively — DO NOT always use N→D→N→D
  - N→D→N: จบด้วย narration สร้างบรรยากาศค้าง
  - N→D→N→D→N: เล่าเรื่องยาว จบด้วย narration ให้จินตนาการ
  - N→D: สั้นกระชับ ตอบเร็ว
  - N→D→N→D: สลับปกติ
- mood: neutral|happy|sad|excited|angry|shy|playful|serious|worried

{"blocks":[...],"mood":"<REQUIRED>","scene_update":null}

## Examples

User: สวัสดี มีขนมปังอะไรบ้าง
{"blocks":[{"type":"narration","text":"เสียงกระดิ่งเล็กๆ ดังกริ๊งเบาๆ เมื่อประตูร้านถูกผลักเปิดออก กลิ่นขนมปังอบใหม่ลอยมาต้อนรับ ราวกับอ้อมแขนที่อบอุ่น"},{"type":"dialogue","text":"\"สวัสดีค่า~ วันนี้มีครัวซองต์เนยสด กับชิอาบัตตาหน้าอโวคาโดนะคะ\""},{"type":"narration","text":"เธอยิ้มพลางชี้ไปที่ตะกร้าหวายบนเคาน์เตอร์ ที่ขนมปังสีน้ำตาลทองเรียงตัวกันอย่างน่ารัก ไอความร้อนยังลอยเป็นสายบางๆ"}],"mood":"happy","scene_update":null}

User: *นั่งเงียบๆ ไม่พูดอะไร*
{"blocks":[{"type":"narration","text":"เสียงเก้าอี้ถูกดึงออกดังแผ่วเบา แสงบ่ายทอดเงายาวผ่านกระจก"},{"type":"dialogue","text":"\"น้ำค่ะ... ดื่มก่อนนะคะ\""},{"type":"narration","text":"เธอวางแก้วน้ำลงตรงหน้าอย่างเบามือ รอยยิ้มบางๆ ผุดขึ้นที่มุมปากก่อนหันกลับไปเช็ดเคาน์เตอร์ต่อ"},{"type":"dialogue","text":"\"ถ้าอยากได้อะไร... บอกได้นะคะ\""},{"type":"narration","text":"เสียงเพลงแจ๊สเบาๆ ไหลแทรกเข้ามาแทนที่บทสนทนา กลิ่นกาแฟคั่วลอยอ้อยอิ่งอยู่ในอากาศ"}],"mood":"worried","scene_update":null}"#,
    );

    prompt
}

/// Wrap character message content for AI conversation format.
/// JSON blocks array → {"blocks": [...]}
/// Legacy plain text → {"blocks": [{"type":"dialogue","text":"..."}]}
fn wrap_character_content(raw: &str) -> String {
    if raw.trim_start().starts_with('[') {
        format!(r#"{{"blocks":{}}}"#, raw)
    } else {
        json!({"blocks": [{"type": "dialogue", "text": raw}]}).to_string()
    }
}

pub fn build_ai_messages(
    recent_messages: &[Message],
    current_user_message: &str,
) -> Vec<AiMessage> {
    let mut ai_messages = Vec::new();

    for msg in recent_messages {
        match msg.role() {
            MessageRole::User => {
                ai_messages.push(AiMessage {
                    role: "user".to_string(),
                    content: msg.content().to_string(),
                });
            }
            MessageRole::Character => {
                ai_messages.push(AiMessage {
                    role: "assistant".to_string(),
                    content: wrap_character_content(msg.content()),
                });
            }
        }
    }

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
    ai_max_tokens: u32,
    summarize_interval: Option<u32>,
    relationship_thresholds: RelationshipThresholds,
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
            "ai_max_tokens",
            "summarize_interval",
            "relationship_threshold_acquaintance",
            "relationship_threshold_friend",
            "relationship_threshold_close_friend",
        ];
        let map = self.config_repo.get_many(keys).await?;

        let summarize_interval = map
            .get("summarize_interval")
            .and_then(|v| v.parse::<u32>().ok());

        let threshold_acquaintance = map
            .get("relationship_threshold_acquaintance")
            .ok_or_else(|| {
                UsecaseError::Infra(anyhow::anyhow!(
                    "Missing app_config: relationship_threshold_acquaintance"
                ))
            })?
            .parse::<u32>()
            .map_err(|e| {
                UsecaseError::Infra(anyhow::anyhow!(
                    "Invalid app_config relationship_threshold_acquaintance: {}",
                    e
                ))
            })?;

        let threshold_friend = map
            .get("relationship_threshold_friend")
            .ok_or_else(|| {
                UsecaseError::Infra(anyhow::anyhow!(
                    "Missing app_config: relationship_threshold_friend"
                ))
            })?
            .parse::<u32>()
            .map_err(|e| {
                UsecaseError::Infra(anyhow::anyhow!(
                    "Invalid app_config relationship_threshold_friend: {}",
                    e
                ))
            })?;

        let threshold_close_friend = map
            .get("relationship_threshold_close_friend")
            .ok_or_else(|| {
                UsecaseError::Infra(anyhow::anyhow!(
                    "Missing app_config: relationship_threshold_close_friend"
                ))
            })?
            .parse::<u32>()
            .map_err(|e| {
                UsecaseError::Infra(anyhow::anyhow!(
                    "Invalid app_config relationship_threshold_close_friend: {}",
                    e
                ))
            })?;

        let relationship_thresholds = RelationshipThresholds::new(
            threshold_acquaintance,
            threshold_friend,
            threshold_close_friend,
        )
        .map_err(|e| {
            UsecaseError::Infra(anyhow::anyhow!("Invalid relationship thresholds: {}", e))
        })?;

        Ok(ResolvedConfig {
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
            relationship_thresholds,
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

        // 4. Resolve config from DB
        let cfg = self.resolve_config().await?;

        // 5. Build prompt (call free functions)
        let system_prompt = build_system_prompt(&character, &scene, &session);
        let ai_messages = build_ai_messages(&recent_messages, job.user_message());

        // 6. Call AI
        let ai_response = self
            .ai_client
            .generate_roleplay_response(AiRoleplayRequest {
                system_prompt,
                messages: ai_messages,
                max_tokens: cfg.ai_max_tokens,
            })
            .await?;

        // 7. Save messages: user + character (blocks JSON + mood)
        let user_msg = Message::new(
            session.id().clone(),
            MessageRole::User,
            job.user_message().to_string(),
            None,
        );
        let character_msg = Message::new(
            session.id().clone(),
            MessageRole::Character,
            ai_response.blocks_json(),
            ai_response.mood.clone(),
        );
        self.message_repo
            .create_many(&[user_msg, character_msg])
            .await?;

        // 8. Deduct credit
        let mut balance = credit_balance;
        let transaction = balance.deduct(1, Some(*job.id().as_uuid()))?;
        self.credit_repo
            .deduct_and_log(session.user_id(), 1, &transaction)
            .await?;

        // 9. Update session
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
        let level_up = session.increment_messages(&cfg.relationship_thresholds);
        self.session_repo.update(&session).await?;

        if let Some(new_level) = level_up {
            tracing::info!(
                session_id = %session.id().as_uuid(),
                new_level = new_level.as_str(),
                "Relationship level up"
            );
        }

        // 10. Push LINE message: single Flex bubble with character sender
        if !ai_response.blocks.is_empty() {
            let location = session
                .current_location()
                .unwrap_or_else(|| scene.location());
            let time_of_day = session.scene_time().unwrap_or_else(|| scene.time_of_day());
            let color_tone = extract_color_tone(scene.atmosphere());

            let bubble = roleplay_flex::build_roleplay_blocks_bubble(
                &ai_response.blocks,
                location,
                time_of_day,
                &color_tone,
            );

            let alt_source = &ai_response.blocks[0].text;

            let line_messages = vec![LineMessage::Flex {
                alt_text: roleplay_flex::truncate_alt_text(alt_source),
                contents: bubble,
                sender_name: character.name().as_str().to_string(),
                sender_icon_url: character.avatar_url().unwrap_or_default().to_string(),
            }];

            self.line_client
                .push_messages(job.line_user_id(), line_messages)
                .await?;
        }

        // 11. Mark job completed
        job.complete()?;
        self.job_repo.update(job).await?;

        tracing::info!(
            job_id = %job.id().as_uuid(),
            session_id = %session.id().as_uuid(),
            "Roleplay message processed"
        );

        // 12. Background summarization (best-effort, after user gets response)
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
                    MessageRole::Character => "assistant".to_string(),
                },
                content: match m.role() {
                    MessageRole::User => m.content().to_string(),
                    MessageRole::Character => wrap_character_content(m.content()),
                },
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
            CharacterGender::Female,
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
            false,
            RelationshipLevel::Stranger,
            CharacterMood::Neutral,
            None,
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
        assert!(prompt.contains("(female)"));
        assert!(prompt.contains("คุณเป็นเด็กสาวขายขนมปัง"));
        // Simple character → personality/style/background included
        assert!(prompt.contains("Personality: ร่าเริง สดใส"));
        assert!(prompt.contains("Speaking style: พูดลงท้ายด้วย ~นะ"));
        assert!(prompt.contains("Background: เด็กสาวขายขนมปังในหมู่บ้านเล็กๆ"));
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

        assert!(prompt.contains("สวนสาธารณะ"));
        assert!(prompt.contains("เย็น"));
        assert!(prompt.contains("Recent: เดินออกจากร้าน"));
        assert!(!prompt.contains("ร้านขนมปังเล็กๆ ริมถนน"));
    }

    #[test]
    fn build_system_prompt_falls_back_to_scene_location() {
        let character = test_character();
        let scene = test_scene();
        let session = test_session(); // no current_location/scene_time

        let prompt = build_system_prompt(&character, &scene, &session);

        assert!(prompt.contains("ร้านขนมปังเล็กๆ ริมถนน"));
        assert!(prompt.contains("เช้า"));
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
            "สวัสดี".to_string(),
            None,
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
    fn build_ai_messages_character_json_blocks() {
        let session_id = SessionId::new();
        let blocks_json = r#"[{"type":"narration","text":"📍 ร้านกาแฟ • 🕒 บ่าย"},{"type":"dialogue","text":"สวัสดีค่า~"}]"#;
        let messages = vec![Message::from_existing(
            MessageId::new(),
            session_id,
            MessageRole::Character,
            blocks_json.to_string(),
            Some("happy".to_string()),
            chrono::Utc::now(),
        )];

        let result = build_ai_messages(&messages, "ว่าไง");

        assert_eq!(result.len(), 2); // 1 assistant + 1 current user
        assert_eq!(result[0].role, "assistant");

        let parsed: serde_json::Value = serde_json::from_str(&result[0].content).unwrap();
        let blocks = parsed["blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["type"], "narration");
        assert_eq!(blocks[0]["text"], "📍 ร้านกาแฟ • 🕒 บ่าย");
        assert_eq!(blocks[1]["type"], "dialogue");
        assert_eq!(blocks[1]["text"], "สวัสดีค่า~");
    }

    #[test]
    fn build_ai_messages_legacy_plain_text_character() {
        let messages = vec![Message::from_existing(
            MessageId::new(),
            SessionId::new(),
            MessageRole::Character,
            "สวัสดีค่า~".to_string(),
            None,
            chrono::Utc::now(),
        )];

        let result = build_ai_messages(&messages, "test");

        assert_eq!(result[0].role, "assistant");
        let parsed: serde_json::Value = serde_json::from_str(&result[0].content).unwrap();
        let blocks = parsed["blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "dialogue");
        assert_eq!(blocks[0]["text"], "สวัสดีค่า~");
    }

    #[test]
    fn build_ai_messages_full_conversation_flow() {
        let session_id = SessionId::new();
        let base_time = chrono::Utc::now();
        let blocks_json = r#"[{"type":"narration","text":"📍 ร้านขนมปัง • 🕒 เช้า"},{"type":"dialogue","text":"สวัสดีค่า~ ยินดีต้อนรับนะคะ"}]"#;
        let messages = vec![
            Message::from_existing(
                MessageId::new(),
                session_id.clone(),
                MessageRole::User,
                "สวัสดี".to_string(),
                None,
                base_time,
            ),
            Message::from_existing(
                MessageId::new(),
                session_id.clone(),
                MessageRole::Character,
                blocks_json.to_string(),
                Some("happy".to_string()),
                base_time,
            ),
            Message::from_existing(
                MessageId::new(),
                session_id,
                MessageRole::User,
                "มีขนมปังอะไรบ้าง".to_string(),
                None,
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

        assert!(prompt.contains("blocks"));
        assert!(prompt.contains("narration"));
        assert!(prompt.contains("dialogue"));
        assert!(prompt.contains("scene_update"));
        assert!(prompt.contains("Reply with ONLY a JSON object"));
    }

    #[test]
    fn wrap_character_content_json_array() {
        let json = r#"[{"type":"narration","text":"hello"}]"#;
        let result = wrap_character_content(json);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["blocks"].is_array());
    }

    #[test]
    fn wrap_character_content_plain_text() {
        let result = wrap_character_content("plain text");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["blocks"][0]["type"], "dialogue");
        assert_eq!(parsed["blocks"][0]["text"], "plain text");
    }

    #[test]
    fn compact_atmosphere_parses_json_blob() {
        let json_atmo = r#"{"mood":"playful","tags":["สนุก","ท้าทาย"],"sensory":{"sight":"ร้านราเมนเล็กๆ","sound":"เสียงคนคุย","smell":"กลิ่นน้ำซุป"}}"#;
        let result = compact_atmosphere(json_atmo);

        assert!(result.contains("playful"));
        assert!(result.contains("สนุก"));
        assert!(result.contains("ท้าทาย"));
        assert!(result.contains("ร้านราเมนเล็กๆ"));
        assert!(result.contains("เสียงคนคุย"));
        assert!(result.contains("กลิ่นน้ำซุป"));
    }

    #[test]
    fn compact_atmosphere_passes_plain_text() {
        let plain = "อบอุ่น หอมกลิ่นขนมปัง";
        let result = compact_atmosphere(plain);
        assert_eq!(result, plain);
    }
}
