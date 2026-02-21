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

    // Try parsing the full text as JSON first
    let output: LlmRoleplayOutput = match serde_json::from_str(json_str) {
        Ok(parsed) => parsed,
        Err(initial_err) => {
            // Fallback: extract the last top-level JSON object from mixed content
            if let Some(extracted) = extract_last_json_object(json_str) {
                serde_json::from_str(extracted).map_err(|e| {
                    AiClientError::ParseError(format!(
                        "Failed to parse extracted JSON: {e}\nExtracted: {extracted}\nRaw: {raw}"
                    ))
                })?
            } else {
                return Err(AiClientError::ParseError(format!(
                    "Failed to parse LLM JSON: {initial_err}\nRaw: {raw}"
                )));
            }
        }
    };

    if output.narrator_text.trim().is_empty() || output.character_text.trim().is_empty() {
        return Err(AiClientError::ParseError(format!(
            "LLM returned empty narrator_text or character_text (both are required)\nRaw: {raw}"
        )));
    }

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

/// Find the last top-level `{...}` block in a string by scanning for balanced braces.
fn extract_last_json_object(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut last_start = None;

    // Scan backwards for the last '{' that starts a balanced block
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        if bytes[i] == b'}' {
            // Found a closing brace — walk backwards to find its matching '{'
            if let Some(start) = find_matching_open_brace(bytes, i) {
                // Only consider blocks that contain "narrator_text" (our expected field)
                let candidate = &text[start..=i];
                if candidate.contains("narrator_text") {
                    last_start = Some(start);
                    break;
                }
            }
        }
    }

    last_start.map(|start| {
        // Find the matching '}' going forward from start for a clean slice
        let end = find_matching_close_brace(bytes, start).unwrap_or(bytes.len() - 1);
        &text[start..=end]
    })
}

/// Given the index of a '}', walk backwards to find the matching '{', respecting nesting.
fn find_matching_open_brace(bytes: &[u8], close_pos: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;

    // Reverse scan counting braces to find the matching '{'.
    let mut i = close_pos;
    loop {
        match bytes[i] {
            b'}' if !in_string => depth += 1,
            b'{' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            b'"' => {
                // Rough check: toggle in_string unless preceded by backslash
                if i == 0 || bytes[i - 1] != b'\\' {
                    in_string = !in_string;
                }
            }
            _ => {}
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    None
}

/// Given the index of a '{', walk forward to find the matching '}', respecting nesting.
fn find_matching_close_brace(bytes: &[u8], open_pos: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;

    for i in open_pos..bytes.len() {
        match bytes[i] {
            b'{' if !in_string => depth += 1,
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            b'"' => {
                if i == 0 || bytes[i - 1] != b'\\' {
                    in_string = !in_string;
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clean_json() {
        let raw = r#"{"narrator_text":"📍 ร้าน","character_text":"สวัสดี","mood":"happy","scene_update":null}"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.narrator_text, "📍 ร้าน");
        assert_eq!(result.character_text, "สวัสดี");
        assert_eq!(result.mood, Some("happy".to_string()));
        assert!(result.scene_update.is_none());
    }

    #[test]
    fn parse_markdown_fenced_json() {
        let raw = "```json\n{\"narrator_text\":\"N\",\"character_text\":\"C\",\"mood\":\"neutral\",\"scene_update\":null}\n```";
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.narrator_text, "N");
        assert_eq!(result.character_text, "C");
    }

    #[test]
    fn parse_json_with_text_before() {
        let raw = r#"📍 ร้านราเมนเล็กๆ ย่านสีลม • 🕒 เย็น
Some narrative text here...

{"narrator_text":"📍 ร้าน","character_text":"สวัสดีค่า~","mood":"playful","scene_update":null}"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.narrator_text, "📍 ร้าน");
        assert_eq!(result.character_text, "สวัสดีค่า~");
        assert_eq!(result.mood, Some("playful".to_string()));
    }

    #[test]
    fn parse_json_with_text_before_and_after() {
        let raw = r#"Here is the response:

{"narrator_text":"N","character_text":"C","mood":"happy","scene_update":null}

Hope that helps!"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.narrator_text, "N");
        assert_eq!(result.character_text, "C");
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let raw = "this is not json at all";
        let result = parse_llm_response(raw);
        assert!(result.is_err());
    }

    #[test]
    fn parse_both_empty_text_returns_error() {
        let raw =
            r#"{"narrator_text":"","character_text":"","mood":"neutral","scene_update":null}"#;
        let result = parse_llm_response(raw);
        assert!(result.is_err());
    }

    #[test]
    fn parse_one_empty_text_returns_error() {
        let raw =
            r#"{"narrator_text":"","character_text":"สวัสดี","mood":"neutral","scene_update":null}"#;
        let result = parse_llm_response(raw);
        assert!(result.is_err());
    }

    #[test]
    fn parse_json_with_scene_update() {
        let raw = r#"{"narrator_text":"N","character_text":"C","mood":"neutral","scene_update":{"location":"park","time":"evening","summary":"walked out"}}"#;
        let result = parse_llm_response(raw).unwrap();
        let su = result.scene_update.unwrap();
        assert_eq!(su.location, Some("park".to_string()));
        assert_eq!(su.time, Some("evening".to_string()));
        assert_eq!(su.summary, Some("walked out".to_string()));
    }
}
