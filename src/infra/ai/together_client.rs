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

const TOGETHER_API_URL: &str = "https://api.together.ai/v1/chat/completions";
const TOGETHER_MODEL: &str = "Qwen/Qwen3.5-9B";

// --- Request types ---

#[derive(Serialize)]
struct TogetherRequest {
    model: &'static str,
    max_tokens: u32,
    messages: Vec<TogetherMessage>,
    stream: bool,
    reasoning: TogetherReasoning,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<Value>,
}

impl TogetherRequest {
    fn new(
        max_tokens: u32,
        messages: Vec<TogetherMessage>,
        tools: Option<Vec<Value>>,
        tool_choice: Option<Value>,
    ) -> Self {
        Self {
            model: TOGETHER_MODEL,
            max_tokens,
            messages,
            stream: true,
            reasoning: TogetherReasoning { enabled: false },
            tools,
            tool_choice,
        }
    }
}

#[derive(Serialize)]
struct TogetherReasoning {
    enabled: bool,
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
    #[serde(default)]
    usage: Option<TogetherUsage>,
}

/// Token usage from Together. `prompt_tokens_details.cached_tokens` is the
/// definitive signal of whether prompt caching is active for this model — it is
/// non-zero only on cache-eligible models, so it answers "does caching work for
/// Qwen?" from production rather than from docs guesswork.
#[derive(Deserialize, Default)]
struct TogetherUsage {
    #[serde(default)]
    prompt_tokens: Option<u32>,
    #[serde(default)]
    completion_tokens: Option<u32>,
    #[serde(default)]
    total_tokens: Option<u32>,
    #[serde(default)]
    prompt_tokens_details: Option<TogetherPromptTokensDetails>,
}

#[derive(Deserialize, Default)]
struct TogetherPromptTokensDetails {
    #[serde(default)]
    cached_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct TogetherChoice {
    message: TogetherChoiceMessage,
}

#[derive(Deserialize)]
struct TogetherChoiceMessage {
    content: Option<String>,
    tool_calls: Option<Vec<TogetherToolCall>>,
}

#[derive(Deserialize)]
struct TogetherToolCall {
    function: TogetherToolCallFunction,
}

#[derive(Deserialize)]
struct TogetherToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct TogetherStreamChunk {
    #[serde(default)]
    choices: Vec<TogetherStreamChoice>,
    #[serde(default)]
    usage: Option<TogetherUsage>,
}

#[derive(Deserialize)]
struct TogetherStreamChoice {
    delta: TogetherStreamDelta,
}

#[derive(Deserialize, Default)]
struct TogetherStreamDelta {
    content: Option<String>,
    tool_calls: Option<Vec<TogetherStreamToolCall>>,
}

#[derive(Deserialize)]
struct TogetherStreamToolCall {
    #[serde(default)]
    index: usize,
    function: Option<TogetherStreamToolCallFunction>,
}

#[derive(Deserialize)]
struct TogetherStreamToolCallFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Default)]
struct AggregatedToolCall {
    name: String,
    arguments: String,
}

fn parse_together_stream(raw: &str) -> Result<TogetherResponse, AiClientError> {
    let mut content = String::new();
    let mut tool_calls: Vec<AggregatedToolCall> = Vec::new();
    let mut usage = None;
    let mut saw_chunk = false;

    for line in raw.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }

        let chunk: TogetherStreamChunk = serde_json::from_str(data).map_err(|e| {
            AiClientError::ParseError(format!("Failed to deserialize Together stream chunk: {e}"))
        })?;
        saw_chunk = true;

        if chunk.usage.is_some() {
            usage = chunk.usage;
        }

        for choice in chunk.choices {
            if let Some(delta_content) = choice.delta.content {
                content.push_str(&delta_content);
            }
            for tool_call in choice.delta.tool_calls.unwrap_or_default() {
                if tool_calls.len() <= tool_call.index {
                    tool_calls.resize_with(tool_call.index + 1, AggregatedToolCall::default);
                }
                if let Some(function) = tool_call.function {
                    let aggregate = &mut tool_calls[tool_call.index];
                    if let Some(name) = function.name {
                        aggregate.name.push_str(&name);
                    }
                    if let Some(arguments) = function.arguments {
                        aggregate.arguments.push_str(&arguments);
                    }
                }
            }
        }
    }

    if !saw_chunk {
        return Err(AiClientError::ParseError(
            "Together stream contained no data chunks".into(),
        ));
    }

    let tool_calls = tool_calls
        .into_iter()
        .filter(|call| !call.name.is_empty() || !call.arguments.is_empty())
        .map(|call| TogetherToolCall {
            function: TogetherToolCallFunction {
                name: call.name,
                arguments: call.arguments,
            },
        })
        .collect::<Vec<_>>();

    Ok(TogetherResponse {
        choices: vec![TogetherChoice {
            message: TogetherChoiceMessage {
                content: (!content.is_empty()).then_some(content),
                tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
            },
        }],
        usage,
    })
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

        let body = TogetherRequest::new(
            request.max_tokens,
            messages,
            Some(vec![openai_tool_definition()]),
            Some(serde_json::json!("required")),
        );

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

        let together_resp = parse_together_stream(&response_text)?;

        // METRIC: token usage. `cached_tokens` > 0 confirms prompt caching is active
        // for this model; consistently 0/absent means no cache benefit on Qwen, so
        // the real lever becomes token reduction (history/static prompt), not caching.
        if let Some(usage) = &together_resp.usage {
            let cached_tokens = usage
                .prompt_tokens_details
                .as_ref()
                .and_then(|d| d.cached_tokens)
                .unwrap_or(0);
            tracing::info!(
                provider = "together",
                model = TOGETHER_MODEL,
                prompt_tokens = usage.prompt_tokens.unwrap_or(0),
                cached_tokens,
                completion_tokens = usage.completion_tokens.unwrap_or(0),
                total_tokens = usage.total_tokens.unwrap_or(0),
                "METRIC: Together token usage"
            );
        }

        // Try tool call first
        if let Some(choice) = together_resp.choices.first() {
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
            provider = "together",
            "No tool call found, falling back to text parsing"
        );
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

        let body = TogetherRequest::new(request.max_tokens, messages, None, None);

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

        let together_resp = parse_together_stream(&response_text)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stream_response_assembles_tool_call_arguments_and_usage() {
        let stream = concat!(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"update_scene_state\",\"arguments\":\"{\\\"content\\\":\\\"*เธอยิ้ม* สวัสดี\\\",\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"current_location\\\":\\\"ร้านกาแฟ\\\",\\\"scene_time\\\":\\\"เย็น\\\",\\\"mood\\\":\\\"happy\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":25,\"total_tokens\":125}}\n\n",
            "data: [DONE]\n\n",
        );

        let response = parse_together_stream(stream).unwrap();
        let tool_call = &response.choices[0].message.tool_calls.as_ref().unwrap()[0];

        assert_eq!(tool_call.function.name, "update_scene_state");
        assert_eq!(
            tool_call.function.arguments,
            r#"{"content":"*เธอยิ้ม* สวัสดี","current_location":"ร้านกาแฟ","scene_time":"เย็น","mood":"happy"}"#
        );
        assert_eq!(response.usage.unwrap().total_tokens, Some(125));
    }

    #[test]
    fn request_contract_uses_qwen35_9b_streaming_without_reasoning() {
        let request = TogetherRequest::new(600, vec![], None, None);
        let json = serde_json::to_value(request).unwrap();

        assert_eq!(
            TOGETHER_API_URL,
            "https://api.together.ai/v1/chat/completions"
        );
        assert_eq!(TOGETHER_MODEL, "Qwen/Qwen3.5-9B");
        assert_eq!(json["stream"], true);
        assert_eq!(json["reasoning"]["enabled"], false);
    }

    #[test]
    fn parse_stream_response_assembles_summary_content() {
        let stream = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"สรุปเหตุการณ์\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"ล่าสุดของเรื่อง\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );

        let response = parse_together_stream(stream).unwrap();

        assert_eq!(
            response.choices[0].message.content.as_deref(),
            Some("สรุปเหตุการณ์ล่าสุดของเรื่อง")
        );
    }

    #[test]
    fn parse_stream_response_rejects_malformed_chunk() {
        let result = parse_together_stream("data: not-json\n\ndata: [DONE]\n");

        assert!(matches!(result, Err(AiClientError::ParseError(_))));
    }
}
