use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::domain::services::ai_client::{
    AiClient, AiRoleplayRequest, AiRoleplayResponse, AiSummaryRequest,
};
use crate::domain::services::AiClientError;

use serde_json::Value;

use super::response::{
    build_summary_system_prompt, claude_tool_definition, parse_llm_response, tool_args_to_response,
    UpdateSceneStateArgs,
};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<Value>,
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
    name: Option<String>,
    input: Option<Value>,
}

pub struct ClaudeClient {
    http: Client,
    api_key: String,
}

impl ClaudeClient {
    pub fn new(api_key: String) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build Claude HTTP client");
        Self { http, api_key }
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
            tools: Some(vec![claude_tool_definition()]),
            tool_choice: Some(serde_json::json!({"type": "any"})),
        };

        tracing::info!(
            provider = "claude",
            model = CLAUDE_MODEL,
            max_tokens = body.max_tokens,
            message_count = body.messages.len(),
            body = %serde_json::to_string(&body).unwrap_or_default(),
            "Sending roleplay request to LLM"
        );

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
        let response_text = response
            .text()
            .await
            .map_err(|e| AiClientError::NetworkError(e.into()))?;

        tracing::info!(
            provider = "claude",
            status,
            body = %response_text,
            "Received roleplay response from LLM"
        );

        if status >= 400 {
            return Err(AiClientError::ApiError {
                status,
                message: response_text,
            });
        }

        let claude_resp: ClaudeResponse = serde_json::from_str(&response_text).map_err(|e| {
            AiClientError::ParseError(format!("Failed to deserialize Claude response: {e}"))
        })?;

        // Try tool_use block first
        if let Some(tool_block) = claude_resp
            .content
            .iter()
            .find(|b| b.block_type == "tool_use" && b.name.as_deref() == Some("update_scene_state"))
        {
            if let Some(input) = &tool_block.input {
                let args: UpdateSceneStateArgs =
                    serde_json::from_value(input.clone()).map_err(|e| {
                        AiClientError::ParseError(format!("Failed to deserialize tool args: {e}"))
                    })?;
                return Ok(tool_args_to_response(args));
            }
        }

        // Fallback: text block
        tracing::warn!(
            provider = "claude",
            "No tool_use block found, falling back to text parsing"
        );
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
            tools: None,
            tool_choice: None,
        };

        tracing::info!(
            provider = "claude",
            model = CLAUDE_MODEL,
            max_tokens = body.max_tokens,
            message_count = body.messages.len(),
            body = %serde_json::to_string(&body).unwrap_or_default(),
            "Sending summary request to LLM"
        );

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
        let response_text = response
            .text()
            .await
            .map_err(|e| AiClientError::NetworkError(e.into()))?;

        tracing::info!(
            provider = "claude",
            status,
            body = %response_text,
            "Received summary response from LLM"
        );

        if status >= 400 {
            return Err(AiClientError::ApiError {
                status,
                message: response_text,
            });
        }

        let claude_resp: ClaudeResponse = serde_json::from_str(&response_text).map_err(|e| {
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
