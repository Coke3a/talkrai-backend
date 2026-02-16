use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::domain::services::ai_client::{
    AiClient, AiRoleplayRequest, AiRoleplayResponse, AiSummaryRequest,
};
use crate::domain::services::AiClientError;

use super::response::{build_summary_system_prompt, parse_llm_response};

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const CLAUDE_MODEL: &str = "claude-sonnet-4-5-20250929";
const ANTHROPIC_VERSION: &str = "2023-06-01";

// --- Request types ---

#[derive(Serialize)]
struct ClaudeRequest {
    model: &'static str,
    max_tokens: u32,
    system: String,
    messages: Vec<ClaudeMessage>,
}

#[derive(Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

// --- Response types ---

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContentBlock>,
}

#[derive(Deserialize)]
struct ClaudeContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

pub struct ClaudeClient {
    http: Client,
    api_key: String,
}

impl ClaudeClient {
    pub fn new(api_key: String) -> Self {
        Self {
            http: Client::new(),
            api_key,
        }
    }
}

#[async_trait]
impl AiClient for ClaudeClient {
    /// https://docs.anthropic.com/en/api/messages
    async fn generate_roleplay_response(
        &self,
        request: AiRoleplayRequest,
    ) -> Result<AiRoleplayResponse, AiClientError> {
        let body = ClaudeRequest {
            model: CLAUDE_MODEL,
            max_tokens: request.max_tokens,
            system: request.system_prompt,
            messages: request
                .messages
                .into_iter()
                .map(|m| ClaudeMessage {
                    role: m.role,
                    content: m.content,
                })
                .collect(),
        };

        let response = self
            .http
            .post(CLAUDE_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
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

        let claude_resp: ClaudeResponse = response.json().await.map_err(|e| {
            AiClientError::ParseError(format!("Failed to deserialize Claude response: {e}"))
        })?;

        let text = claude_resp
            .content
            .iter()
            .find(|b| b.block_type == "text")
            .and_then(|b| b.text.as_deref())
            .ok_or_else(|| AiClientError::ParseError("No text block in Claude response".into()))?;

        parse_llm_response(text)
    }

    async fn generate_summary(&self, request: AiSummaryRequest) -> Result<String, AiClientError> {
        let system_prompt = build_summary_system_prompt(&request.existing_summary);

        let body = ClaudeRequest {
            model: CLAUDE_MODEL,
            max_tokens: request.max_tokens,
            system: system_prompt,
            messages: request
                .messages_to_summarize
                .into_iter()
                .map(|m| ClaudeMessage {
                    role: m.role,
                    content: m.content,
                })
                .collect(),
        };

        let response = self
            .http
            .post(CLAUDE_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
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

        let claude_resp: ClaudeResponse = response.json().await.map_err(|e| {
            AiClientError::ParseError(format!(
                "Failed to deserialize Claude summary response: {e}"
            ))
        })?;

        let text = claude_resp
            .content
            .iter()
            .find(|b| b.block_type == "text")
            .and_then(|b| b.text.as_deref())
            .ok_or_else(|| {
                AiClientError::ParseError("No text block in Claude summary response".into())
            })?;

        Ok(text.trim().to_string())
    }
}
