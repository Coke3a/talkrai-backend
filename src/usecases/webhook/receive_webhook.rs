use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::mpsc;

use crate::domain::entities::{CreditBalance, Job, User};
use crate::domain::repositories::{
    AppConfigRepository, CreditRepository, JobRepository, RoleplaySessionRepository, UserRepository,
};
use crate::domain::services::line_client::{LineClient, LineReplyMessage};
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

                // Fire loading animation (non-blocking, fire-and-forget)
                let lc = Arc::clone(&self.line_client);
                let uid = line_user_id.to_string();
                tokio::spawn(async move {
                    if let Err(e) = lc.show_loading_animation(&uid, Some(20)).await {
                        tracing::warn!(error = %e, "Failed to show loading animation");
                    }
                });

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
                    let messages = vec![LineReplyMessage::Flex {
                        alt_text: "กรุณาลงทะเบียนก่อนเล่นนะคะ".to_string(),
                        contents: flex_contents,
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
                        let messages = vec![LineReplyMessage::Text {
                            text: "ยังไม่มีเรื่องราวที่กำลังเล่นอยู่ กดเมนูด้านล่างเพื่อเลือกฉากและเริ่มเล่นเลย!"
                                .to_string(),
                        }];
                        if let Err(e) = self.line_client.reply_messages(reply_token, messages).await
                        {
                            tracing::warn!(error = %e, "Failed to send no-session reply");
                        }
                        return Ok(None);
                    }
                };

                self.create_and_dispatch_job(
                    JobMode::RoleplayMessage,
                    Some(session.id().clone()),
                    user.id().clone(),
                    line_user_id.to_string(),
                    text,
                )
                .await
                .map(Some)
            }
            "sticker" => {
                let _user = self.sync_user_from_line(line_user_id).await?;
                if let Some(token) = reply_token {
                    let messages = vec![LineReplyMessage::Text {
                        text: "ตอนนี้รองรับเฉพาะข้อความตัวอักษรเท่านั้นนะ ลองพิมพ์ข้อความมาแทนสติกเกอร์ดูนะ!"
                            .to_string(),
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
                    let messages = vec![LineReplyMessage::Text {
                        text: "ตอนนี้รองรับเฉพาะข้อความตัวอักษรเท่านั้นนะ ลองพิมพ์ข้อความมาแทนรูปภาพดูนะ!"
                            .to_string(),
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
        let flex_contents = flex_messages::build_welcome_flex(&scenes_url);
        let messages = vec![LineReplyMessage::Flex {
            alt_text: "ยินดีต้อนรับ! กดปุ่มเพื่อเริ่มเล่นเลย".to_string(),
            contents: flex_contents,
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

        // End active session if any
        if let Some(mut session) = self.session_repo.find_active_by_user_id(user.id()).await? {
            session.end()?;
            self.session_repo.update(&session).await?;
            tracing::info!(
                user_id = %user.id().as_uuid(),
                session_id = %session.id().as_uuid(),
                "Ended active session due to unfollow"
            );
        }

        tracing::info!(
            user_id = %user.id().as_uuid(),
            line_user_id = line_user_id,
            "Unfollow event handled successfully"
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

    async fn create_and_dispatch_job(
        &self,
        mode: JobMode,
        session_id: Option<SessionId>,
        user_id: UserId,
        line_user_id: String,
        user_message: String,
    ) -> Result<JobId, UsecaseError> {
        let job = Job::new(mode, session_id, user_id, line_user_id, user_message);
        let job_id = job.id().clone();

        self.job_repo.create(&job).await?;

        if let Err(e) = self.job_sender.try_send(job_id.clone()) {
            tracing::warn!(error = %e, "Job channel full, job will be picked up by poller");
        }

        Ok(job_id)
    }

    async fn sync_user_from_line(&self, line_user_id: &str) -> Result<User, UsecaseError> {
        // Fast path: known user → return from DB immediately, skip LINE API
        if let Some(user) = self.user_repo.find_by_line_user_id(line_user_id).await? {
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
