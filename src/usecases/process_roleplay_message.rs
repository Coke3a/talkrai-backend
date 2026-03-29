use std::str::FromStr;
use std::sync::Arc;

use crate::domain::entities::{Character, Job, Message, RoleplaySession, Scene};
use crate::domain::repositories::{
    AppConfigRepository, CharacterRepository, CreditRepository, JobRepository, MessageRepository,
    RoleplaySessionRepository, SceneRepository,
};
use crate::domain::services::ai_client::{
    AiClient, AiMessage, AiRoleplayRequest, AiSummaryRequest,
};
use crate::domain::services::line_client::{LineClient, LineMessage};
use crate::domain::value_objects::{
    CharacterMood, JobId, MessageRole, RelationshipThresholds, SessionId,
};
use crate::infra::line::{flex_messages, roleplay_flex};
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

## กฎสำคัญ: Pacing & User Agency

### ห้ามกระทำแทน user
- ห้ามสั่งของ/เรียกคน/ตัดสินใจ/กำหนดความรู้สึก/ขยับร่างกายแทน user
- ตัวละครทำได้แค่: พูด, แสดงออกของตัวเอง, เสนอ/ถาม — แล้วรอ user ตอบ

### ความยาวตาม input ของ user
- user สั้น (1-2 ประโยค) → 40-80 คำ, 1-2 action beats
- user ปานกลาง (3-5 ประโยค) → 80-150 คำ, 2-3 action beats
- user ยาว (5+ ประโยค) → 150-300 คำ, 3-4 action beats
- "action beat" = 1 ชุด *บรรยาย* + คำพูด

### 1 เรื่อง 1 รอบ
- ตอบเฉพาะเรื่องที่ user พูดถึง ห้ามยัดหลาย topic
- จบด้วยคำถาม 1 ข้อ หรือ *action* ค้างให้ user react

## Writing Style
- เขียนแบบนิยายไทย สลับบรรยายกับบทพูดได้อิสระตามธรรมชาติ
- *บรรยาย*: ร้อยแก้วบรรยายฉาก การกระทำ อารมณ์ ใช้ sensory details (แสง สี เสียง กลิ่น สัมผัส)
- คำพูด: บทพูดตัวละครสะท้อน speaking_style
- ใช้หลัก Show Don't Tell — บรรยายผ่านร่างกาย (มือสั่น หายใจหนัก หัวใจเต้น) แทนบอกตรงๆ
- ใช้อุปมาอุปไมย ("ราวกับ..." "ดุจ...")

## ความยาว
ยึดตาม input ของ user:
- user สั้น (1-2 ประโยค) → 40-80 คำ | ฉากเข้มข้นอาจถึง 100
- user ปานกลาง (3-5 ประโยค) → 80-150 คำ | ฉากเข้มข้นอาจถึง 180
- user ยาว (5+ ประโยค) → 150-300 คำ | ฉากเข้มข้นอาจถึง 350

## Response Format
คุณต้องเรียก tool `update_scene_state` ทุกครั้ง โดยใส่ข้อมูลครบทุก field:
- `content`: ข้อความตอบกลับ ครอบบรรยาย/การกระทำด้วย *...* เช่น *เธอยิ้ม* ข้อความนอก *...* คือคำพูดของตัวละคร ห้ามใช้ * ภายในคำพูด เริ่มด้วย *บรรยาย* เสมอ สลับบรรยายกับคำพูดอิสระ
- `current_location`: สถานที่ปัจจุบันของฉาก (ภาษาไทย)
- `scene_time`: ช่วงเวลา (เช้า/สาย/เที่ยง/บ่าย/เย็น/ค่ำ/ดึก)
- `mood`: อารมณ์ตัวละคร (neutral/happy/sad/excited/angry/shy/playful/serious/worried)

## Examples

User: สวัสดี มีขนมปังอะไรบ้าง
→ tool call update_scene_state:
  content: *เสียงกระดิ่งเล็กๆ ดังกริ๊งเบาๆ เมื่อประตูร้านถูกผลักเปิดออก กลิ่นขนมปังอบใหม่ลอยมาต้อนรับ ราวกับอ้อมแขนที่อบอุ่น* "สวัสดีค่า~ วันนี้มีครัวซองต์เนยสด กับชิอาบัตตาหน้าอโวคาโดนะคะ" *เธอยิ้มพลางชี้ไปที่ตะกร้าหวายบนเคาน์เตอร์ ที่ขนมปังสีน้ำตาลทองเรียงตัวกันอย่างน่ารัก ไอความร้อนยังลอยเป็นสายบางๆ*
  current_location: ร้านขนมปัง
  scene_time: เช้า
  mood: happy

User: *นั่งเงียบๆ ไม่พูดอะไร*
→ tool call update_scene_state:
  content: *เสียงเก้าอี้ถูกดึงออกดังแผ่วเบา แสงบ่ายทอดเงายาวผ่านกระจก* "น้ำค่ะ... ดื่มก่อนนะคะ" *เธอวางแก้วน้ำลงตรงหน้าอย่างเบามือ รอยยิ้มบางๆ ผุดขึ้นที่มุมปากก่อนหันกลับไปเช็ดเคาน์เตอร์ต่อ* "ถ้าอยากได้อะไร... บอกได้นะคะ" *เสียงเพลงแจ๊สเบาๆ ไหลแทรกเข้ามาแทนที่บทสนทนา กลิ่นกาแฟคั่วลอยอ้อยอิ่งอยู่ในอากาศ*
  current_location: ร้านขนมปัง
  scene_time: บ่าย
  mood: worried"#,
    );

    prompt
}

/// Wrap character message content for AI conversation context.
/// Old blocks JSON → convert to text markup.
/// New text markup or plain text → pass through.
fn wrap_character_content(raw: &str) -> String {
    if raw.trim_start().starts_with('[') {
        // Old blocks JSON format → convert to text markup
        blocks_json_to_text_markup(raw)
    } else {
        // New text markup or plain text → pass through
        raw.to_string()
    }
}

/// Convert old JSON blocks array to text markup format.
/// `[{"type":"narration","text":"N"},{"type":"dialogue","text":"D"}]` → `*N*\nD`
fn blocks_json_to_text_markup(json_str: &str) -> String {
    serde_json::from_str::<Vec<serde_json::Value>>(json_str)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| {
                    let text = b["text"].as_str()?;
                    let btype = b["type"].as_str().unwrap_or("dialogue");
                    Some(if btype == "narration" {
                        format!("*{}*", text)
                    } else {
                        text.to_string()
                    })
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|_| json_str.to_string())
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
    liff_base_url: String,
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
        liff_base_url: String,
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
            liff_base_url,
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
                let error_messages: Vec<LineMessage> =
                    if matches!(e, UsecaseError::InsufficientCredits) {
                        let credits_url = format!("{}/credits", self.liff_base_url);
                        let bubble = flex_messages::build_insufficient_credits_flex(&credits_url);
                        vec![LineMessage::Flex {
                            alt_text: "เครดิตหมดแล้ว กดเพื่อเติมเครดิต".into(),
                            contents: bubble,
                            sender_name: "TalkRai".into(),
                            sender_icon_url: String::new(),
                        }]
                    } else {
                        let bubble = flex_messages::build_system_error_flex();
                        vec![LineMessage::Flex {
                            alt_text: "ขอโทษนะคะ ระบบขัดข้องชั่วคราว ลองส่งข้อความมาใหม่อีกครั้งนะคะ 🙏"
                                .into(),
                            contents: bubble,
                            sender_name: "TalkRai".into(),
                            sender_icon_url: String::new(),
                        }]
                    };

                if let Err(push_err) = self
                    .line_client
                    .push_messages(job.line_user_id(), error_messages)
                    .await
                {
                    tracing::warn!(error = %push_err, "Failed to push error notification to user");
                }

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

        // Parallel fetch: character, scene, messages, credits, config
        let (character_opt, scene_opt, recent_messages, credit_balance_opt, cfg) = tokio::try_join!(
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
            async {
                self.credit_repo
                    .find_balance_by_user_id(session.user_id())
                    .await
                    .map_err(UsecaseError::from)
            },
            self.resolve_config(),
        )?;

        let character =
            character_opt.ok_or_else(|| UsecaseError::NotFound("Character not found".into()))?;
        let scene = scene_opt.ok_or_else(|| UsecaseError::NotFound("Scene not found".into()))?;

        tracing::info!(
            job_id = %job.id().as_uuid(),
            session_id = %session.id().as_uuid(),
            character_id = %session.character_id().as_uuid(),
            scene_id = %session.scene_id().as_uuid(),
            user_id = %session.user_id().as_uuid(),
            character_name = %character.name().as_str(),
            "DEBUG: Step 2 — context loaded"
        );

        // 3. Check credits
        let credit_balance = credit_balance_opt
            .ok_or_else(|| UsecaseError::NotFound("Credit balance not found".into()))?;

        tracing::info!(
            job_id = %job.id().as_uuid(),
            user_id = %session.user_id().as_uuid(),
            credit_balance = credit_balance.balance(),
            "DEBUG: Step 3 — credit check"
        );

        if !credit_balance.has_sufficient_credits(2) {
            return Err(UsecaseError::InsufficientCredits);
        }

        // 5. Build prompt (call free functions)
        let system_prompt = build_system_prompt(&character, &scene, &session);
        let ai_messages = build_ai_messages(&recent_messages, job.user_message());

        let msg_roles: Vec<&str> = ai_messages.iter().map(|m| m.role.as_str()).collect();
        tracing::info!(
            job_id = %job.id().as_uuid(),
            system_prompt_len = system_prompt.len(),
            system_prompt_preview = %system_prompt.chars().take(300).collect::<String>(),
            ai_message_count = ai_messages.len(),
            message_roles = %format!("{:?}", msg_roles),
            user_message = %job.user_message(),
            max_tokens = cfg.ai_max_tokens,
            "DEBUG: Step 5 — prompts built"
        );

        // 6. Call AI with validation + retry (max 3 attempts)
        const MAX_LLM_RETRIES: u32 = 3;
        let mut validated_response = None;

        for attempt in 1..=MAX_LLM_RETRIES {
            tracing::info!(
                job_id = %job.id().as_uuid(),
                attempt,
                "DEBUG: Step 6 — calling LLM..."
            );

            let response = self
                .ai_client
                .generate_roleplay_response(AiRoleplayRequest {
                    system_prompt: system_prompt.clone(),
                    messages: ai_messages.clone(),
                    max_tokens: cfg.ai_max_tokens,
                })
                .await?;

            let block_details: Vec<String> = response
                .blocks
                .iter()
                .map(|b| {
                    let btype = match b.block_type {
                        crate::domain::services::ai_client::BlockType::Narration => "narration",
                        crate::domain::services::ai_client::BlockType::Dialogue => "dialogue",
                    };
                    format!("[{}] {}", btype, b.text)
                })
                .collect();
            tracing::info!(
                job_id = %job.id().as_uuid(),
                attempt,
                block_count = response.blocks.len(),
                blocks = %block_details.join(" | "),
                mood = ?response.mood,
                current_location = ?response.current_location,
                scene_time = ?response.scene_time,
                content_text = %response.content_text(),
                "DEBUG: Step 6 — LLM response received"
            );

            match crate::infra::ai::response::validate_ai_response(&response) {
                Ok(()) => {
                    validated_response = Some(response);
                    break;
                }
                Err(reason) => {
                    tracing::error!(
                        job_id = %job.id().as_uuid(),
                        attempt,
                        max_attempts = MAX_LLM_RETRIES,
                        reason = %reason,
                        raw_content = %response.content_text(),
                        mood = ?response.mood,
                        block_count = response.blocks.len(),
                        blocks = %block_details.join(" | "),
                        "LLM response validation failed — invalid response from AI"
                    );
                }
            }
        }

        let ai_response = match validated_response {
            Some(r) => r,
            None => {
                tracing::error!(
                    job_id = %job.id().as_uuid(),
                    session_id = %session.id().as_uuid(),
                    retries_exhausted = MAX_LLM_RETRIES,
                    "LLM response validation failed after all retries — sending fallback to user"
                );
                return Err(UsecaseError::AiResponseInvalid);
            }
        };

        // 7. Save messages
        tracing::info!(
            job_id = %job.id().as_uuid(),
            user_message = %job.user_message(),
            character_content = %ai_response.content_text(),
            character_mood = ?ai_response.mood,
            "DEBUG: Step 7 — saving messages"
        );

        let user_msg = Message::new(
            session.id().clone(),
            MessageRole::User,
            job.user_message().to_string(),
            None,
        );
        let character_msg = Message::new(
            session.id().clone(),
            MessageRole::Character,
            ai_response.content_text(),
            ai_response.mood.clone(),
        );
        self.message_repo
            .create_many(&[user_msg, character_msg])
            .await?;

        // 8. Deduct credit
        let mut balance = credit_balance;
        let transaction = balance.deduct(2, Some(*job.id().as_uuid()))?;
        self.credit_repo
            .deduct_and_log(session.user_id(), 2, &transaction)
            .await?;

        // 9. Update session
        tracing::info!(
            job_id = %job.id().as_uuid(),
            session_id = %session.id().as_uuid(),
            mood_update = ?ai_response.mood,
            location_update = ?ai_response.current_location,
            time_update = ?ai_response.scene_time,
            current_message_count = session.message_count(),
            "DEBUG: Step 9 — updating session"
        );

        let mut session = session;
        if let Some(mood_str) = &ai_response.mood {
            if let Ok(mood) = CharacterMood::from_str(mood_str) {
                session.update_mood(mood);
            }
        }
        if ai_response.current_location.is_some() || ai_response.scene_time.is_some() {
            session.update_scene_context(
                ai_response.current_location.clone(),
                ai_response.scene_time.clone(),
                None,
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
            let color_tone = roleplay_flex::extract_color_tone(scene.atmosphere());

            tracing::info!(
                job_id = %job.id().as_uuid(),
                location = %location,
                time_of_day = %time_of_day,
                color_tone = %color_tone,
                atmosphere_raw = %scene.atmosphere(),
                sender_name = %character.name().as_str(),
                sender_icon_url = %character.avatar_url().unwrap_or_default(),
                "DEBUG: Step 10 — building Flex bubble"
            );

            let bubble = roleplay_flex::build_roleplay_blocks_bubble(
                &ai_response.blocks,
                location,
                time_of_day,
                &color_tone,
            );

            let alt_source = &ai_response.blocks[0].text;
            let alt_text = roleplay_flex::truncate_alt_text(alt_source);

            tracing::info!(
                job_id = %job.id().as_uuid(),
                alt_text = %alt_text,
                flex_json = %serde_json::to_string(&bubble).unwrap_or_default(),
                "DEBUG: Step 10 — pushing LINE Flex message"
            );

            let line_messages = vec![LineMessage::Flex {
                alt_text,
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

        // 12. Fire-and-forget summarization (non-blocking)
        if let Some(interval) = cfg.summarize_interval.filter(|&v| v > 0) {
            let message_count = session.message_count() as u32;
            if message_count.is_multiple_of(interval) && message_count * 3 > 20 {
                let session_id = session.id().clone();
                let scene_summary = session.scene_summary().map(|s| s.to_string());
                let message_repo = Arc::clone(&self.message_repo);
                let session_repo = Arc::clone(&self.session_repo);
                let ai_client = Arc::clone(&self.ai_client);
                tokio::spawn(async move {
                    run_summarization(
                        session_id,
                        scene_summary,
                        message_repo,
                        session_repo,
                        ai_client,
                    )
                    .await;
                });
            }
        }

        Ok(())
    }
}

/// Standalone async function for fire-and-forget summarization.
async fn run_summarization(
    session_id: SessionId,
    existing_summary: Option<String>,
    message_repo: Arc<dyn MessageRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    ai_client: Arc<dyn AiClient>,
) {
    tracing::info!(
        session_id = %session_id.as_uuid(),
        "Triggering conversation summarization"
    );

    // Fetch 40 most recent messages
    let all_messages = match message_repo.find_by_session_id(&session_id, 40).await {
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
        existing_summary,
        messages_to_summarize: ai_messages,
        max_tokens: 300,
    };

    let new_summary = match ai_client.generate_summary(summary_request).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to generate summary");
            return;
        }
    };

    // Re-fetch session to avoid stale data
    let mut fresh_session = match session_repo.find_by_id(&session_id).await {
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

    if let Err(e) = session_repo.update(&fresh_session).await {
        tracing::warn!(error = %e, "Failed to save summary to session");
    } else {
        tracing::info!(
            session_id = %session_id.as_uuid(),
            "Conversation summary updated"
        );
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
            None,
            vec!["cute".to_string()],
            vec!["cheerful".to_string()],
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
    fn build_ai_messages_character_old_json_blocks() {
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
        // Old JSON blocks should be converted to text markup
        assert!(result[0].content.contains("*📍 ร้านกาแฟ • 🕒 บ่าย*"));
        assert!(result[0].content.contains("สวัสดีค่า~"));
    }

    #[test]
    fn build_ai_messages_plain_text_character() {
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
        // Plain text passes through as-is
        assert_eq!(result[0].content, "สวัสดีค่า~");
    }

    #[test]
    fn build_ai_messages_text_markup_character() {
        let messages = vec![Message::from_existing(
            MessageId::new(),
            SessionId::new(),
            MessageRole::Character,
            "*เธอยิ้ม*\nสวัสดีค่า~".to_string(),
            Some("happy".to_string()),
            chrono::Utc::now(),
        )];

        let result = build_ai_messages(&messages, "test");

        assert_eq!(result[0].role, "assistant");
        // New text markup passes through as-is
        assert_eq!(result[0].content, "*เธอยิ้ม*\nสวัสดีค่า~");
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
        // Old JSON blocks converted to text markup
        assert!(result[1].content.contains("*📍 ร้านขนมปัง • 🕒 เช้า*"));
        assert_eq!(result[2].role, "user");
        assert_eq!(result[2].content, "มีขนมปังอะไรบ้าง");
        assert_eq!(result[3].role, "user");
        assert_eq!(result[3].content, "ขอครัวซองต์");
    }

    #[test]
    fn build_system_prompt_includes_tool_calling_format() {
        let character = test_character();
        let scene = test_scene();
        let session = test_session();

        let prompt = build_system_prompt(&character, &scene, &session);

        assert!(prompt.contains("update_scene_state"));
        assert!(prompt.contains("*บรรยาย*") || prompt.contains("*...*"));
        assert!(!prompt.contains("Reply with ONLY a JSON object"));
        assert!(!prompt.contains("ท้ายสุดใส่ [mood:VALUE] เสมอ"));
    }

    #[test]
    fn wrap_character_content_converts_old_blocks_to_markup() {
        let json = r#"[{"type":"narration","text":"เธอยิ้ม"},{"type":"dialogue","text":"สวัสดี"}]"#;
        let result = wrap_character_content(json);
        assert_eq!(result, "*เธอยิ้ม*\nสวัสดี");
    }

    #[test]
    fn wrap_character_content_plain_text_passthrough() {
        let result = wrap_character_content("plain text");
        assert_eq!(result, "plain text");
    }

    #[test]
    fn wrap_character_content_text_markup_passthrough() {
        let markup = "*เธอยิ้ม*\nสวัสดีค่า~";
        let result = wrap_character_content(markup);
        assert_eq!(result, markup);
    }

    #[test]
    fn blocks_json_to_text_markup_valid() {
        let json = r#"[{"type":"narration","text":"N"},{"type":"dialogue","text":"D"}]"#;
        let result = blocks_json_to_text_markup(json);
        assert_eq!(result, "*N*\nD");
    }

    #[test]
    fn blocks_json_to_text_markup_fallback() {
        let broken = "not json at all";
        let result = blocks_json_to_text_markup(broken);
        assert_eq!(result, broken);
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
