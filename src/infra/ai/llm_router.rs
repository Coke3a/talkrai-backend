use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::repositories::AppConfigRepository;
use crate::domain::services::ai_client::{
    AiClient, AiRoleplayRequest, AiRoleplayResponse, AiSummaryRequest,
};
use crate::domain::services::AiClientError;

const ACTIVE_LLM_PROVIDER_KEY: &str = "active_llm_provider";

pub struct LlmRouter {
    claude: Arc<dyn AiClient>,
    openai: Arc<dyn AiClient>,
    venice: Arc<dyn AiClient>,
    config_repo: Arc<dyn AppConfigRepository>,
    env_default: String,
}

impl LlmRouter {
    pub fn new(
        claude: Arc<dyn AiClient>,
        openai: Arc<dyn AiClient>,
        venice: Arc<dyn AiClient>,
        config_repo: Arc<dyn AppConfigRepository>,
        env_default: String,
    ) -> Self {
        Self {
            claude,
            openai,
            venice,
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
        tracing::info!(provider = %provider, "Routing LLM request");
        let client = self.get_client(&provider);
        client.generate_roleplay_response(request).await
    }

    async fn generate_summary(&self, request: AiSummaryRequest) -> Result<String, AiClientError> {
        let provider = self.resolve_provider().await;
        tracing::info!(provider = %provider, "Routing summary request");
        let client = self.get_client(&provider);
        client.generate_summary(request).await
    }
}
