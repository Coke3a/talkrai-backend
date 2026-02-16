use serde::Deserialize;

use crate::domain::services::ai_client::{AiRoleplayResponse, AiSceneUpdate};
use crate::domain::services::AiClientError;

#[derive(Debug, Deserialize)]
pub struct LlmRoleplayOutput {
    pub narrator_text: String,
    pub character_text: String,
    pub mood: Option<String>,
    pub scene_update: Option<LlmSceneUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct LlmSceneUpdate {
    pub location: Option<String>,
    pub time: Option<String>,
    pub summary: Option<String>,
}

/// Build a system prompt for conversation summarization.
pub fn build_summary_system_prompt(existing_summary: &Option<String>) -> String {
    let mut prompt = String::from(
        "You are a concise summarizer for a Thai roleplay conversation. \
         Summarize the key events, character interactions, and emotional developments \
         in 2-4 sentences in Thai. Focus on what matters for story continuity. \
         Return only the summary text, no JSON or markdown.",
    );

    if let Some(existing) = existing_summary {
        prompt.push_str(&format!(
            "\n\nPrevious summary to incorporate: {}",
            existing
        ));
    }

    prompt
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
        scene_update: output.scene_update.map(|su| AiSceneUpdate {
            location: su.location,
            time: su.time,
            summary: su.summary,
        }),
    })
}
