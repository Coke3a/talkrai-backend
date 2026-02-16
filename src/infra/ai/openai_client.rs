use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::domain::services::ai_client::{
    AiClient, AiRoleplayRequest, AiRoleplayResponse, AiSummaryRequest,
};
use crate::domain::services::AiClientError;

use super::response::{build_summary_system_prompt, parse_llm_response};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
const OPENAI_MODEL: &str = "gpt-4o-mini";

// --- Request types ---

#[derive(Serialize)]
struct OpenAiRequest {
    model: &'static str,
    max_tokens: u32,
    messages: Vec<OpenAiMessage>,
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
}

pub struct OpenAiClient {
    http: Client,
    api_key: String,
}

impl OpenAiClient {
    pub fn new(api_key: String) -> Self {
        Self {
            http: Client::new(),
            api_key,
        }
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
        };

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
        if !response.status().is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".into());
            return Err(AiClientError::ApiError { status, message });
        }

        let openai_resp: OpenAiResponse = response.json().await.map_err(|e| {
            AiClientError::ParseError(format!("Failed to deserialize OpenAI response: {e}"))
        })?;

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
        };

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
        if !response.status().is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".into());
            return Err(AiClientError::ApiError { status, message });
        }

        let openai_resp: OpenAiResponse = response.json().await.map_err(|e| {
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
