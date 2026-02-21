use serde::Deserialize;

use crate::domain::services::ai_client::{
    AiRoleplayResponse, AiSceneUpdate, BlockType, ResponseBlock,
};
use crate::domain::services::AiClientError;

/// New blocks-format output from LLM.
#[derive(Debug, Deserialize)]
struct LlmBlocksOutput {
    blocks: Vec<LlmBlock>,
    mood: Option<String>,
    scene_update: Option<LlmSceneUpdate>,
}

#[derive(Debug, Deserialize)]
struct LlmBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
}

/// Legacy 2-field output (backward compatibility).
#[derive(Debug, Deserialize)]
struct LlmRoleplayOutput {
    narrator_text: String,
    character_text: String,
    mood: Option<String>,
    scene_update: Option<LlmSceneUpdate>,
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
///
/// Tries blocks format first, then falls back to legacy narrator_text/character_text format.
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

    // Try parsing full text — blocks first, then legacy
    if let Some(resp) = try_parse(json_str) {
        return Ok(resp);
    }

    // Try repairing JSON then parsing
    let repaired = repair_json(json_str);
    if let Some(resp) = try_parse(&repaired) {
        return Ok(resp);
    }

    // Fallback: extract last JSON object from mixed content
    if let Some(extracted) = extract_last_json_object(json_str) {
        if let Some(resp) = try_parse(extracted) {
            return Ok(resp);
        }
        let repaired_extracted = repair_json(extracted);
        if let Some(resp) = try_parse(&repaired_extracted) {
            return Ok(resp);
        }
    }

    Err(AiClientError::ParseError(format!(
        "Failed to parse LLM JSON (tried blocks and legacy formats)\nRaw: {raw}"
    )))
}

/// Try parsing as blocks format. Returns None if parsing or validation fails.
fn try_parse_blocks(json_str: &str) -> Option<AiRoleplayResponse> {
    let output: LlmBlocksOutput = serde_json::from_str(json_str).ok()?;

    // Validate: min 2 blocks, min 1 narration
    if output.blocks.len() < 2 {
        return None;
    }
    let has_narration = output
        .blocks
        .iter()
        .any(|b| b.block_type == "narration" && !b.text.trim().is_empty());
    if !has_narration {
        return None;
    }

    let blocks: Vec<ResponseBlock> = output
        .blocks
        .into_iter()
        .filter(|b| !b.text.trim().is_empty())
        .map(|b| ResponseBlock {
            block_type: if b.block_type == "dialogue" {
                BlockType::Dialogue
            } else {
                BlockType::Narration
            },
            text: b.text,
        })
        .collect();

    if blocks.len() < 2 {
        return None;
    }

    Some(AiRoleplayResponse {
        blocks,
        mood: output.mood,
        scene_update: output.scene_update.map(convert_scene_update),
    })
}

/// Try parsing as legacy narrator_text/character_text format.
fn try_parse_legacy(json_str: &str) -> Option<AiRoleplayResponse> {
    let output: LlmRoleplayOutput = serde_json::from_str(json_str).ok()?;

    if output.narrator_text.trim().is_empty() || output.character_text.trim().is_empty() {
        return None;
    }

    let blocks = vec![
        ResponseBlock {
            block_type: BlockType::Narration,
            text: output.narrator_text,
        },
        ResponseBlock {
            block_type: BlockType::Dialogue,
            text: output.character_text,
        },
    ];

    Some(AiRoleplayResponse {
        blocks,
        mood: output.mood,
        scene_update: output.scene_update.map(convert_scene_update),
    })
}

fn convert_scene_update(su: LlmSceneUpdate) -> AiSceneUpdate {
    AiSceneUpdate {
        location: su.location,
        time: su.time,
        summary: su.summary,
    }
}

/// Repair common LLM JSON errors **outside** of string values.
///
/// Fixes:
/// - Stray `(` `)` outside strings → removed
/// - Trailing commas `,]` `,}` → comma removed
///
/// Characters inside JSON string values are never touched.
fn repair_json(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut in_string = false;
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        if in_string {
            out.push(b);
            if b == b'"' {
                // Count preceding backslashes to determine if this quote is escaped
                let mut backslashes = 0;
                while backslashes < out.len() - 1 && out[out.len() - 2 - backslashes] == b'\\' {
                    backslashes += 1;
                }
                if backslashes % 2 == 0 {
                    in_string = false;
                }
            }
            i += 1;
            continue;
        }

        // Outside string
        match b {
            b'"' => {
                in_string = true;
                out.push(b);
            }
            // Remove stray parentheses outside strings
            b'(' | b')' => { /* skip */ }
            // Remove trailing commas before ] or }
            b',' => {
                // Peek ahead past whitespace to see if next non-ws char is ] or }
                let mut j = i + 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && (bytes[j] == b']' || bytes[j] == b'}') {
                    // Skip this comma (trailing comma)
                } else {
                    out.push(b);
                }
            }
            _ => out.push(b),
        }
        i += 1;
    }

    String::from_utf8(out).expect("repair_json: removed only ASCII bytes from valid UTF-8")
}

/// Try parsing a JSON string as either blocks or legacy format.
fn try_parse(json_str: &str) -> Option<AiRoleplayResponse> {
    try_parse_blocks(json_str).or_else(|| try_parse_legacy(json_str))
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
                let candidate = &text[start..=i];
                if candidate.contains("blocks") || candidate.contains("narrator_text") {
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
    use crate::domain::services::ai_client::BlockType;

    // --- Blocks format tests ---

    #[test]
    fn parse_blocks_format() {
        let raw = r#"{"blocks":[{"type":"narration","text":"ลมพัดเบาๆ"},{"type":"dialogue","text":"\"สวัสดีค่า~\""}],"mood":"happy","scene_update":null}"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 2);
        assert_eq!(result.blocks[0].block_type, BlockType::Narration);
        assert_eq!(result.blocks[0].text, "ลมพัดเบาๆ");
        assert_eq!(result.blocks[1].block_type, BlockType::Dialogue);
        assert!(result.blocks[1].text.contains("สวัสดีค่า~"));
        assert_eq!(result.mood, Some("happy".to_string()));
        assert!(result.scene_update.is_none());
    }

    #[test]
    fn parse_blocks_multiple() {
        let raw = r#"{"blocks":[{"type":"narration","text":"N1"},{"type":"dialogue","text":"D1"},{"type":"narration","text":"N2"},{"type":"dialogue","text":"D2"}],"mood":"playful","scene_update":null}"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 4);
        assert_eq!(result.blocks[0].block_type, BlockType::Narration);
        assert_eq!(result.blocks[1].block_type, BlockType::Dialogue);
        assert_eq!(result.blocks[2].block_type, BlockType::Narration);
        assert_eq!(result.blocks[3].block_type, BlockType::Dialogue);
    }

    #[test]
    fn parse_blocks_markdown_fenced() {
        let raw = "```json\n{\"blocks\":[{\"type\":\"narration\",\"text\":\"N\"},{\"type\":\"dialogue\",\"text\":\"D\"}],\"mood\":\"neutral\",\"scene_update\":null}\n```";
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 2);
    }

    #[test]
    fn parse_blocks_with_text_before() {
        let raw = r#"Here is the response:

{"blocks":[{"type":"narration","text":"ลมพัด"},{"type":"dialogue","text":"สวัสดี"}],"mood":"happy","scene_update":null}"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 2);
    }

    #[test]
    fn parse_blocks_rejects_single_block() {
        // blocks format requires min 2 blocks — should fail blocks, but might work as legacy fallback...
        // Actually this has no narrator_text either, so it fails both
        let raw = r#"{"blocks":[{"type":"narration","text":"only one"}],"mood":"neutral","scene_update":null}"#;
        let result = parse_llm_response(raw);
        assert!(result.is_err());
    }

    #[test]
    fn parse_blocks_rejects_no_narration() {
        let raw = r#"{"blocks":[{"type":"dialogue","text":"D1"},{"type":"dialogue","text":"D2"}],"mood":"neutral","scene_update":null}"#;
        let result = parse_llm_response(raw);
        assert!(result.is_err());
    }

    // --- Legacy format tests (backward compat) ---

    #[test]
    fn parse_legacy_clean_json() {
        let raw = r#"{"narrator_text":"📍 ร้าน","character_text":"สวัสดี","mood":"happy","scene_update":null}"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 2);
        assert_eq!(result.blocks[0].block_type, BlockType::Narration);
        assert_eq!(result.blocks[0].text, "📍 ร้าน");
        assert_eq!(result.blocks[1].block_type, BlockType::Dialogue);
        assert_eq!(result.blocks[1].text, "สวัสดี");
        assert_eq!(result.mood, Some("happy".to_string()));
    }

    #[test]
    fn parse_legacy_markdown_fenced() {
        let raw = "```json\n{\"narrator_text\":\"N\",\"character_text\":\"C\",\"mood\":\"neutral\",\"scene_update\":null}\n```";
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 2);
        assert_eq!(result.blocks[0].text, "N");
        assert_eq!(result.blocks[1].text, "C");
    }

    #[test]
    fn parse_legacy_with_text_before() {
        let raw = r#"📍 ร้านราเมนเล็กๆ ย่านสีลม • 🕒 เย็น
Some narrative text here...

{"narrator_text":"📍 ร้าน","character_text":"สวัสดีค่า~","mood":"playful","scene_update":null}"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks[0].text, "📍 ร้าน");
        assert_eq!(result.blocks[1].text, "สวัสดีค่า~");
        assert_eq!(result.mood, Some("playful".to_string()));
    }

    #[test]
    fn parse_legacy_with_text_before_and_after() {
        let raw = r#"Here is the response:

{"narrator_text":"N","character_text":"C","mood":"happy","scene_update":null}

Hope that helps!"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks[0].text, "N");
        assert_eq!(result.blocks[1].text, "C");
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let raw = "this is not json at all";
        let result = parse_llm_response(raw);
        assert!(result.is_err());
    }

    #[test]
    fn parse_legacy_both_empty_returns_error() {
        let raw =
            r#"{"narrator_text":"","character_text":"","mood":"neutral","scene_update":null}"#;
        let result = parse_llm_response(raw);
        assert!(result.is_err());
    }

    #[test]
    fn parse_legacy_one_empty_returns_error() {
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

    #[test]
    fn parse_blocks_with_scene_update() {
        let raw = r#"{"blocks":[{"type":"narration","text":"N"},{"type":"dialogue","text":"D"}],"mood":"neutral","scene_update":{"location":"park","time":"evening","summary":"walked"}}"#;
        let result = parse_llm_response(raw).unwrap();
        let su = result.scene_update.unwrap();
        assert_eq!(su.location, Some("park".to_string()));
    }

    // --- JSON repair tests ---

    #[test]
    fn repair_stray_paren_production_error() {
        // Exact pattern from production: stray ) between closing " of text value and }
        let raw = r#"{"blocks":[{"type":"narration","text":"เธอยิ้มให้"},{"type":"dialogue","text":"\"มาช่วยจัดการเรื่องของบัคด้วยนะ\""}],"mood":"playful","scene_update":null}"#;
        // Insert stray ) between the closing " and the }
        let broken = raw.replace(r#"\""}]"#, r#"\"")}]"#);
        assert!(serde_json::from_str::<serde_json::Value>(&broken).is_err());
        let result = parse_llm_response(&broken).unwrap();
        assert_eq!(result.blocks.len(), 2);
        assert_eq!(result.mood, Some("playful".to_string()));
    }

    #[test]
    fn repair_trailing_comma_before_brace() {
        let raw = r#"{"blocks":[{"type":"narration","text":"N"},{"type":"dialogue","text":"D"}],"mood":"happy",}"#;
        assert!(serde_json::from_str::<serde_json::Value>(raw).is_err());
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.mood, Some("happy".to_string()));
    }

    #[test]
    fn repair_trailing_comma_before_bracket() {
        let raw = r#"{"blocks":[{"type":"narration","text":"N"},{"type":"dialogue","text":"D"},],"mood":"happy","scene_update":null}"#;
        assert!(serde_json::from_str::<serde_json::Value>(raw).is_err());
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 2);
    }

    #[test]
    fn repair_valid_json_unchanged() {
        let raw = r#"{"blocks":[{"type":"narration","text":"N"},{"type":"dialogue","text":"D"}],"mood":"happy","scene_update":null}"#;
        let repaired = repair_json(raw);
        assert_eq!(repaired, raw);
    }

    #[test]
    fn repair_preserves_parens_inside_strings() {
        let raw = r#"{"blocks":[{"type":"narration","text":"เธอยิ้ม (อย่างอ่อนโยน)"},{"type":"dialogue","text":"D"}],"mood":"happy","scene_update":null}"#;
        let repaired = repair_json(raw);
        assert_eq!(repaired, raw);
        let result = parse_llm_response(raw).unwrap();
        assert!(result.blocks[0].text.contains("(อย่างอ่อนโยน)"));
    }

    #[test]
    fn repair_multiple_stray_chars() {
        // Multiple stray parens outside strings
        let raw = r#"({"blocks":[{"type":"narration","text":"N"},{"type":"dialogue","text":"D"}],"mood":"happy","scene_update":null})"#;
        assert!(serde_json::from_str::<serde_json::Value>(raw).is_err());
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 2);
    }

    #[test]
    fn repair_escaped_backslash_before_close_quote() {
        // String value ends with literal backslash (\\), followed by stray ) outside
        // The \\\\ in the raw string becomes \\ in the JSON string literal,
        // which means the closing " is NOT escaped — repair must correctly
        // identify the string boundary and remove the outer paren.
        let raw = r#"{"blocks":[{"type":"narration","text":"hello\\"},{"type":"dialogue","text":"D"}],"mood":"happy","scene_update":null})"#;
        assert!(serde_json::from_str::<serde_json::Value>(raw).is_err());
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 2);
        assert_eq!(result.blocks[0].text, "hello\\");
    }
}
