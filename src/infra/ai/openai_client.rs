use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::domain::services::ai_client::{
    AiClient, AiRoleplayRequest, AiRoleplayResponse, AiSummaryRequest,
};
use crate::domain::services::AiClientError;

use serde_json::Value;

use super::response::{
    build_summary_system_prompt, openai_tool_definition, parse_llm_response, tool_args_to_response,
    UpdateSceneStateArgs,
};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
const OPENAI_MODEL: &str = "gpt-4o-mini";

// --- Request types ---

#[derive(Serialize)]
struct OpenAiRequest {
    model: &'static str,
    max_tokens: u32,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<Value>,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

// --- Response types ---

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
}

#[derive(Deserialize)]
struct OpenAiChoiceMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Deserialize)]
struct OpenAiToolCall {
    function: OpenAiToolCallFunction,
}

#[derive(Deserialize)]
struct OpenAiToolCallFunction {
    name: String,
    arguments: String,
}

pub struct OpenAiClient {
    http: Client,
    api_key: String,
}

impl OpenAiClient {
    pub fn new(api_key: String) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build OpenAI HTTP client");
        Self { http, api_key }
    }
}

#[async_trait]
impl AiClient for OpenAiClient {
    /// https://platform.openai.com/docs/api-reference/chat/create
    async fn generate_roleplay_response(
        &self,
        request: AiRoleplayRequest,
    ) -> Result<AiRoleplayResponse, AiClientError> {
        let mut messages = Vec::with_capacity(request.messages.len() + 1);

        // System prompt as first message with role "system"
        messages.push(OpenAiMessage {
            role: "system".into(),
            content: request.system_prompt,
        });

        for m in request.messages {
            messages.push(OpenAiMessage {
                role: m.role,
                content: m.content,
            });
        }

        let body = OpenAiRequest {
            model: OPENAI_MODEL,
            max_tokens: request.max_tokens,
            messages,
            tools: Some(vec![openai_tool_definition()]),
            tool_choice: Some(serde_json::json!("required")),
        };

        tracing::debug!(
            provider = "openai",
            model = OPENAI_MODEL,
            max_tokens = body.max_tokens,
            message_count = body.messages.len(),
            body = %serde_json::to_string(&body).unwrap_or_default(),
            "Sending roleplay request to LLM"
        );

        let response = self
            .http
            .post(OPENAI_API_URL)
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
        let response_text = response
            .text()
            .await
            .map_err(|e| AiClientError::NetworkError(e.into()))?;

        tracing::debug!(
            provider = "openai",
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

        let openai_resp: OpenAiResponse = serde_json::from_str(&response_text).map_err(|e| {
            AiClientError::ParseError(format!("Failed to deserialize OpenAI response: {e}"))
        })?;

        // Try tool call first
        if let Some(choice) = openai_resp.choices.first() {
            if let Some(tool_calls) = &choice.message.tool_calls {
                if let Some(tc) = tool_calls
                    .iter()
                    .find(|tc| tc.function.name == "update_scene_state")
                {
                    let args: UpdateSceneStateArgs = serde_json::from_str(&tc.function.arguments)
                        .map_err(|e| {
                        AiClientError::ParseError(format!("Failed to deserialize tool args: {e}"))
                    })?;
                    return Ok(tool_args_to_response(args));
                }
            }
        }

        // Fallback: text content
        tracing::warn!(
            provider = "openai",
            "No tool call found, falling back to text parsing"
        );
        let text = openai_resp
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .ok_or_else(|| AiClientError::ParseError("No content in OpenAI response".into()))?;

        parse_llm_response(text)
    }

    async fn generate_summary(&self, request: AiSummaryRequest) -> Result<String, AiClientError> {
        let system_prompt = build_summary_system_prompt(&request.existing_summary);

        let mut messages = Vec::with_capacity(request.messages_to_summarize.len() + 1);
        messages.push(OpenAiMessage {
            role: "system".into(),
            content: system_prompt,
        });
        for m in request.messages_to_summarize {
            messages.push(OpenAiMessage {
                role: m.role,
                content: m.content,
            });
        }

        let body = OpenAiRequest {
            model: OPENAI_MODEL,
            max_tokens: request.max_tokens,
            messages,
            tools: None,
            tool_choice: None,
        };

        tracing::debug!(
            provider = "openai",
            model = OPENAI_MODEL,
            max_tokens = body.max_tokens,
            message_count = body.messages.len(),
            body = %serde_json::to_string(&body).unwrap_or_default(),
            "Sending summary request to LLM"
        );

        let response = self
            .http
            .post(OPENAI_API_URL)
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
        let response_text = response
            .text()
            .await
            .map_err(|e| AiClientError::NetworkError(e.into()))?;

        tracing::debug!(
            provider = "openai",
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

        let openai_resp: OpenAiResponse = serde_json::from_str(&response_text).map_err(|e| {
            AiClientError::ParseError(format!(
                "Failed to deserialize OpenAI summary response: {e}"
            ))
        })?;

        let text = openai_resp
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .ok_or_else(|| {
                AiClientError::ParseError("No content in OpenAI summary response".into())
            })?;

        Ok(text.trim().to_string())
    }
}
