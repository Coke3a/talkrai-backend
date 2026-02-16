use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::domain::entities::{CreditBalance, Job, User};
use crate::domain::repositories::{
    AppConfigRepository, CreditRepository, JobRepository, RoleplaySessionRepository,
    UserRepository,
};
use crate::domain::services::line_client::{LineClient, LineReplyMessage};
use crate::domain::value_objects::{JobId, JobMode, SessionId, UserId};
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

pub struct ReceiveWebhookInput {
    pub body: Vec<u8>,
    pub signature: String,
}

pub struct ReceiveWebhookOutput {
    pub job_ids: Vec<Uuid>,
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
}

impl ReceiveWebhookUseCase {
    pub fn new(
        line_client: Arc<dyn LineClient>,
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn RoleplaySessionRepository>,
        job_repo: Arc<dyn JobRepository>,
        credit_repo: Arc<dyn CreditRepository>,
        config_repo: Arc<dyn AppConfigRepository>,
        job_sender: mpsc::Sender<JobId>,
        liff_base_url: String,
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
        }
    }

    async fn resolve_rich_menu_no_session(&self) -> Result<String, UsecaseError> {
        self.config_repo
            .get("rich_menu_no_session")
            .await?
            .ok_or_else(|| UsecaseError::Infra(anyhow::anyhow!("Missing app_config: rich_menu_no_session")))
    }

    async fn resolve_welcome_credits(&self) -> Result<i32, UsecaseError> {
        let value = self
            .config_repo
            .get("welcome_credits")
            .await?
            .ok_or_else(|| UsecaseError::Infra(anyhow::anyhow!("Missing app_config: welcome_credits")))?;
        value
            .parse::<i32>()
            .map_err(|e| UsecaseError::Infra(anyhow::anyhow!("Invalid app_config welcome_credits: {}", e)))
    }

    pub async fn execute(
        &self,
        input: ReceiveWebhookInput,
    ) -> Result<ReceiveWebhookOutput, UsecaseError> {
        let valid = self
            .line_client
            .verify_signature(&input.body, &input.signature)?;
        if !valid {
            return Err(UsecaseError::Validation(
                "Invalid webhook signature".to_string(),
            ));
        }

        let webhook_body: LineWebhookBody = serde_json::from_slice(&input.body)
            .map_err(|e| UsecaseError::Validation(e.to_string()))?;

        let mut job_ids = Vec::new();

        for event in &webhook_body.events {
            match self.route_event(event).await {
                Ok(Some(job_id)) => job_ids.push(*job_id.as_uuid()),
                Ok(None) => {}
                Err(e) => tracing::error!(error = %e, "Failed to process webhook event"),
            }
        }

        Ok(ReceiveWebhookOutput { job_ids })
    }

    async fn route_event(&self, event: &LineEvent) -> Result<Option<JobId>, UsecaseError> {
        match event.event_type.as_str() {
            "message" => self.handle_message_event(event).await.map(Some),
            "follow" => self.handle_follow_event(event).await,
            "unfollow" => self.handle_unfollow_event(event).await.map(Some),
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

    async fn handle_message_event(&self, event: &LineEvent) -> Result<JobId, UsecaseError> {
        let message = event
            .message
            .as_ref()
            .ok_or_else(|| UsecaseError::Validation("Missing message in message event".into()))?;

        let line_user_id = Self::extract_user_id(event)?;

        match message.message_type.as_str() {
            "text" => {
                let text = message.text.as_deref().unwrap_or_default().to_string();

                // Fire loading animation (non-blocking, fire-and-forget)
                let lc = Arc::clone(&self.line_client);
                let uid = line_user_id.to_string();
                tokio::spawn(async move {
                    if let Err(e) = lc.show_loading_animation(&uid, Some(20)).await {
                        tracing::warn!(error = %e, "Failed to show loading animation");
                    }
                });

                let user = self.sync_user_from_line(line_user_id).await?;

                let session = self
                    .session_repo
                    .find_active_by_user_id(user.id())
                    .await?
                    .ok_or_else(|| {
                        UsecaseError::NotFound("No active session for user".to_string())
                    })?;

                self.create_and_dispatch_job(
                    JobMode::RoleplayMessage,
                    Some(session.id().clone()),
                    user.id().clone(),
                    line_user_id.to_string(),
                    text,
                )
                .await
            }
            "sticker" => {
                let user = self.sync_user_from_line(line_user_id).await?;
                let sticker_info = format!(
                    "sticker:{}:{}",
                    message.package_id.as_deref().unwrap_or("unknown"),
                    message.sticker_id.as_deref().unwrap_or("unknown")
                );

                self.create_and_dispatch_job(
                    JobMode::StickerMessage,
                    None,
                    user.id().clone(),
                    line_user_id.to_string(),
                    sticker_info,
                )
                .await
            }
            "image" => {
                let user = self.sync_user_from_line(line_user_id).await?;
                let image_info = format!(
                    "image:{}",
                    message.id.as_deref().unwrap_or("unknown")
                );

                self.create_and_dispatch_job(
                    JobMode::ImageMessage,
                    None,
                    user.id().clone(),
                    line_user_id.to_string(),
                    image_info,
                )
                .await
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

        // Build greeting messages (Thai, playful/inviting tone)
        let welcome_text = "สวัสดีค่า~ ยินดีต้อนรับสู่ KhuiAI นะคะ ✨\nที่นี่คุณสามารถแชทกับตัวละคร AI สุดพิเศษได้แบบเรียลไทม์เลยค่ะ";
        let cta_text = format!(
            "เลือกตัวละครที่ชอบแล้วเริ่มแชทกันเลย!\n{}",
            self.liff_base_url
        );
        let messages = vec![
            LineReplyMessage { text: welcome_text.to_string() },
            LineReplyMessage { text: cta_text },
        ];

        // Send greeting — non-fatal (reply token may have expired)
        if let Err(e) = self.line_client.reply_messages(reply_token, messages).await {
            tracing::warn!(
                error = %e,
                line_user_id = line_user_id,
                "Failed to send follow greeting reply"
            );
        }

        // Link "no session" rich menu — non-fatal
        let rich_menu_id = self.resolve_rich_menu_no_session().await?;
        if let Err(e) = self
            .line_client
            .link_rich_menu(line_user_id, &rich_menu_id)
            .await
        {
            tracing::warn!(
                error = %e,
                line_user_id = line_user_id,
                "Failed to link no-session rich menu"
            );
        }

        tracing::info!(
            user_id = %user.id().as_uuid(),
            line_user_id = line_user_id,
            is_unblocked = is_unblocked,
            "Follow event handled successfully"
        );

        Ok(None)
    }

    async fn handle_unfollow_event(&self, event: &LineEvent) -> Result<JobId, UsecaseError> {
        let line_user_id = Self::extract_user_id(event)?;

        // For unfollow, don't call LINE API — just find existing user
        let user = self
            .user_repo
            .find_by_line_user_id(line_user_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("User not found for unfollow event".into()))?;

        self.create_and_dispatch_job(
            JobMode::UnfollowEvent,
            None,
            user.id().clone(),
            line_user_id.to_string(),
            String::new(),
        )
        .await
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
        let profile = self.line_client.get_profile(line_user_id).await?;

        match self.user_repo.find_by_line_user_id(line_user_id).await? {
            Some(mut user) => {
                user.update_profile(profile.display_name, profile.picture_url);
                self.user_repo.upsert(&user).await?;
                Ok(user)
            }
            None => {
                let user =
                    User::new(line_user_id.to_string(), profile.display_name, profile.picture_url);
                self.user_repo.upsert(&user).await?;

                let welcome_credits = self.resolve_welcome_credits().await?;
                let balance =
                    CreditBalance::new(user.id().clone(), welcome_credits);
                self.credit_repo.create_balance(&balance).await?;

                tracing::info!(
                    user_id = %user.id().as_uuid(),
                    "New user created with {} welcome credits",
                    welcome_credits
                );

                Ok(user)
            }
        }
    }
}
