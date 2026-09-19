use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use super::response::{
    build_summary_system_prompt, openai_tool_definition, tool_args_to_response,
    UpdateSceneStateArgs,
};
use crate::domain::services::ai_client::{
    AiClient, AiMessage, AiRoleplayRequest, AiRoleplayResponse, AiSummaryRequest,
};
use crate::domain::services::AiClientError;

const API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

pub struct OpenRouterClient {
    http: Client,
    endpoint: String,
    api_key: String,
    model: String,
}

impl OpenRouterClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            http: Client::new(),
            endpoint: API_URL.into(),
            api_key,
            model,
        }
    }

    fn body(
        &self,
        system: String,
        messages: Vec<AiMessage>,
        max_tokens: u32,
        roleplay: bool,
    ) -> Value {
        let mut all = vec![json!({"role":"system", "content":system})];
        all.extend(
            messages
                .into_iter()
                .map(|m| json!({"role":m.role,"content":m.content})),
        );
        let mut body = json!({
            "model": self.model,
            "messages": all,
            "max_tokens": max_tokens,
            "stream": false,
            "reasoning": {"enabled": false}
        });
        if roleplay {
            body["tools"] = json!([openai_tool_definition()]);
            body["tool_choice"] =
                json!({"type":"function","function":{"name":"update_scene_state"}});
            // Do not silently route state updates to endpoints that ignore tools.
            body["provider"] = json!({"require_parameters":true});
        }
        body
    }

    async fn complete(&self, body: Value) -> Result<Value, AiClientError> {
        if self.api_key.trim().is_empty() || self.model.trim().is_empty() {
            return Err(AiClientError::ApiError {
                status: 400,
                message: "OpenRouter API key and model must be configured".into(),
            });
        }
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .timeout(std::time::Duration::from_secs(120))
            .json(&body)
            .send()
            .await
            .map_err(|e| AiClientError::NetworkError(e.without_url().into()))?;
        let status = response.status().as_u16();
        if status == 429 {
            return Err(AiClientError::RateLimited);
        }
        if status >= 400 {
            // Upstream error bodies can contain prompts or credentials; retain only status.
            return Err(AiClientError::ApiError {
                status,
                message: "OpenRouter request rejected".into(),
            });
        }
        let value: Value = response
            .json()
            .await
            .map_err(|_| AiClientError::ParseError("Invalid OpenRouter JSON response".into()))?;
        if value.get("error").is_some() {
            let code = value["error"]["code"]
                .as_u64()
                .and_then(|c| u16::try_from(c).ok())
                .unwrap_or(502);
            if code == 429 {
                return Err(AiClientError::RateLimited);
            }
            return Err(AiClientError::ApiError {
                status: code,
                message: "OpenRouter generation failed".into(),
            });
        }
        Ok(value)
    }
}

fn message(value: &Value) -> Result<&Value, AiClientError> {
    let choice = value["choices"]
        .as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| AiClientError::ParseError("Missing OpenRouter choice".into()))?;
    if !matches!(
        choice["finish_reason"].as_str(),
        Some("stop" | "tool_calls")
    ) {
        return Err(AiClientError::ParseError(
            "Incomplete OpenRouter response".into(),
        ));
    }
    Ok(&choice["message"])
}

fn roleplay_response(value: &Value) -> Result<AiRoleplayResponse, AiClientError> {
    let message = message(value)?;
    let calls = message["tool_calls"]
        .as_array()
        .ok_or_else(|| AiClientError::ParseError("Missing scene state tool call".into()))?;
    if calls.len() != 1 || calls[0]["function"]["name"] != "update_scene_state" {
        return Err(AiClientError::ParseError(
            "Unexpected scene state tool calls".into(),
        ));
    }
    let args = calls[0]["function"]["arguments"]
        .as_str()
        .ok_or_else(|| AiClientError::ParseError("Missing tool arguments".into()))?;
    let args: UpdateSceneStateArgs = serde_json::from_str(args)
        .map_err(|_| AiClientError::ParseError("Invalid scene state arguments".into()))?;
    let schema = openai_tool_definition();
    let properties = &schema["function"]["parameters"]["properties"];
    if args.content.trim().is_empty()
        || args.current_location.trim().is_empty()
        || !properties["mood"]["enum"]
            .as_array()
            .is_some_and(|values| values.contains(&json!(args.mood)))
        || !properties["scene_time"]["enum"]
            .as_array()
            .is_some_and(|values| values.contains(&json!(args.scene_time)))
    {
        return Err(AiClientError::ParseError(
            "Invalid scene state values".into(),
        ));
    }
    Ok(tool_args_to_response(args))
}

#[async_trait]
impl AiClient for OpenRouterClient {
    async fn generate_roleplay_response(
        &self,
        request: AiRoleplayRequest,
    ) -> Result<AiRoleplayResponse, AiClientError> {
        let body = self.body(
            request.system_prompt,
            request.messages,
            request.max_tokens,
            true,
        );
        roleplay_response(&self.complete(body).await?)
    }

    async fn generate_summary(&self, request: AiSummaryRequest) -> Result<String, AiClientError> {
        let body = self.body(
            build_summary_system_prompt(&request.existing_summary),
            request.messages_to_summarize,
            request.max_tokens,
            false,
        );
        let value = self.complete(body).await?;
        message(&value)?["content"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(|| AiClientError::ParseError("Missing OpenRouter summary".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roleplay_requires_supported_tools_but_summary_is_plain_text() {
        let client = OpenRouterClient::new("test".into(), "qwen/qwen3.8-27b".into());
        let body = client.body("persona".into(), vec![], 600, true);
        assert_eq!(body["model"], "qwen/qwen3.8-27b");
        assert_eq!(body["messages"][0]["content"], "persona");
        assert_eq!(body["reasoning"]["enabled"], false);
        assert_eq!(body["provider"]["require_parameters"], true);
        assert_eq!(
            body["tool_choice"]["function"]["name"],
            "update_scene_state"
        );
        let summary = client.body("summary".into(), vec![], 400, false);
        assert_eq!(summary["reasoning"]["enabled"], false);
        assert!(summary.get("tools").is_none());
        assert!(summary.get("tool_choice").is_none());
    }

    #[test]
    fn complete_tool_response_parses_but_truncation_and_missing_fields_fail() {
        let args = json!({"content":"*ยิ้ม* สวัสดีมิน", "current_location":"ปราสาท", "scene_time":"ดึก", "mood":"happy"});
        let mut response = json!({"choices":[{"finish_reason":"tool_calls","message":{"tool_calls":[{"function":{"name":"update_scene_state","arguments":args.to_string()}}]}}]});
        assert!(roleplay_response(&response).is_ok());
        response["choices"][0]["finish_reason"] = json!("length");
        assert!(roleplay_response(&response).is_err());
        response["choices"][0]["finish_reason"] = json!("tool_calls");
        response["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"] = json!("{}");
        assert!(roleplay_response(&response).is_err());
        assert!(roleplay_response(&json!({"choices":[]})).is_err());
    }

    #[tokio::test]
    async fn unconfigured_provider_fails_without_network_request() {
        let client = OpenRouterClient::new("".into(), "qwen/qwen3.8-27b".into());
        assert!(matches!(
            client.complete(json!({})).await,
            Err(AiClientError::ApiError { status: 400, .. })
        ));
    }
    #[tokio::test]
    async fn http_boundary_authenticates_and_maps_errors_without_leaking_body() {
        use axum::{
            http::{HeaderMap, StatusCode},
            routing::post,
            Json, Router,
        };
        for (status, raw, expected) in [
            (
                200,
                r#"{"choices":[{"finish_reason":"stop","message":{"content":"สรุป"}}]}"#,
                0,
            ),
            (429, "private body", 429),
            (401, "private body", 401),
            (503, "private body", 503),
            (
                200,
                r#"{"error":{"code":429,"message":"private body"}}"#,
                429,
            ),
            (200, "invalid json private body", 1),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let app = Router::new().route(
                "/chat",
                post(
                    move |headers: HeaderMap, Json(body): Json<Value>| async move {
                        assert_eq!(headers["authorization"], "Bearer test-key");
                        assert_eq!(body["model"], "test-model");
                        (StatusCode::from_u16(status).unwrap(), raw)
                    },
                ),
            );
            let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let mut client = OpenRouterClient::new("test-key".into(), "test-model".into());
            client.endpoint = format!("http://{address}/chat");
            let result = client
                .complete(client.body("test".into(), vec![], 10, false))
                .await;
            server.abort();
            match expected {
                0 => assert_eq!(message(&result.unwrap()).unwrap()["content"], "สรุป"),
                429 => assert!(matches!(result, Err(AiClientError::RateLimited))),
                1 => assert!(matches!(result, Err(AiClientError::ParseError(_)))),
                code => match result {
                    Err(AiClientError::ApiError { status, message }) => {
                        assert_eq!(status, code);
                        assert!(!message.contains("private body"));
                    }
                    _ => panic!("Expected API error"),
                },
            }
        }
    }
}
