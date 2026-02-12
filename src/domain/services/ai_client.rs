use async_trait::async_trait;

use super::ai_client_error::AiClientError;

#[async_trait]
pub trait AiClient: Send + Sync {
    /// Generate roleplay response (narrator + character dialogue)
    async fn generate_roleplay_response(
        &self,
        request: AiRoleplayRequest,
    ) -> Result<AiRoleplayResponse, AiClientError>;
}

pub struct AiRoleplayRequest {
    pub system_prompt: String,
    pub messages: Vec<AiMessage>,
    pub max_tokens: u32,
}

pub struct AiMessage {
    pub role: String,
    pub content: String,
}

pub struct AiRoleplayResponse {
    pub narrator_text: String,
    pub character_text: String,
    pub mood: Option<String>,
    pub memories: Vec<String>,
    pub scene_update: Option<AiSceneUpdate>,
}

pub struct AiSceneUpdate {
    pub location: Option<String>,
    pub time: Option<String>,
    pub summary: Option<String>,
}
