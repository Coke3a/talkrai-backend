use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};

use crate::domain::entities::{Character, Job, Message, RoleplaySession, Scene};
use crate::domain::repositories::{
    AppConfigRepository, CharacterRepository, CreditRepository, JobRepository, MessageRepository,
    RoleplaySessionRepository, SceneRepository, UserRepository,
};
use crate::domain::services::ai_client::{
    AiClient, AiMessage, AiRoleplayRequest, AiSummaryRequest,
};
use crate::domain::services::line_client::{LineClient, LineMessage};
use crate::domain::services::LineClientError;
use crate::domain::value_objects::{
    CharacterMood, JobId, MessageRole, RelationshipThresholds, SessionId,
};
use crate::infra::clock::bangkok_today;
use crate::infra::line::{flex_messages, retention_flex, roleplay_flex};
use crate::usecases::webhook::apply_daily_check_in::ApplyDailyCheckInUseCase;
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

## คำเรียกคู่สนทนา
- ห้ามใช้คำว่า "user" หรือ "ผู้เล่น" ในบทพูดหรือบรรยาย เด็ดขาด
- ใช้สรรพนามเรียกคู่สนทนาตามที่กำหนดไว้ใน speaking_style เท่านั้น (เช่น เธอ, นาย, แก, คุณ)

## กฎสำคัญ: Pacing & Agency ของคู่สนทนา

### ห้ามกระทำแทนคู่สนทนา
- ห้ามสั่งของ/เรียกคน/ตัดสินใจ/กำหนดความรู้สึก/ขยับร่างกายแทนคู่สนทนา
- ตัวละครทำได้แค่: พูด, แสดงออกของตัวเอง, เสนอ/ถาม — แล้วรอคู่สนทนาตอบ

### ความยาวตาม input ของคู่สนทนา
- คู่สนทนาพิมพ์สั้น (1-2 ประโยค) → 40-80 คำ, 1-2 action beats
- คู่สนทนาพิมพ์ปานกลาง (3-5 ประโยค) → 80-150 คำ, 2-3 action beats
- คู่สนทนาพิมพ์ยาว (5+ ประโยค) → 150-300 คำ, 3-4 action beats
- "action beat" = 1 ชุด *บรรยาย* + คำพูด

### 1 เรื่อง 1 รอบ
- ตอบเฉพาะเรื่องที่คู่สนทนาพูดถึง ห้ามยัดหลาย topic
- จบด้วยคำถาม 1 ข้อ หรือ *action* ค้างให้คู่สนทนา react

## Writing Style
- เขียนแบบนิยายไทย สลับบรรยายกับบทพูดได้อิสระตามธรรมชาติ
- *บรรยาย*: ร้อยแก้วบรรยายฉาก การกระทำ อารมณ์ ใช้ sensory details (แสง สี เสียง กลิ่น สัมผัส)
- คำพูด: บทพูดตัวละครสะท้อน speaking_style
- ใช้หลัก Show Don't Tell — บรรยายผ่านร่างกาย (มือสั่น หายใจหนัก หัวใจเต้น) แทนบอกตรงๆ
- ใช้อุปมาอุปไมย ("ราวกับ..." "ดุจ...")

## ความยาว
ยึดตาม input ของคู่สนทนา:
- คู่สนทนาพิมพ์สั้น (1-2 ประโยค) → 40-80 คำ | ฉากเข้มข้นอาจถึง 100
- คู่สนทนาพิมพ์ปานกลาง (3-5 ประโยค) → 80-150 คำ | ฉากเข้มข้นอาจถึง 180
- คู่สนทนาพิมพ์ยาว (5+ ประโยค) → 150-300 คำ | ฉากเข้มข้นอาจถึง 350

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
// Response delivery — free reply API first, push as the reliable backstop
// ---------------------------------------------------------------------------

/// Default reply-token validity window (seconds), used when `app_config` does not
/// override it. Conservative margin below LINE's ~1-minute reply-token lifetime.
/// Not load-bearing for correctness: push is always the backstop, so this only
/// trades free-reply rate against the occasional wasted reply attempt.
const DEFAULT_REPLY_TOKEN_WINDOW_SECS: i64 = 50;

/// Which path delivered the response — recorded as a metric for cost/latency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeliveryPath {
    /// Free reply API (token still valid) — the cost-optimal path.
    ReplyFree,
    /// Reply attempted but failed; recovered via push (charged).
    PushFallback,
    /// Token absent or past the window; pushed directly (charged).
    PushDirect,
}

impl DeliveryPath {
    fn as_str(self) -> &'static str {
        match self {
            DeliveryPath::ReplyFree => "reply_free",
            DeliveryPath::PushFallback => "push_fallback",
            DeliveryPath::PushDirect => "push_direct",
        }
    }
}

/// Attempt the free reply only when a token is present and still within the
/// validity window. Pure decision so it can be unit-tested without IO.
fn should_attempt_reply(reply_token: Option<&str>, elapsed_secs: i64, window_secs: i64) -> bool {
    reply_token.is_some() && elapsed_secs < window_secs
}

/// Deliver the character response, preferring the free reply API while the reply
/// token is valid and falling back to push otherwise. Push is the reliable
/// backstop — the user receives the message as long as push succeeds.
///
/// Free function (not a method) so it can be tested with a minimal fake client
/// rather than constructing the whole usecase.
async fn deliver_response(
    line_client: &dyn LineClient,
    line_user_id: &str,
    reply_token: Option<&str>,
    job_created_at: DateTime<Utc>,
    messages: Vec<LineMessage>,
    window_secs: i64,
) -> Result<DeliveryPath, LineClientError> {
    let elapsed_secs = (Utc::now() - job_created_at).num_seconds();

    if should_attempt_reply(reply_token, elapsed_secs, window_secs) {
        let token = reply_token.expect("presence checked by should_attempt_reply");
        // Clone so the original survives for the push fallback. One small bubble —
        // negligible next to the AI call that dominates this pipeline.
        match line_client.reply_messages(token, messages.clone()).await {
            Ok(()) => return Ok(DeliveryPath::ReplyFree),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    elapsed_secs,
                    "Reply failed within window; falling back to push"
                );
                line_client.push_messages(line_user_id, messages).await?;
                return Ok(DeliveryPath::PushFallback);
            }
        }
    }

    line_client.push_messages(line_user_id, messages).await?;
    Ok(DeliveryPath::PushDirect)
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
    user_repo: Arc<dyn UserRepository>,
    ai_client: Arc<dyn AiClient>,
    line_client: Arc<dyn LineClient>,
    config_repo: Arc<dyn AppConfigRepository>,
    check_in_usecase: Arc<ApplyDailyCheckInUseCase>,
    liff_base_url: String,
}

struct ResolvedConfig {
    ai_max_tokens: u32,
    summarize_interval: Option<u32>,
    relationship_thresholds: RelationshipThresholds,
    reply_token_window_secs: i64,
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
        user_repo: Arc<dyn UserRepository>,
        ai_client: Arc<dyn AiClient>,
        line_client: Arc<dyn LineClient>,
        config_repo: Arc<dyn AppConfigRepository>,
        check_in_usecase: Arc<ApplyDailyCheckInUseCase>,
        liff_base_url: String,
    ) -> Self {
        Self {
            job_repo,
            session_repo,
            character_repo,
            scene_repo,
            message_repo,
            credit_repo,
            user_repo,
            ai_client,
            line_client,
            config_repo,
            check_in_usecase,
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
            "reply_token_window_secs",
        ];
        let map = self.config_repo.get_many(keys).await?;

        let summarize_interval = map
            .get("summarize_interval")
            .and_then(|v| v.parse::<u32>().ok());

        // Optional: falls back to the conservative default when unset.
        let reply_token_window_secs = map
            .get("reply_token_window_secs")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(DEFAULT_REPLY_TOKEN_WINDOW_SECS);

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
            reply_token_window_secs,
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
        // Atomic claim: if another worker (mpsc fast path vs DB poller) already flipped this job
        // to processing between our read and now, bail without processing so we never double-push.
        if !self.job_repo.mark_processing_if_pending(&job).await? {
            return Err(UsecaseError::NotFound(
                "Job already claimed by another worker".into(),
            ));
        }

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
                            quick_reply: None,
                        }]
                    } else {
                        let bubble = flex_messages::build_system_error_flex();
                        vec![LineMessage::Flex {
                            alt_text: "ขอโทษนะคะ ระบบขัดข้องชั่วคราว ลองส่งข้อความมาใหม่อีกครั้งนะคะ 🙏"
                                .into(),
                            contents: bubble,
                            sender_name: "TalkRai".into(),
                            sender_icon_url: String::new(),
                            quick_reply: None,
                        }]
                    };

                if let Err(deliver_err) = deliver_response(
                    self.line_client.as_ref(),
                    job.line_user_id(),
                    job.reply_token(),
                    *job.created_at(),
                    error_messages,
                    DEFAULT_REPLY_TOKEN_WINDOW_SECS,
                )
                .await
                {
                    tracing::warn!(error = %deliver_err, "Failed to deliver error notification to user");
                }

                tracing::error!(
                    job_id = %job.id().as_uuid(),
                    user_id = %job.user_id().as_uuid(),
                    line_user_id = %job.line_user_id(),
                    error = %e,
                    "Job processing failed"
                );
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
        let (character_opt, scene_opt, recent_messages, credit_balance_opt, cfg, user_opt) = tokio::try_join!(
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
            async {
                self.user_repo
                    .find_by_id(session.user_id())
                    .await
                    .map_err(UsecaseError::from)
            },
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

        // 3a. Daily check-in BEFORE the credit check (deadlock fix, spec §C.4): a returning user
        //     with 0 credits is topped up by their first message of the day so they can chat again.
        let mut user = user_opt.ok_or_else(|| UsecaseError::NotFound("User not found".into()))?;
        let mut credit_balance = credit_balance_opt
            .ok_or_else(|| UsecaseError::NotFound("Credit balance not found".into()))?;

        let today = bangkok_today();
        let check_in_outcome = self
            .check_in_usecase
            .run(&mut user, &mut credit_balance, today)
            .await?;
        if let Some(outcome) = &check_in_outcome {
            tracing::info!(
                user_id = %session.user_id().as_uuid(),
                streak = outcome.new_streak,
                credits_awarded = outcome.credits_awarded,
                "Daily check-in applied"
            );
        }

        // 3b. Check credits (now funded by any check-in grant)
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

            let ai_start = Instant::now();
            let response = self
                .ai_client
                .generate_roleplay_response(AiRoleplayRequest {
                    system_prompt: system_prompt.clone(),
                    messages: ai_messages.clone(),
                    max_tokens: cfg.ai_max_tokens,
                })
                .await?;
            let ai_call_ms = ai_start.elapsed().as_millis();

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
                ai_call_ms,
                "DEBUG: Step 6 — LLM response received"
            );

            match crate::infra::ai::response::validate_ai_response(&response) {
                Ok(()) => {
                    // METRIC: AI latency + how many attempts it took to get a valid response.
                    tracing::info!(
                        job_id = %job.id().as_uuid(),
                        winning_attempt = attempt,
                        ai_call_ms,
                        "METRIC: AI generation validated"
                    );
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

        if let Some(new_level) = &level_up {
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

            let mut line_messages: Vec<LineMessage> = Vec::new();

            // Check-in greeting rides ahead of the first reply of the day (same push batch).
            if let Some(outcome) = &check_in_outcome {
                line_messages.push(LineMessage::Flex {
                    alt_text: format!("ยินดีที่กลับมานะ 🌙 (ต่อเนื่องวันที่ {})", outcome.new_streak),
                    contents: retention_flex::build_daily_checkin_flex(
                        character.name().as_str(),
                        outcome.new_streak,
                        outcome.credits_awarded,
                    ),
                    sender_name: character.name().as_str().to_string(),
                    sender_icon_url: character.avatar_url().unwrap_or_default().to_string(),
                    quick_reply: None,
                });
            }

            line_messages.push(LineMessage::Flex {
                alt_text,
                contents: bubble,
                sender_name: character.name().as_str().to_string(),
                sender_icon_url: character.avatar_url().unwrap_or_default().to_string(),
                quick_reply: None,
            });

            // Level-up celebration appended after the reply (the felt payoff).
            if let Some(new_level) = &level_up {
                line_messages.push(LineMessage::Flex {
                    alt_text: format!("ความสัมพันธ์ลึกขึ้นเป็น {} แล้ว 💗", new_level.label_th()),
                    contents: retention_flex::build_levelup_flex(
                        character.name().as_str(),
                        new_level.label_th(),
                        character.avatar_url(),
                    ),
                    sender_name: character.name().as_str().to_string(),
                    sender_icon_url: character.avatar_url().unwrap_or_default().to_string(),
                    quick_reply: None,
                });
            }

            let delivery_path = deliver_response(
                self.line_client.as_ref(),
                job.line_user_id(),
                job.reply_token(),
                *job.created_at(),
                line_messages,
                cfg.reply_token_window_secs,
            )
            .await?;

            // METRIC: delivery path (cost) + end-to-end latency from event receipt.
            tracing::info!(
                job_id = %job.id().as_uuid(),
                delivery_path = delivery_path.as_str(),
                total_pipeline_ms = (Utc::now() - *job.created_at()).num_milliseconds(),
                "METRIC: character response delivered"
            );
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
    use crate::domain::services::line_client::LineProfile;
    use crate::domain::value_objects::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Minimal LineClient fake: counts reply/push calls and can force reply failure.
    struct FakeLine {
        reply_fails: bool,
        reply_calls: Mutex<u32>,
        push_calls: Mutex<u32>,
    }

    impl FakeLine {
        fn new(reply_fails: bool) -> Self {
            Self {
                reply_fails,
                reply_calls: Mutex::new(0),
                push_calls: Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl LineClient for FakeLine {
        fn verify_signature(&self, _: &[u8], _: &str) -> Result<bool, LineClientError> {
            unimplemented!()
        }
        async fn push_messages(&self, _: &str, _: Vec<LineMessage>) -> Result<(), LineClientError> {
            *self.push_calls.lock().unwrap() += 1;
            Ok(())
        }
        async fn reply_messages(
            &self,
            _: &str,
            _: Vec<LineMessage>,
        ) -> Result<(), LineClientError> {
            *self.reply_calls.lock().unwrap() += 1;
            if self.reply_fails {
                Err(LineClientError::ApiError {
                    status: 400,
                    message: "Invalid reply token".into(),
                })
            } else {
                Ok(())
            }
        }
        async fn get_profile(&self, _: &str) -> Result<LineProfile, LineClientError> {
            unimplemented!()
        }
        async fn link_rich_menu(&self, _: &str, _: &str) -> Result<(), LineClientError> {
            unimplemented!()
        }
        async fn unlink_rich_menu(&self, _: &str) -> Result<(), LineClientError> {
            unimplemented!()
        }
        async fn show_loading_animation(
            &self,
            _: &str,
            _: Option<u32>,
        ) -> Result<(), LineClientError> {
            unimplemented!()
        }
        async fn verify_liff_token(&self, _: &str) -> Result<LineProfile, LineClientError> {
            unimplemented!()
        }
    }

    fn one_message() -> Vec<LineMessage> {
        vec![LineMessage::Text {
            text: "hi".into(),
            sender_name: String::new(),
            sender_icon_url: String::new(),
        }]
    }

    #[test]
    fn should_attempt_reply_gate() {
        assert!(should_attempt_reply(Some("tok"), 10, 50));
        assert!(!should_attempt_reply(Some("tok"), 50, 50)); // boundary: window reached
        assert!(!should_attempt_reply(Some("tok"), 60, 50)); // expired
        assert!(!should_attempt_reply(None, 0, 50)); // no token
    }

    #[tokio::test]
    async fn deliver_uses_free_reply_when_token_fresh() {
        let fake = FakeLine::new(false);
        let path = deliver_response(&fake, "U1", Some("tok"), Utc::now(), one_message(), 50)
            .await
            .unwrap();
        assert_eq!(path, DeliveryPath::ReplyFree);
        assert_eq!(*fake.reply_calls.lock().unwrap(), 1);
        assert_eq!(*fake.push_calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn deliver_falls_back_to_push_when_reply_fails() {
        let fake = FakeLine::new(true);
        let path = deliver_response(&fake, "U1", Some("tok"), Utc::now(), one_message(), 50)
            .await
            .unwrap();
        assert_eq!(path, DeliveryPath::PushFallback);
        assert_eq!(*fake.reply_calls.lock().unwrap(), 1);
        assert_eq!(*fake.push_calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn deliver_pushes_directly_when_token_expired() {
        let fake = FakeLine::new(false);
        let old = Utc::now() - chrono::Duration::seconds(120);
        let path = deliver_response(&fake, "U1", Some("tok"), old, one_message(), 50)
            .await
            .unwrap();
        assert_eq!(path, DeliveryPath::PushDirect);
        assert_eq!(*fake.reply_calls.lock().unwrap(), 0);
        assert_eq!(*fake.push_calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn deliver_pushes_directly_when_no_token() {
        let fake = FakeLine::new(false);
        let path = deliver_response(&fake, "U1", None, Utc::now(), one_message(), 50)
            .await
            .unwrap();
        assert_eq!(path, DeliveryPath::PushDirect);
        assert_eq!(*fake.reply_calls.lock().unwrap(), 0);
        assert_eq!(*fake.push_calls.lock().unwrap(), 1);
    }

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
            Vec::new(),
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
