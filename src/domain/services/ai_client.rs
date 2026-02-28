use async_trait::async_trait;

use super::ai_client_error::AiClientError;

#[async_trait]
pub trait AiClient: Send + Sync {
    /// Generate roleplay response (narrator + character dialogue)
    async fn generate_roleplay_response(
        &self,
        request: AiRoleplayRequest,
    ) -> Result<AiRoleplayResponse, AiClientError>;

    /// Generate a summary of conversation messages
    async fn generate_summary(&self, request: AiSummaryRequest) -> Result<String, AiClientError>;
}

#[derive(Clone)]
pub struct AiRoleplayRequest {
    pub system_prompt: String,
    pub messages: Vec<AiMessage>,
    pub max_tokens: u32,
}

#[derive(Clone)]
pub struct AiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockType {
    Narration,
    Dialogue,
}

#[derive(Debug, Clone)]
pub struct ResponseBlock {
    pub block_type: BlockType,
    pub text: String,
}

pub struct AiRoleplayResponse {
    pub blocks: Vec<ResponseBlock>,
    pub mood: Option<String>,
}

impl AiRoleplayResponse {
    /// Serialize blocks to text markup for storage.
    pub fn content_text(&self) -> String {
        self.blocks
            .iter()
            .map(|b| match b.block_type {
                BlockType::Narration => format!("*{}*", b.text),
                BlockType::Dialogue => b.text.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone)]
pub struct AiSummaryRequest {
    pub existing_summary: Option<String>,
    pub messages_to_summarize: Vec<AiMessage>,
    pub max_tokens: u32,
}
