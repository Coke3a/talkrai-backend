use async_trait::async_trait;
use base64::Engine;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use crate::domain::services::line_client::{
    LineClient, LineMessage, LineProfile, LineReplyMessage,
};
use crate::domain::services::LineClientError;

const LINE_API_PUSH: &str = "https://api.line.me/v2/bot/message/push";
const LINE_API_REPLY: &str = "https://api.line.me/v2/bot/message/reply";
const LINE_API_LOADING: &str = "https://api.line.me/v2/bot/chat/loading/start";
const LINE_API_PROFILE: &str = "https://api.line.me/v2/bot/profile";
const LINE_API_USER_PROFILE: &str = "https://api.line.me/v2/profile";
const LINE_API_TOKEN_VERIFY: &str = "https://api.line.me/oauth2/v2.1/verify";

/// Retry delays for transient errors (408, 429, 500).
const RETRY_DELAYS_MS: &[u64] = &[500, 1500];

// ---------------------------------------------------------------------------
// Serialization types (LINE Messaging API)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct PushMessageRequest {
    to: String,
    messages: Vec<serde_json::Value>,
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

/// Sanitize a display name for LINE's `sender.name` field.
///
/// LINE rejects control characters, zero-width characters, and names longer
/// than 20 characters. This strips those out while preserving normal text
/// (including Thai, Japanese, emoji, etc.).
fn sanitize_sender_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| {
            // Remove control characters (C0, DEL, C1)
            if c.is_control() {
                return false;
            }
            // Remove zero-width / invisible formatting characters
            !matches!(*c,
                '\u{200B}'..='\u{200F}' // zero-width space, ZWNJ, ZWJ, LRM, RLM
                | '\u{2028}'..='\u{2029}' // line/paragraph separator
                | '\u{202A}'..='\u{202E}' // bidi overrides
                | '\u{2060}'..='\u{2064}' // word joiner, invisible times, etc.
                | '\u{2066}'..='\u{2069}' // bidi isolates
                | '\u{FEFF}' // BOM / zero-width no-break space
                | '\u{FFF9}'..='\u{FFFB}' // interlinear annotations
            )
        })
        .collect();

    let trimmed = cleaned.trim();

    // Truncate to 20 characters (LINE's limit)
    if trimmed.chars().count() > 20 {
        trimmed
            .chars()
            .take(20)
            .collect::<String>()
            .trim_end()
            .to_string()
    } else {
        trimmed.to_string()
    }
}

/// Build the `sender` JSON object, omitting `name` when empty.
fn build_sender_object(sender_name: &str, sender_icon_url: &str) -> Value {
    let name = sanitize_sender_name(sender_name);
    if name.is_empty() {
        json!({ "iconUrl": sender_icon_url })
    } else {
        json!({ "name": name, "iconUrl": sender_icon_url })
    }
}

/// Build a LINE `quickReply` object from plain option texts, enforcing the
/// official LINE quick-reply spec. This is the single chokepoint that guarantees
/// every pushed quick reply is spec-valid:
/// - each item is trimmed; empty (or whitespace-only) items are dropped;
/// - the `label` must be ≤ 20 grapheme clusters — longer items are dropped, not
///   truncated (Phase 2 generation should already guarantee this; this guards
///   against bad data reaching the push boundary);
/// - at most 13 items are kept (LINE's hard limit);
/// - in our usage `label` == `text` == the option string, so the resulting
///   `text` is well within LINE's 300-char limit.
///
/// Returns `None` when no valid item remains, so the caller omits `quickReply`.
fn build_quick_reply_object(items: &[String]) -> Option<Value> {
    const MAX_ITEMS: usize = 13;
    const MAX_LABEL_GRAPHEMES: usize = 20;

    let actions: Vec<Value> = items
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty() && item.graphemes(true).count() <= MAX_LABEL_GRAPHEMES)
        .take(MAX_ITEMS)
        .map(|item| {
            json!({
                "type": "action",
                "action": {
                    "type": "message",
                    "label": item,
                    "text": item,
                }
            })
        })
        .collect();

    if actions.is_empty() {
        None
    } else {
        Some(json!({ "items": actions }))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadingAnimationRequest {
    chat_id: String,
    loading_seconds: u32,
}

#[derive(Deserialize)]
struct TokenVerifyResponse {
    client_id: String,
    #[allow(dead_code)]
    expires_in: i64,
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
    line_channel_id: String,
}

impl LineClientImpl {
    pub fn new(
        channel_secret: String,
        channel_access_token: String,
        line_channel_id: String,
    ) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("Failed to build LINE HTTP client");
        Self {
            http,
            channel_secret,
            channel_access_token,
            line_channel_id,
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
                .map(|m| match m {
                    LineMessage::Text {
                        text,
                        sender_name,
                        sender_icon_url,
                    } => {
                        let sender = build_sender_object(&sender_name, &sender_icon_url);
                        json!({
                            "type": "text",
                            "text": text,
                            "sender": sender
                        })
                    }
                    LineMessage::Flex {
                        alt_text,
                        contents,
                        sender_name,
                        sender_icon_url,
                        quick_reply,
                    } => {
                        let sender = build_sender_object(&sender_name, &sender_icon_url);
                        let mut message = json!({
                            "type": "flex",
                            "altText": alt_text,
                            "contents": contents,
                            "sender": sender
                        });
                        // Attach a quick reply only when the option texts survive
                        // LINE-spec validation; otherwise omit `quickReply` entirely.
                        if let Some(items) = quick_reply {
                            match build_quick_reply_object(&items) {
                                Some(qr) => message["quickReply"] = qr,
                                // Non-empty input but nothing survived validation:
                                // omit quickReply, but leave a trace so bad Phase-2
                                // data is observable rather than silently dropped.
                                None if !items.is_empty() => tracing::warn!(
                                    item_count = items.len(),
                                    "quick reply omitted: all items failed LINE-spec validation"
                                ),
                                None => {}
                            }
                        }
                        message
                    }
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
                tracing::error!(
                    status,
                    line_user_id = %body.to,
                    response_body = %message,
                    "LINE push_messages non-retryable error"
                );
                return Err(LineClientError::ApiError { status, message });
            }

            tracing::warn!(
                status,
                attempt = attempt + 1,
                "LINE push_messages transient error, will retry"
            );
            last_err = Some(LineClientError::ApiError { status, message });
        }

        let final_err = last_err.unwrap_or_else(|| {
            LineClientError::NetworkError(anyhow::anyhow!("push_messages: all retries exhausted"))
        });
        tracing::error!(
            line_user_id = %body.to,
            error = %final_err,
            "LINE push_messages failed after all retries"
        );
        Err(final_err)
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
            tracing::error!(
                status,
                line_user_id = %body.chat_id,
                response_body = %message,
                "LINE show_loading_animation failed"
            );
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
            tracing::error!(
                status,
                response_body = %message,
                "LINE reply_messages failed"
            );
            return Err(LineClientError::ApiError { status, message });
        }

        Ok(())
    }

    async fn verify_liff_token(&self, access_token: &str) -> Result<LineProfile, LineClientError> {
        // Step 1: Verify token validity and check client_id
        let verify_url = format!("{}?access_token={}", LINE_API_TOKEN_VERIFY, access_token);
        let verify_resp = self
            .http
            .get(&verify_url)
            .send()
            .await
            .map_err(|e| LineClientError::NetworkError(e.into()))?;

        if !verify_resp.status().is_success() {
            let status = verify_resp.status().as_u16();
            let message = verify_resp
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".into());
            tracing::error!(
                status,
                response_body = %message,
                "LINE LIFF token verification failed"
            );
            return Err(LineClientError::ApiError { status, message });
        }

        let verify_body: TokenVerifyResponse = verify_resp
            .json()
            .await
            .map_err(|e| LineClientError::NetworkError(e.into()))?;

        if verify_body.client_id != self.line_channel_id {
            return Err(LineClientError::ApiError {
                status: 401,
                message: "Token does not belong to this LIFF channel".into(),
            });
        }

        // Step 2: Get user profile
        let response = self
            .http
            .get(LINE_API_USER_PROFILE)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| LineClientError::NetworkError(e.into()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".into());
            tracing::error!(
                status,
                response_body = %message,
                "LINE user profile fetch failed during LIFF verification"
            );
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
            tracing::error!(
                status,
                line_user_id,
                response_body = %message,
                "LINE get_profile failed"
            );
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
            .header("Content-Length", "0")
            .send()
            .await
            .map_err(|e| LineClientError::NetworkError(e.into()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".into());
            tracing::error!(
                status,
                line_user_id,
                rich_menu_id,
                response_body = %message,
                "LINE link_rich_menu failed"
            );
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
            tracing::error!(
                status,
                line_user_id,
                response_body = %message,
                "LINE unlink_rich_menu failed"
            );
            return Err(LineClientError::ApiError { status, message });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_message_actions_for_valid_items() {
        let qr = build_quick_reply_object(&["สวัสดี".to_string(), "ทักทาย".to_string()])
            .expect("expected a quick reply object");
        let items = qr["items"].as_array().expect("items array");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["type"], "action");
        assert_eq!(items[0]["action"]["type"], "message");
        // In our usage label == text == the option string.
        assert_eq!(items[0]["action"]["label"], "สวัสดี");
        assert_eq!(items[0]["action"]["text"], "สวัสดี");
        assert_eq!(items[1]["action"]["label"], "ทักทาย");
    }

    #[test]
    fn drops_empty_and_whitespace_items_and_trims() {
        let qr =
            build_quick_reply_object(&["".to_string(), "   ".to_string(), "  ตอบ  ".to_string()])
                .expect("one valid item should remain");
        let items = qr["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["action"]["label"], "ตอบ");
    }

    #[test]
    fn drops_items_over_twenty_graphemes() {
        // 21 base Thai consonants => 21 grapheme clusters (over the limit) => dropped.
        let too_long = "ก".repeat(21);
        let ok = "ก".repeat(20);
        let qr = build_quick_reply_object(&[too_long, ok.clone()]).expect("one valid item");
        let items = qr["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["action"]["label"], ok);
    }

    #[test]
    fn counts_grapheme_clusters_not_code_points() {
        // "e" + U+0301 combining acute = 2 code points but 1 grapheme cluster.
        // 20 clusters (40 code points) must survive; 21 clusters must be dropped.
        let base = "e\u{0301}";
        let twenty = base.repeat(20);
        let twenty_one = base.repeat(21);
        assert_eq!(
            twenty.chars().count(),
            40,
            "sanity: code points != clusters"
        );
        let qr = build_quick_reply_object(&[twenty.clone(), twenty_one])
            .expect("the 20-cluster item survives");
        let items = qr["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["action"]["label"], twenty);
    }

    #[test]
    fn returns_none_when_no_valid_items() {
        let qr = build_quick_reply_object(&["".to_string(), "ก".repeat(25)]);
        assert!(qr.is_none());
    }

    #[test]
    fn returns_none_for_empty_input() {
        assert!(build_quick_reply_object(&[]).is_none());
    }

    #[test]
    fn caps_at_thirteen_items() {
        let many: Vec<String> = (0..20).map(|i| format!("ตอบ{i}")).collect();
        let qr = build_quick_reply_object(&many).expect("object");
        let items = qr["items"].as_array().unwrap();
        assert_eq!(items.len(), 13);
    }
}
