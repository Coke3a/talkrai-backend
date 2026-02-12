use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::domain::services::ai_client::{AiClient, AiRoleplayRequest, AiRoleplayResponse};
use crate::domain::services::AiClientError;

use super::response::parse_llm_response;

const VENICE_API_URL: &str = "https://api.venice.ai/api/v1/chat/completions";
const VENICE_MODEL: &str = "venice-uncensored";

// --- Request types ---

#[derive(Serialize)]
struct VeniceRequest {
    model: &'static str,
    max_tokens: u32,
    messages: Vec<VeniceMessage>,
}

#[derive(Serialize)]
struct VeniceMessage {
    role: String,
    content: String,
}

// --- Response types ---

#[derive(Deserialize)]
struct VeniceResponse {
    choices: Vec<VeniceChoice>,
}

#[derive(Deserialize)]
struct VeniceChoice {
    message: VeniceChoiceMessage,
}

#[derive(Deserialize)]
struct VeniceChoiceMessage {
    content: Option<String>,
}

pub struct VeniceClient {
    http: Client,
    api_key: String,
}

impl VeniceClient {
    pub fn new(api_key: String) -> Self {
        Self {
            http: Client::new(),
            api_key,
        }
    }
}

#[async_trait]
impl AiClient for VeniceClient {
    /// https://docs.venice.ai/api-reference/chat-completions
    async fn generate_roleplay_response(
        &self,
        request: AiRoleplayRequest,
    ) -> Result<AiRoleplayResponse, AiClientError> {
        let mut messages = Vec::with_capacity(request.messages.len() + 1);

        // System prompt as first message with role "system" (OpenAI-compatible)
        messages.push(VeniceMessage {
            role: "system".into(),
            content: request.system_prompt,
        });

        for m in request.messages {
            messages.push(VeniceMessage {
                role: m.role,
                content: m.content,
            });
        }

        let body = VeniceRequest {
            model: VENICE_MODEL,
            max_tokens: request.max_tokens,
            messages,
        };

        let response = self
            .http
            .post(VENICE_API_URL)
            .header("authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AiClientError::NetworkError(e.into()))?;

        let status = response.status().as_u16();
        if status == 429 {
            return Err(AiClientError::RateLimited);
        }
        if !response.status().is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".into());
            return Err(AiClientError::ApiError { status, message });
        }

        let venice_resp: VeniceResponse = response
            .json()
            .await
            .map_err(|e| AiClientError::ParseError(format!("Failed to deserialize Venice response: {e}")))?;

        let text = venice_resp
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .ok_or_else(|| AiClientError::ParseError("No content in Venice response".into()))?;

        parse_llm_response(text)
    }
}
