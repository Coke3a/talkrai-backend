use async_trait::async_trait;
use base64::Engine;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::domain::services::line_client::{
    LineClient, LineMessage, LineProfile, LineReplyMessage,
};
use crate::domain::services::LineClientError;

const LINE_API_PUSH: &str = "https://api.line.me/v2/bot/message/push";
const LINE_API_REPLY: &str = "https://api.line.me/v2/bot/message/reply";
const LINE_API_LOADING: &str = "https://api.line.me/v2/bot/chat/loading/start";
const LINE_API_PROFILE: &str = "https://api.line.me/v2/bot/profile";

/// Retry delays for transient errors (408, 429, 500).
const RETRY_DELAYS_MS: &[u64] = &[500, 1500];

// ---------------------------------------------------------------------------
// Serialization types (LINE Messaging API)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct PushMessageRequest {
    to: String,
    messages: Vec<TextMessageObject>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplyMessageRequest {
    reply_token: String,
    messages: Vec<ReplyMessageObject>,
}

#[derive(Serialize, Clone)]
#[serde(tag = "type")]
enum ReplyMessageObject {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "flex")]
    Flex {
        #[serde(rename = "altText")]
        alt_text: String,
        contents: serde_json::Value,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextMessageObject {
    #[serde(rename = "type")]
    msg_type: &'static str,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sender: Option<SenderOverride>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SenderOverride {
    name: String,
    icon_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadingAnimationRequest {
    chat_id: String,
    loading_seconds: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileResponse {
    user_id: String,
    display_name: String,
    picture_url: Option<String>,
    language: Option<String>,
}

// ---------------------------------------------------------------------------
// LineClientImpl
// ---------------------------------------------------------------------------

pub struct LineClientImpl {
    http: Client,
    channel_secret: String,
    channel_access_token: String,
}

impl LineClientImpl {
    pub fn new(channel_secret: String, channel_access_token: String) -> Self {
        Self {
            http: Client::new(),
            channel_secret,
            channel_access_token,
        }
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.channel_access_token)
    }

    /// Returns true if the status code is transient and worth retrying.
    fn is_retryable(status: u16) -> bool {
        matches!(status, 408 | 429 | 500)
    }
}

#[async_trait]
impl LineClient for LineClientImpl {
    fn verify_signature(&self, body: &[u8], signature: &str) -> Result<bool, LineClientError> {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.channel_secret.as_bytes())
            .map_err(|e| LineClientError::NetworkError(anyhow::anyhow!("HMAC init error: {e}")))?;
        mac.update(body);
        let computed = mac.finalize().into_bytes();
        let computed_b64 = base64::engine::general_purpose::STANDARD.encode(computed);
        Ok(computed_b64 == signature)
    }

    async fn push_messages(
        &self,
        line_user_id: &str,
        messages: Vec<LineMessage>,
    ) -> Result<(), LineClientError> {
        let body = PushMessageRequest {
            to: line_user_id.to_string(),
            messages: messages
                .into_iter()
                .map(|m| TextMessageObject {
                    msg_type: "text",
                    text: m.text,
                    sender: Some(SenderOverride {
                        name: m.sender_name,
                        icon_url: m.sender_icon_url,
                    }),
                })
                .collect(),
        };

        // Same retry key across all attempts for idempotency
        let retry_key = Uuid::new_v4().to_string();

        let mut last_err: Option<LineClientError> = None;
        let max_attempts = 1 + RETRY_DELAYS_MS.len();

        for attempt in 0..max_attempts {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(
                    RETRY_DELAYS_MS[attempt - 1],
                ))
                .await;
            }

            let response = self
                .http
                .post(LINE_API_PUSH)
                .header("Authorization", self.auth_header())
                .header("X-Line-Retry-Key", &retry_key)
                .json(&body)
                .send()
                .await
                .map_err(|e| LineClientError::NetworkError(e.into()))?;

            let status = response.status().as_u16();

            if response.status().is_success() {
                return Ok(());
            }

            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".into());

            // Non-retryable errors — bail immediately
            if !Self::is_retryable(status) {
                return Err(LineClientError::ApiError { status, message });
            }

            tracing::warn!(
                status,
                attempt = attempt + 1,
                "LINE push_messages transient error, will retry"
            );
            last_err = Some(LineClientError::ApiError { status, message });
        }

        Err(last_err.unwrap_or_else(|| {
            LineClientError::NetworkError(anyhow::anyhow!("push_messages: all retries exhausted"))
        }))
    }

    async fn show_loading_animation(
        &self,
        line_user_id: &str,
        loading_seconds: Option<u32>,
    ) -> Result<(), LineClientError> {
        let body = LoadingAnimationRequest {
            chat_id: line_user_id.to_string(),
            loading_seconds: loading_seconds.unwrap_or(20),
        };

        let response = self
            .http
            .post(LINE_API_LOADING)
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| LineClientError::NetworkError(e.into()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".into());
            return Err(LineClientError::ApiError { status, message });
        }

        Ok(())
    }

    async fn reply_messages(
        &self,
        reply_token: &str,
        messages: Vec<LineReplyMessage>,
    ) -> Result<(), LineClientError> {
        let body = ReplyMessageRequest {
            reply_token: reply_token.to_string(),
            messages: messages
                .into_iter()
                .map(|m| match m {
                    LineReplyMessage::Text { text } => ReplyMessageObject::Text { text },
                    LineReplyMessage::Flex { alt_text, contents } => {
                        ReplyMessageObject::Flex { alt_text, contents }
                    }
                })
                .collect(),
        };

        let response = self
            .http
            .post(LINE_API_REPLY)
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| LineClientError::NetworkError(e.into()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".into());
            return Err(LineClientError::ApiError { status, message });
        }

        Ok(())
    }

    async fn get_profile(&self, line_user_id: &str) -> Result<LineProfile, LineClientError> {
        let url = format!("{}/{}", LINE_API_PROFILE, line_user_id);

        let response = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| LineClientError::NetworkError(e.into()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".into());
            return Err(LineClientError::ApiError { status, message });
        }

        let profile: ProfileResponse = response
            .json()
            .await
            .map_err(|e| LineClientError::NetworkError(e.into()))?;

        Ok(LineProfile {
            user_id: profile.user_id,
            display_name: profile.display_name,
            picture_url: profile.picture_url,
            language: profile.language,
        })
    }

    async fn link_rich_menu(
        &self,
        line_user_id: &str,
        rich_menu_id: &str,
    ) -> Result<(), LineClientError> {
        let url = format!(
            "https://api.line.me/v2/bot/user/{}/richmenu/{}",
            line_user_id, rich_menu_id
        );

        let response = self
            .http
            .post(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| LineClientError::NetworkError(e.into()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".into());
            return Err(LineClientError::ApiError { status, message });
        }

        Ok(())
    }

    async fn unlink_rich_menu(&self, line_user_id: &str) -> Result<(), LineClientError> {
        let url = format!("https://api.line.me/v2/bot/user/{}/richmenu", line_user_id);

        let response = self
            .http
            .delete(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| LineClientError::NetworkError(e.into()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".into());
            return Err(LineClientError::ApiError { status, message });
        }

        Ok(())
    }
}
