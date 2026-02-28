use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::repositories::AppConfigRepository;
use crate::domain::services::ai_client::{
    AiClient, AiRoleplayRequest, AiRoleplayResponse, AiSummaryRequest,
};
use crate::domain::services::AiClientError;

const ACTIVE_LLM_PROVIDER_KEY: &str = "active_llm_provider";
const RETRY_DELAY_MS: u64 = 1000;
const MAX_RETRIES: usize = 2;

pub struct LlmRouter {
    claude: Arc<dyn AiClient>,
    openai: Arc<dyn AiClient>,
    venice: Arc<dyn AiClient>,
    together: Arc<dyn AiClient>,
    config_repo: Arc<dyn AppConfigRepository>,
    env_default: String,
}

impl LlmRouter {
    pub fn new(
        claude: Arc<dyn AiClient>,
        openai: Arc<dyn AiClient>,
        venice: Arc<dyn AiClient>,
        together: Arc<dyn AiClient>,
        config_repo: Arc<dyn AppConfigRepository>,
        env_default: String,
    ) -> Self {
        Self {
            claude,
            openai,
            venice,
            together,
            config_repo,
            env_default,
        }
    }

    /// Resolve provider name: DB → env default → hardcoded "claude"
    async fn resolve_provider(&self) -> String {
        match self.config_repo.get(ACTIVE_LLM_PROVIDER_KEY).await {
            Ok(Some(provider)) => provider,
            Ok(None) => {
                tracing::debug!(
                    "No '{}' key in app_config, using env default '{}'",
                    ACTIVE_LLM_PROVIDER_KEY,
                    self.env_default
                );
                self.env_default.clone()
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to read app_config, falling back to env default '{}'",
                    self.env_default
                );
                self.env_default.clone()
            }
        }
    }

    fn get_client(&self, provider: &str) -> &Arc<dyn AiClient> {
        match provider {
            "claude" => &self.claude,
            "openai" => &self.openai,
            "venice" => &self.venice,
            "together" => &self.together,
            unknown => {
                tracing::warn!(
                    provider = unknown,
                    "Unknown LLM provider, falling back to Claude"
                );
                &self.claude
            }
        }
    }
}

#[async_trait]
impl AiClient for LlmRouter {
    async fn generate_roleplay_response(
        &self,
        request: AiRoleplayRequest,
    ) -> Result<AiRoleplayResponse, AiClientError> {
        let provider = self.resolve_provider().await;
        tracing::debug!(provider = %provider, "Routing LLM request");
        let client = self.get_client(&provider);

        let max_attempts = 1 + MAX_RETRIES;
        let mut last_err: Option<AiClientError> = None;

        for attempt in 0..max_attempts {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
            }

            match client.generate_roleplay_response(request.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if !e.is_retryable() || attempt + 1 == max_attempts {
                        return Err(e);
                    }
                    tracing::warn!(
                        error = %e,
                        attempt = attempt + 1,
                        provider = %provider,
                        "LLM roleplay request failed (transient), will retry"
                    );
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            AiClientError::NetworkError(anyhow::anyhow!("roleplay: all retries exhausted"))
        }))
    }

    async fn generate_summary(&self, request: AiSummaryRequest) -> Result<String, AiClientError> {
        let provider = self.resolve_provider().await;
        tracing::debug!(provider = %provider, "Routing summary request");
        let client = self.get_client(&provider);

        let max_attempts = 1 + MAX_RETRIES;
        let mut last_err: Option<AiClientError> = None;

        for attempt in 0..max_attempts {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
            }

            match client.generate_summary(request.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if !e.is_retryable() || attempt + 1 == max_attempts {
                        return Err(e);
                    }
                    tracing::warn!(
                        error = %e,
                        attempt = attempt + 1,
                        provider = %provider,
                        "LLM summary request failed (transient), will retry"
                    );
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            AiClientError::NetworkError(anyhow::anyhow!("summary: all retries exhausted"))
        }))
    }
}
