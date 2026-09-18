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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AiRoleplayRequest {
    pub system_prompt: String,
    pub messages: Vec<AiMessage>,
    pub max_tokens: u32,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
    pub current_location: Option<String>,
    pub scene_time: Option<String>,
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AiSummaryRequest {
    pub existing_summary: Option<String>,
    pub messages_to_summarize: Vec<AiMessage>,
    pub max_tokens: u32,
}

pub fn validate_ai_response(response: &AiRoleplayResponse) -> Result<(), String> {
    let content = response.content_text();
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return Err("Empty response".into());
    }

    let alpha_count = trimmed
        .chars()
        .filter(|c| c.is_ascii_alphabetic() || ('\u{0E01}'..='\u{0E4F}').contains(c))
        .count();

    if alpha_count < 10 {
        return Err(format!("Too few alphabetic characters: {alpha_count}"));
    }

    let total = trimmed.chars().count();
    let ratio = alpha_count as f64 / total as f64;
    if ratio < 0.3 {
        return Err(format!("Low alphabetic ratio: {:.1}%", ratio * 100.0));
    }

    Ok(())
}
