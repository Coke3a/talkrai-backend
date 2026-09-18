use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::mpsc;

use crate::domain::entities::{CreditBalance, Job, User};
use crate::domain::repositories::{
    AppConfigRepository, CreditRepository, JobRepository, RoleplaySessionRepository, UserRepository,
};
use crate::domain::services::line_client::{LineClient, LineMessage};
use crate::domain::value_objects::{JobId, JobMode, SessionId, UserId};
use crate::infra::line::flex_messages;
use crate::usecases::UsecaseError;

// LINE webhook DTOs — private, deserialization only

#[derive(Deserialize)]
struct LineWebhookBody {
    events: Vec<LineEvent>,
}

#[derive(Deserialize)]
struct LineEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(rename = "replyToken")]
    reply_token: Option<String>,
    source: Option<LineEventSource>,
    message: Option<LineEventMessage>,
    postback: Option<LineEventPostback>,
    follow: Option<LineEventFollow>,
}

#[derive(Deserialize)]
struct LineEventFollow {
    #[serde(rename = "isUnblocked")]
    is_unblocked: bool,
}

#[derive(Deserialize)]
struct LineEventSource {
    #[serde(rename = "userId")]
    user_id: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct LineEventMessage {
    #[serde(rename = "type")]
    message_type: String,
    text: Option<String>,
    id: Option<String>,
    #[serde(rename = "packageId")]
    package_id: Option<String>,
    #[serde(rename = "stickerId")]
    sticker_id: Option<String>,
}

#[derive(Deserialize)]
struct LineEventPostback {
    data: String,
}

pub struct ReceiveWebhookUseCase {
    shared_turns: Option<Arc<dyn crate::domain::web::TurnRepository>>,
    line_client: Arc<dyn LineClient>,
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    job_repo: Arc<dyn JobRepository>,
    credit_repo: Arc<dyn CreditRepository>,
    config_repo: Arc<dyn AppConfigRepository>,
    job_sender: mpsc::Sender<JobId>,
    liff_base_url: String,
    rich_menu_0_id: String,
}

impl ReceiveWebhookUseCase {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        line_client: Arc<dyn LineClient>,
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn RoleplaySessionRepository>,
        job_repo: Arc<dyn JobRepository>,
        credit_repo: Arc<dyn CreditRepository>,
        config_repo: Arc<dyn AppConfigRepository>,
        job_sender: mpsc::Sender<JobId>,
        liff_base_url: String,
        rich_menu_0_id: String,
    ) -> Self {
        Self {
            shared_turns: None,
            line_client,
            user_repo,
            session_repo,
            job_repo,
            credit_repo,
            config_repo,
            job_sender,
            liff_base_url,
            rich_menu_0_id,
        }
    }

    pub fn with_shared_turns(
        mut self,
        turns: Option<Arc<dyn crate::domain::web::TurnRepository>>,
    ) -> Self {
        self.shared_turns = turns;
        self
    }

    async fn resolve_welcome_credits(&self) -> Result<i32, UsecaseError> {
        let value = self
            .config_repo
            .get("welcome_credits")
            .await?
            .ok_or_else(|| {
                UsecaseError::Infra(anyhow::anyhow!("Missing app_config: welcome_credits"))
            })?;
        value.parse::<i32>().map_err(|e| {
            UsecaseError::Infra(anyhow::anyhow!("Invalid app_config welcome_credits: {}", e))
        })
    }

    /// Verify the webhook signature (CPU-only, <1ms).
    pub fn verify_signature(&self, body: &[u8], signature: &str) -> Result<bool, UsecaseError> {
        self.line_client
            .verify_signature(body, signature)
            .map_err(UsecaseError::from)
    }

    /// Deserialize and process all events from a verified webhook body.
    /// Intended to be called from a `tokio::spawn` background task.
    pub async fn process_events(&self, body: Vec<u8>) -> Result<(), UsecaseError> {
        let webhook_body: LineWebhookBody =
            serde_json::from_slice(&body).map_err(|e| UsecaseError::Validation(e.to_string()))?;

        for event in &webhook_body.events {
            match self.route_event(event).await {
                Ok(Some(_job_id)) => {}
                Ok(None) => {}
                Err(e) => {
                    let line_user_id = event
                        .source
                        .as_ref()
                        .and_then(|s| s.user_id.as_deref())
                        .unwrap_or("unknown");
                    tracing::error!(
                        error = %e,
                        event_type = %event.event_type,
                        line_user_id,
                        "Failed to process webhook event"
                    );
                }
            }
        }

        Ok(())
    }

    async fn route_event(&self, event: &LineEvent) -> Result<Option<JobId>, UsecaseError> {
        match event.event_type.as_str() {
            "message" => self.handle_message_event(event).await,
            "follow" => self.handle_follow_event(event).await,
            "unfollow" => self.handle_unfollow_event(event).await,
            "postback" => self.handle_postback_event(event).await.map(Some),
            other => {
                tracing::debug!(event_type = other, "Skipping unhandled event type");
                Err(UsecaseError::Validation(format!(
                    "Unhandled event type: {}",
                    other
                )))
            }
        }
    }

    async fn handle_message_event(&self, event: &LineEvent) -> Result<Option<JobId>, UsecaseError> {
        let message = event
            .message
            .as_ref()
            .ok_or_else(|| UsecaseError::Validation("Missing message in message event".into()))?;

        let line_user_id = Self::extract_user_id(event)?;
        let reply_token = event.reply_token.as_deref();

        match message.message_type.as_str() {
            "text" => {
                let text = message.text.as_deref().unwrap_or_default().to_string();

                let reply_token = reply_token.ok_or_else(|| {
                    UsecaseError::Validation("Missing replyToken in message event".into())
                })?;

                let user = self.sync_user_from_line(line_user_id).await?;

                // Check if user has accepted terms
                if !user.has_accepted_terms() {
                    tracing::info!(
                        line_user_id = line_user_id,
                        "User has not accepted terms, sending registration Flex Message"
                    );
                    let scenes_url = format!("{}/scenes", self.liff_base_url);
                    let flex_contents =
                        flex_messages::build_registration_required_flex(&scenes_url);
                    let messages = vec![LineMessage::Flex {
                        alt_text: "กรุณาลงทะเบียนก่อนเล่นนะคะ".to_string(),
                        contents: flex_contents,
                        sender_name: String::new(),
                        sender_icon_url: String::new(),
                        quick_reply: None,
                    }];
                    if let Err(e) = self.line_client.reply_messages(reply_token, messages).await {
                        tracing::warn!(
                            error = %e,
                            line_user_id = line_user_id,
                            "Failed to send registration required Flex Message"
                        );
                    }
                    return Ok(None);
                }

                let session = match self.session_repo.find_active_by_user_id(user.id()).await? {
                    Some(s) => s,
                    None => {
                        let messages = vec![LineMessage::Text {
                            text: "ยังไม่มีเรื่องราวที่กำลังเล่นอยู่ กดเมนูด้านล่างเพื่อเลือกฉากและเริ่มเล่นเลย!"
                                .to_string(),
                            sender_name: String::new(),
                            sender_icon_url: String::new(),
                        }];
                        if let Err(e) = self.line_client.reply_messages(reply_token, messages).await
                        {
                            tracing::warn!(error = %e, "Failed to send no-session reply");
                        }
                        return Ok(None);
                    }
                };

                if let Some(turns) = &self.shared_turns {
                    let event_id = message.id.as_deref().ok_or_else(|| {
                        UsecaseError::Validation("Missing LINE message ID".into())
                    })?;
                    match turns
                        .admit(
                            *user.id().as_uuid(),
                            *session.id().as_uuid(),
                            "line",
                            "turn",
                            &format!("line:{event_id}"),
                            serde_json::json!({"content":text,"reply_token":reply_token}),
                        )
                        .await
                    {
                        Ok(_) => return Ok(None),
                        Err(error) => {
                            let message = match error {
                                crate::domain::web::WebError::Rejected("INSUFFICIENT_CREDITS") => {
                                    "เครดิตไม่พอ กรุณาเติมเครดิตแล้วลองอีกครั้ง"
                                }
                                crate::domain::web::WebError::Rejected("TURN_IN_PROGRESS") => {
                                    "กำลังตอบข้อความก่อนหน้าอยู่ รอสักครู่นะ"
                                }
                                _ => "ยังส่งข้อความไม่ได้ กรุณาลองอีกครั้ง",
                            };
                            let _ = self
                                .line_client
                                .reply_messages(
                                    reply_token,
                                    vec![LineMessage::Text {
                                        text: message.into(),
                                        sender_name: String::new(),
                                        sender_icon_url: String::new(),
                                    }],
                                )
                                .await;
                            return Ok(None);
                        }
                    }
                }

                // Fire loading animation only when session exists and we'll process the message
                let lc = Arc::clone(&self.line_client);
                let uid = line_user_id.to_string();
                tokio::spawn(async move {
                    if let Err(e) = lc.show_loading_animation(&uid, Some(20)).await {
                        tracing::warn!(error = %e, "Failed to show loading animation");
                    }
                });

                // Concurrency guard: reject if session already has an active job
                if self
                    .job_repo
                    .has_active_job_for_session(session.id())
                    .await?
                {
                    return Ok(None);
                }

                self.create_and_dispatch_job(
                    JobMode::RoleplayMessage,
                    Some(session.id().clone()),
                    user.id().clone(),
                    line_user_id.to_string(),
                    text,
                    Some(reply_token.to_string()),
                )
                .await
                .map(Some)
            }
            "sticker" => {
                let _user = self.sync_user_from_line(line_user_id).await?;
                if let Some(token) = reply_token {
                    let messages = vec![LineMessage::Text {
                        text: "ตอนนี้รองรับเฉพาะข้อความตัวอักษรเท่านั้นนะ ลองพิมพ์ข้อความมาแทนสติกเกอร์ดูนะ!"
                            .to_string(),
                        sender_name: String::new(),
                        sender_icon_url: String::new(),
                    }];
                    if let Err(e) = self.line_client.reply_messages(token, messages).await {
                        tracing::warn!(error = %e, "Failed to send sticker reply");
                    }
                }
                Ok(None)
            }
            "image" => {
                let _user = self.sync_user_from_line(line_user_id).await?;
                if let Some(token) = reply_token {
                    let messages = vec![LineMessage::Text {
                        text: "ตอนนี้รองรับเฉพาะข้อความตัวอักษรเท่านั้นนะ ลองพิมพ์ข้อความมาแทนรูปภาพดูนะ!"
                            .to_string(),
                        sender_name: String::new(),
                        sender_icon_url: String::new(),
                    }];
                    if let Err(e) = self.line_client.reply_messages(token, messages).await {
                        tracing::warn!(error = %e, "Failed to send image reply");
                    }
                }
                Ok(None)
            }
            other => {
                tracing::debug!(message_type = other, "Skipping unhandled message type");
                Err(UsecaseError::Validation(format!(
                    "Unhandled message type: {}",
                    other
                )))
            }
        }
    }

    async fn handle_follow_event(&self, event: &LineEvent) -> Result<Option<JobId>, UsecaseError> {
        let line_user_id = Self::extract_user_id(event)?;

        let reply_token = event
            .reply_token
            .as_deref()
            .ok_or_else(|| UsecaseError::Validation("Missing replyToken in follow event".into()))?;

        let is_unblocked = event
            .follow
            .as_ref()
            .map(|f| f.is_unblocked)
            .unwrap_or(false);

        let user = self.sync_user_from_line(line_user_id).await?;

        // Build welcome Flex Message with CTA button → LIFF /scenes
        let scenes_url = format!("{}/scenes", self.liff_base_url);
        let hero_image_url = self
            .config_repo
            .get("welcome_hero_image_url")
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to read welcome_hero_image_url config");
                None
            });
        let flex_contents =
            flex_messages::build_welcome_flex(&scenes_url, hero_image_url.as_deref());
        let messages = vec![LineMessage::Flex {
            alt_text: "ยินดีต้อนรับสู่ TalkRai! เลือกตัวละครที่ชอบเลย".to_string(),
            contents: flex_contents,
            sender_name: String::new(),
            sender_icon_url: String::new(),
            quick_reply: None,
        }];

        // Send Flex Message — non-fatal (reply token may have expired)
        if let Err(e) = self.line_client.reply_messages(reply_token, messages).await {
            tracing::warn!(
                error = %e,
                line_user_id = line_user_id,
                "Failed to send follow welcome Flex Message"
            );
        }

        // Assign Rich Menu State 0 (best-effort)
        if let Err(e) = self
            .line_client
            .link_rich_menu(line_user_id, &self.rich_menu_0_id)
            .await
        {
            tracing::warn!(
                error = %e,
                line_user_id = line_user_id,
                "Failed to link Rich Menu State 0 after follow"
            );
        }

        tracing::info!(
            user_id = %user.id().as_uuid(),
            line_user_id = line_user_id,
            is_unblocked = is_unblocked,
            "Follow event handled successfully — welcome Flex Message sent, Rich Menu 0 linked"
        );

        Ok(None)
    }

    async fn handle_unfollow_event(
        &self,
        event: &LineEvent,
    ) -> Result<Option<JobId>, UsecaseError> {
        let line_user_id = Self::extract_user_id(event)?;

        // For unfollow, don't call LINE API — just find existing user
        let user = match self.user_repo.find_by_line_user_id(line_user_id).await? {
            Some(u) => u,
            None => {
                tracing::warn!(
                    line_user_id = line_user_id,
                    "Unfollow event for unknown user — skipping"
                );
                return Ok(None);
            }
        };

        // Friendship changes must persist even when a shared turn holds the story lease.
        let mut user = user;
        user.deactivate();
        self.user_repo.update(&user).await?;
        if self.shared_turns.is_none() {
            if let Some(mut session) = self.session_repo.find_active_by_user_id(user.id()).await? {
                session.end()?;
                self.session_repo.update(&session).await?;
            }
        }

        tracing::info!(
            user_id = %user.id().as_uuid(),
            line_user_id = line_user_id,
            "Unfollow event handled — user deactivated"
        );

        Ok(None)
    }

    async fn handle_postback_event(&self, event: &LineEvent) -> Result<JobId, UsecaseError> {
        let line_user_id = Self::extract_user_id(event)?;
        let user = self.sync_user_from_line(line_user_id).await?;

        let postback_data = event
            .postback
            .as_ref()
            .map(|p| p.data.clone())
            .unwrap_or_default();

        self.create_and_dispatch_job(
            JobMode::PostbackEvent,
            None,
            user.id().clone(),
            line_user_id.to_string(),
            postback_data,
            None,
        )
        .await
    }

    fn extract_user_id(event: &LineEvent) -> Result<&str, UsecaseError> {
        event
            .source
            .as_ref()
            .and_then(|s| s.user_id.as_deref())
            .ok_or_else(|| UsecaseError::Validation("Missing source userId".to_string()))
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_and_dispatch_job(
        &self,
        mode: JobMode,
        session_id: Option<SessionId>,
        user_id: UserId,
        line_user_id: String,
        user_message: String,
        reply_token: Option<String>,
    ) -> Result<JobId, UsecaseError> {
        let job = Job::new(mode, session_id, user_id, line_user_id, user_message)
            .with_reply_token(reply_token);
        let job_id = job.id().clone();

        self.job_repo.create(&job).await?;

        if let Err(e) = self.job_sender.try_send(job_id.clone()) {
            tracing::warn!(error = %e, "Job channel full, job will be picked up by poller");
        }

        Ok(job_id)
    }

    async fn sync_user_from_line(&self, line_user_id: &str) -> Result<User, UsecaseError> {
        // Fast path: known user → return from DB immediately, skip LINE API
        if let Some(mut user) = self.user_repo.find_by_line_user_id(line_user_id).await? {
            // Reactivate if inactive (re-follow after unfollow)
            if !user.is_active() {
                user.activate();
                self.user_repo.update(&user).await?;
                tracing::info!(
                    user_id = %user.id().as_uuid(),
                    "User reactivated on re-follow"
                );
            }

            // Ensure credit balance exists (may be missing for re-followed users)
            if self
                .credit_repo
                .find_balance_by_user_id(user.id())
                .await?
                .is_none()
            {
                let welcome_credits = self.resolve_welcome_credits().await?;
                let balance = CreditBalance::new(user.id().clone(), welcome_credits);
                self.credit_repo.create_balance(&balance).await?;
                tracing::info!(
                    user_id = %user.id().as_uuid(),
                    "Restored missing credit balance with {} welcome credits",
                    welcome_credits
                );
            }
            return Ok(user);
        }

        // Slow path: new user → call LINE Profile API
        let profile = self.line_client.get_profile(line_user_id).await?;
        let user = User::new(
            line_user_id.to_string(),
            profile.display_name,
            profile.picture_url,
        );
        self.user_repo.upsert(&user).await?;
        let user = self
            .user_repo
            .find_by_line_user_id(line_user_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("User not found after upsert".into()))?;

        let welcome_credits = self.resolve_welcome_credits().await?;
        let balance = CreditBalance::new(user.id().clone(), welcome_credits);
        self.credit_repo.create_balance(&balance).await?;

        tracing::info!(
            user_id = %user.id().as_uuid(),
            "New user created with {} welcome credits",
            welcome_credits
        );

        Ok(user)
    }
}
