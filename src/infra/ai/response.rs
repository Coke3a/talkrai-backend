use serde::Deserialize;

use crate::domain::services::ai_client::{AiRoleplayResponse, AiSceneUpdate};
use crate::domain::services::AiClientError;

#[derive(Debug, Deserialize)]
pub struct LlmRoleplayOutput {
    pub narrator_text: String,
    pub character_text: String,
    pub mood: Option<String>,
    #[serde(default)]
    pub memories: Vec<String>,
    pub scene_update: Option<LlmSceneUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct LlmSceneUpdate {
    pub location: Option<String>,
    pub time: Option<String>,
    pub summary: Option<String>,
}

/// Strip markdown code fences (```json ... ```) and parse JSON into `AiRoleplayResponse`.
pub fn parse_llm_response(raw: &str) -> Result<AiRoleplayResponse, AiClientError> {
    let trimmed = raw.trim();

    // Strip markdown fences if present
    let json_str = if trimmed.starts_with("```") {
        let without_opening = trimmed
            .strip_prefix("```json")
            .or_else(|| trimmed.strip_prefix("```"))
            .unwrap_or(trimmed);
        without_opening
            .strip_suffix("```")
            .unwrap_or(without_opening)
            .trim()
    } else {
        trimmed
    };

    let output: LlmRoleplayOutput = serde_json::from_str(json_str).map_err(|e| {
        AiClientError::ParseError(format!("Failed to parse LLM JSON: {e}\nRaw: {raw}"))
    })?;

    Ok(AiRoleplayResponse {
        narrator_text: output.narrator_text,
        character_text: output.character_text,
        mood: output.mood,
        memories: output.memories,
        scene_update: output.scene_update.map(|su| AiSceneUpdate {
            location: su.location,
            time: su.time,
            summary: su.summary,
        }),
    })
}
