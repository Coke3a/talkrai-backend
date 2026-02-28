use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::domain::services::ai_client::{
    AiClient, AiRoleplayRequest, AiRoleplayResponse, AiSummaryRequest,
};
use crate::domain::services::AiClientError;

use super::response::{build_summary_system_prompt, parse_llm_response};

const TOGETHER_API_URL: &str = "https://api.together.xyz/v1/chat/completions";
const TOGETHER_MODEL: &str = "Qwen/Qwen3-235B-A22B-Instruct-2507-tput";

// --- Request types ---

#[derive(Serialize)]
struct TogetherRequest {
    model: &'static str,
    max_tokens: u32,
    messages: Vec<TogetherMessage>,
}

#[derive(Serialize)]
struct TogetherMessage {
    role: String,
    content: String,
}

// --- Response types ---

#[derive(Deserialize)]
struct TogetherResponse {
    choices: Vec<TogetherChoice>,
}

#[derive(Deserialize)]
struct TogetherChoice {
    message: TogetherChoiceMessage,
}

#[derive(Deserialize)]
struct TogetherChoiceMessage {
    content: Option<String>,
}

pub struct TogetherClient {
    http: Client,
    api_key: String,
}

impl TogetherClient {
    pub fn new(api_key: String) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build Together HTTP client");
        Self { http, api_key }
    }
}

#[async_trait]
impl AiClient for TogetherClient {
    async fn generate_roleplay_response(
        &self,
        request: AiRoleplayRequest,
    ) -> Result<AiRoleplayResponse, AiClientError> {
        let mut messages = Vec::with_capacity(request.messages.len() + 1);

        messages.push(TogetherMessage {
            role: "system".into(),
            content: request.system_prompt,
        });

        for m in request.messages {
            messages.push(TogetherMessage {
                role: m.role,
                content: m.content,
            });
        }

        let body = TogetherRequest {
            model: TOGETHER_MODEL,
            max_tokens: request.max_tokens,
            messages,
        };

        tracing::debug!(
            provider = "together",
            model = TOGETHER_MODEL,
            max_tokens = body.max_tokens,
            message_count = body.messages.len(),
            body = %serde_json::to_string(&body).unwrap_or_default(),
            "Sending roleplay request to LLM"
        );

        let response = self
            .http
            .post(TOGETHER_API_URL)
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
            provider = "together",
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

        let together_resp: TogetherResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                AiClientError::ParseError(format!("Failed to deserialize Together response: {e}"))
            })?;

        let text = together_resp
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .ok_or_else(|| AiClientError::ParseError("No content in Together response".into()))?;

        parse_llm_response(text)
    }

    async fn generate_summary(&self, request: AiSummaryRequest) -> Result<String, AiClientError> {
        let system_prompt = build_summary_system_prompt(&request.existing_summary);

        let mut messages = Vec::with_capacity(request.messages_to_summarize.len() + 1);
        messages.push(TogetherMessage {
            role: "system".into(),
            content: system_prompt,
        });
        for m in request.messages_to_summarize {
            messages.push(TogetherMessage {
                role: m.role,
                content: m.content,
            });
        }

        let body = TogetherRequest {
            model: TOGETHER_MODEL,
            max_tokens: request.max_tokens,
            messages,
        };

        tracing::debug!(
            provider = "together",
            model = TOGETHER_MODEL,
            max_tokens = body.max_tokens,
            message_count = body.messages.len(),
            body = %serde_json::to_string(&body).unwrap_or_default(),
            "Sending summary request to LLM"
        );

        let response = self
            .http
            .post(TOGETHER_API_URL)
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
            provider = "together",
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

        let together_resp: TogetherResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                AiClientError::ParseError(format!(
                    "Failed to deserialize Together summary response: {e}"
                ))
            })?;

        let text = together_resp
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .ok_or_else(|| {
                AiClientError::ParseError("No content in Together summary response".into())
            })?;

        Ok(text.trim().to_string())
    }
}
