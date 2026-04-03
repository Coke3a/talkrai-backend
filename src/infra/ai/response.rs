use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::domain::services::ai_client::{AiRoleplayResponse, BlockType, ResponseBlock};
use crate::domain::services::AiClientError;

#[derive(Deserialize)]
pub struct UpdateSceneStateArgs {
    pub content: String,
    pub current_location: String,
    pub scene_time: String,
    pub mood: String,
}

/// Tool definition in Anthropic format (used by Claude API).
pub fn claude_tool_definition() -> Value {
    json!({
        "name": "update_scene_state",
        "description": "Update the scene state with the roleplay response content, location, time, and mood.",
        "input_schema": {
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Response text with *narration* and dialogue markup"
                },
                "current_location": {
                    "type": "string",
                    "description": "Current scene location in Thai"
                },
                "scene_time": {
                    "type": "string",
                    "enum": ["เช้า", "สาย", "เที่ยง", "บ่าย", "เย็น", "ค่ำ", "ดึก"],
                    "description": "Time of day"
                },
                "mood": {
                    "type": "string",
                    "enum": ["neutral", "happy", "sad", "excited", "angry", "shy", "playful", "serious", "worried"],
                    "description": "Character mood"
                }
            },
            "required": ["content", "current_location", "scene_time", "mood"]
        }
    })
}

/// Tool definition in OpenAI-compatible format (used by OpenAI, Together, Venice).
pub fn openai_tool_definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "update_scene_state",
            "description": "Update the scene state with the roleplay response content, location, time, and mood.",
            "parameters": {
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "Response text with *narration* and dialogue markup"
                    },
                    "current_location": {
                        "type": "string",
                        "description": "Current scene location in Thai"
                    },
                    "scene_time": {
                        "type": "string",
                        "enum": ["เช้า", "สาย", "เที่ยง", "บ่าย", "เย็น", "ค่ำ", "ดึก"],
                        "description": "Time of day"
                    },
                    "mood": {
                        "type": "string",
                        "enum": ["neutral", "happy", "sad", "excited", "angry", "shy", "playful", "serious", "worried"],
                        "description": "Character mood"
                    }
                },
                "required": ["content", "current_location", "scene_time", "mood"]
            }
        }
    })
}

/// Convert tool call args into `AiRoleplayResponse`.
pub fn tool_args_to_response(args: UpdateSceneStateArgs) -> AiRoleplayResponse {
    tracing::info!(
        content = %args.content,
        current_location = %args.current_location,
        scene_time = %args.scene_time,
        mood = %args.mood,
        "DEBUG: tool_args_to_response — raw tool args"
    );

    let blocks = parse_text_into_blocks(&args.content);

    let block_types: Vec<&str> = blocks
        .iter()
        .map(|b| match b.block_type {
            BlockType::Narration => "narration",
            BlockType::Dialogue => "dialogue",
        })
        .collect();
    tracing::info!(
        block_count = blocks.len(),
        block_types = %format!("{:?}", block_types),
        "DEBUG: tool_args_to_response — parsed blocks"
    );

    AiRoleplayResponse {
        blocks,
        mood: Some(args.mood),
        current_location: Some(args.current_location),
        scene_time: Some(args.scene_time),
    }
}

static MOOD_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[mood:([^\]]+)\]\s*$").unwrap());

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

/// Parse LLM text markup response into `AiRoleplayResponse`.
///
/// Format: `*narration* dialogue *narration*` with optional `[mood:VALUE]` at end.
/// Never fails except on completely empty input.
pub fn parse_llm_response(raw: &str) -> Result<AiRoleplayResponse, AiClientError> {
    tracing::info!(
        raw_text = %raw,
        "DEBUG: parse_llm_response — input text"
    );

    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return Err(AiClientError::ParseError("Empty LLM response".to_string()));
    }

    // Strip markdown fences if present
    let content = if trimmed.starts_with("```") {
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

    let (text, mood) = extract_mood_tag(content);
    let blocks = parse_text_into_blocks(&text);

    let block_types: Vec<&str> = blocks
        .iter()
        .map(|b| match b.block_type {
            BlockType::Narration => "narration",
            BlockType::Dialogue => "dialogue",
        })
        .collect();
    tracing::info!(
        mood = ?mood,
        block_count = blocks.len(),
        block_types = %format!("{:?}", block_types),
        "DEBUG: parse_llm_response — parsed result"
    );

    Ok(AiRoleplayResponse {
        blocks,
        mood,
        current_location: None,
        scene_time: None,
    })
}

/// Extract `[mood:VALUE]` tag from end of text.
/// Returns (text_without_tag, Option<mood_value>).
fn extract_mood_tag(text: &str) -> (String, Option<String>) {
    if let Some(caps) = MOOD_TAG_RE.captures(text) {
        let mood = caps[1].trim().to_string();
        let without_tag = MOOD_TAG_RE.replace(text, "").trim().to_string();
        (without_tag, Some(mood))
    } else {
        (text.to_string(), None)
    }
}

/// Parse text markup into blocks using stack-based char scanner.
///
/// `*...*` = narration, everything else = dialogue.
/// `*` inside `"..."` is regular text; `"` inside `*...*` is regular text.
/// Never returns empty vec for non-empty input.
pub fn parse_text_into_blocks(text: &str) -> Vec<ResponseBlock> {
    let mut blocks: Vec<ResponseBlock> = Vec::new();
    let mut buffer = String::new();
    let mut in_narration = false;
    let mut in_quote = false;

    for ch in text.chars() {
        match ch {
            '*' if !in_quote => {
                // Flush buffer as current type
                let trimmed = buffer.trim().to_string();
                if !trimmed.is_empty() {
                    blocks.push(ResponseBlock {
                        block_type: if in_narration {
                            BlockType::Narration
                        } else {
                            BlockType::Dialogue
                        },
                        text: trimmed,
                    });
                }
                buffer.clear();
                in_narration = !in_narration;
            }
            '"' if !in_narration => {
                in_quote = !in_quote;
                buffer.push(ch);
            }
            _ => {
                buffer.push(ch);
            }
        }
    }

    // Flush remaining buffer
    let trimmed = buffer.trim().to_string();
    if !trimmed.is_empty() {
        blocks.push(ResponseBlock {
            block_type: if in_narration {
                BlockType::Narration
            } else {
                BlockType::Dialogue
            },
            text: trimmed,
        });
    }

    // Never return empty for non-empty input
    if blocks.is_empty() {
        blocks.push(ResponseBlock {
            block_type: BlockType::Dialogue,
            text: text.trim().to_string(),
        });
    }

    blocks
}

/// Validate that an AI roleplay response contains meaningful content.
///
/// Rules:
/// 1. Content must not be empty.
/// 2. Must contain at least 10 Thai or English alphabetic characters.
/// 3. Alphabetic characters must be at least 30% of total characters.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic() {
        let raw = "*เธอยิ้ม* สวัสดีค่า *เธอโบกมือ*";
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 3);
        assert_eq!(result.blocks[0].block_type, BlockType::Narration);
        assert_eq!(result.blocks[0].text, "เธอยิ้ม");
        assert_eq!(result.blocks[1].block_type, BlockType::Dialogue);
        assert_eq!(result.blocks[1].text, "สวัสดีค่า");
        assert_eq!(result.blocks[2].block_type, BlockType::Narration);
        assert_eq!(result.blocks[2].text, "เธอโบกมือ");
        assert!(result.mood.is_none());
    }

    #[test]
    fn parse_with_mood() {
        let raw = "*เธอยิ้ม* สวัสดีค่า\n[mood:playful]";
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 2);
        assert_eq!(result.blocks[0].block_type, BlockType::Narration);
        assert_eq!(result.blocks[1].block_type, BlockType::Dialogue);
        assert_eq!(result.mood, Some("playful".to_string()));
    }

    #[test]
    fn parse_production_pattern() {
        let raw = r#"*เสียงกระดิ่งเล็กๆ ดังกริ๊งเบาๆ เมื่อประตูร้านถูกผลักเปิดออก กลิ่นขนมปังอบใหม่ลอยมาต้อนรับ ราวกับอ้อมแขนที่อบอุ่น* "สวัสดีค่า~ วันนี้มีครัวซองต์เนยสด กับชิอาบัตตาหน้าอโวคาโดนะคะ" *เธอยิ้มพลางชี้ไปที่ตะกร้าหวายบนเคาน์เตอร์ ที่ขนมปังสีน้ำตาลทองเรียงตัวกันอย่างน่ารัก ไอความร้อนยังลอยเป็นสายบางๆ*
[mood:happy]"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 3);
        assert_eq!(result.blocks[0].block_type, BlockType::Narration);
        assert_eq!(result.blocks[1].block_type, BlockType::Dialogue);
        assert_eq!(result.blocks[2].block_type, BlockType::Narration);
        assert_eq!(result.mood, Some("happy".to_string()));
        assert!(result.blocks[0].text.contains("กระดิ่ง"));
        assert!(result.blocks[1].text.contains("สวัสดีค่า~"));
        assert!(result.blocks[2].text.contains("ตะกร้าหวาย"));
    }

    #[test]
    fn parse_dialogue_only() {
        let raw = "สวัสดีค่า ยินดีต้อนรับนะคะ";
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.blocks[0].block_type, BlockType::Dialogue);
        assert_eq!(result.blocks[0].text, "สวัสดีค่า ยินดีต้อนรับนะคะ");
    }

    #[test]
    fn parse_preserves_quotes() {
        let raw = r#"*เธอยิ้ม* "สวัสดี" *พยักหน้า*"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 3);
        assert_eq!(result.blocks[1].block_type, BlockType::Dialogue);
        assert!(result.blocks[1].text.contains('"'));
    }

    #[test]
    fn parse_unclosed_asterisk() {
        let raw = "*เธอยิ้ม* สวัสดี *เธอโบกมือ";
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 3);
        assert_eq!(result.blocks[0].block_type, BlockType::Narration);
        assert_eq!(result.blocks[1].block_type, BlockType::Dialogue);
        assert_eq!(result.blocks[2].block_type, BlockType::Narration);
        assert_eq!(result.blocks[2].text, "เธอโบกมือ");
    }

    #[test]
    fn parse_unclosed_quote() {
        let raw = r#"*เธอยิ้ม* "สวัสดี ไม่ปิด"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 2);
        assert_eq!(result.blocks[0].block_type, BlockType::Narration);
        assert_eq!(result.blocks[1].block_type, BlockType::Dialogue);
    }

    #[test]
    fn parse_asterisk_inside_quotes() {
        let raw = r#"*N* "D *not narration* D" *N2*"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 3);
        assert_eq!(result.blocks[0].block_type, BlockType::Narration);
        assert_eq!(result.blocks[0].text, "N");
        assert_eq!(result.blocks[1].block_type, BlockType::Dialogue);
        assert!(result.blocks[1].text.contains("*not narration*"));
        assert_eq!(result.blocks[2].block_type, BlockType::Narration);
        assert_eq!(result.blocks[2].text, "N2");
    }

    #[test]
    fn parse_quote_inside_narration() {
        let raw = r#"*เธอพูดว่า "อะไรนะ" แล้วหัวเราะ*"#;
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.blocks[0].block_type, BlockType::Narration);
        assert!(result.blocks[0].text.contains(r#""อะไรนะ""#));
    }

    #[test]
    fn parse_malformed_mood_tag() {
        let raw = "*เธอยิ้ม* สวัสดี [mood:";
        let result = parse_llm_response(raw).unwrap();
        assert!(result.mood.is_none());
        // The malformed tag text stays as content
        assert_eq!(result.blocks.len(), 2);
    }

    #[test]
    fn parse_no_mood_tag() {
        let raw = "*เธอยิ้ม* สวัสดี";
        let result = parse_llm_response(raw).unwrap();
        assert!(result.mood.is_none());
    }

    #[test]
    fn parse_markdown_fenced() {
        let raw = "```\n*เธอยิ้ม* สวัสดี\n[mood:happy]\n```";
        let result = parse_llm_response(raw).unwrap();
        assert_eq!(result.blocks.len(), 2);
        assert_eq!(result.mood, Some("happy".to_string()));
    }

    #[test]
    fn parse_empty_returns_error() {
        let result = parse_llm_response("");
        assert!(result.is_err());
        let result2 = parse_llm_response("   ");
        assert!(result2.is_err());
    }

    #[test]
    fn extract_mood_tag_valid() {
        let (text, mood) = extract_mood_tag("สวัสดี\n[mood:happy]");
        assert_eq!(mood, Some("happy".to_string()));
        assert_eq!(text, "สวัสดี");
    }

    #[test]
    fn extract_mood_tag_malformed() {
        let (text, mood) = extract_mood_tag("สวัสดี [mood:");
        assert!(mood.is_none());
        assert_eq!(text, "สวัสดี [mood:");
    }

    #[test]
    fn parse_blocks_unit_multiple_narration() {
        let blocks = parse_text_into_blocks("*N1* D *N2* D2");
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0].block_type, BlockType::Narration);
        assert_eq!(blocks[0].text, "N1");
        assert_eq!(blocks[1].block_type, BlockType::Dialogue);
        assert_eq!(blocks[1].text, "D");
        assert_eq!(blocks[2].block_type, BlockType::Narration);
        assert_eq!(blocks[2].text, "N2");
        assert_eq!(blocks[3].block_type, BlockType::Dialogue);
        assert_eq!(blocks[3].text, "D2");
    }

    #[test]
    fn parse_blocks_unit_empty_segments_skipped() {
        let blocks = parse_text_into_blocks("** สวัสดี **");
        // empty narration segments are skipped, only dialogue remains
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Dialogue);
        assert_eq!(blocks[0].text, "สวัสดี");
    }

    #[test]
    fn tool_args_to_response_produces_correct_blocks_and_state() {
        let args = UpdateSceneStateArgs {
            content: "*เธอยิ้ม* สวัสดีค่า".to_string(),
            current_location: "ร้านกาแฟ".to_string(),
            scene_time: "บ่าย".to_string(),
            mood: "happy".to_string(),
        };
        let resp = tool_args_to_response(args);
        assert_eq!(resp.blocks.len(), 2);
        assert_eq!(resp.blocks[0].block_type, BlockType::Narration);
        assert_eq!(resp.blocks[0].text, "เธอยิ้ม");
        assert_eq!(resp.blocks[1].block_type, BlockType::Dialogue);
        assert_eq!(resp.blocks[1].text, "สวัสดีค่า");
        assert_eq!(resp.mood, Some("happy".to_string()));
        assert_eq!(resp.current_location, Some("ร้านกาแฟ".to_string()));
        assert_eq!(resp.scene_time, Some("บ่าย".to_string()));
    }

    #[test]
    fn parse_llm_response_sets_location_and_time_to_none() {
        let raw = "*เธอยิ้ม* สวัสดี\n[mood:happy]";
        let result = parse_llm_response(raw).unwrap();
        assert!(result.current_location.is_none());
        assert!(result.scene_time.is_none());
    }

    // --- validate_ai_response tests ---

    fn make_response(content: &str) -> AiRoleplayResponse {
        AiRoleplayResponse {
            blocks: vec![ResponseBlock {
                block_type: BlockType::Dialogue,
                text: content.to_string(),
            }],
            mood: None,
            current_location: None,
            scene_time: None,
        }
    }

    #[test]
    fn validate_rejects_single_bracket() {
        let resp = make_response("[");
        assert!(validate_ai_response(&resp).is_err());
    }

    #[test]
    fn validate_rejects_only_special_chars() {
        let resp = make_response("***!!!~~~[]");
        assert!(validate_ai_response(&resp).is_err());
    }

    #[test]
    fn validate_rejects_empty() {
        let resp = make_response("");
        assert!(validate_ai_response(&resp).is_err());
    }

    #[test]
    fn validate_rejects_too_short_thai() {
        // 3 Thai chars — below threshold of 10
        let resp = make_response("ค่ะ");
        assert!(validate_ai_response(&resp).is_err());
    }

    #[test]
    fn validate_rejects_low_alpha_ratio() {
        // 10 Thai alpha chars buried in 60+ special chars — passes min count but ratio < 30%
        let resp =
            make_response("[[[***///---!!!สวัสดีค่ะนะคะ!!!~~~]]]***///---!!!~~~[[[***///---!!!~~~]]]");
        let result = validate_ai_response(&resp);
        assert!(result.is_err());
    }

    #[test]
    fn validate_accepts_normal_thai_response() {
        let resp = make_response("สวัสดีค่า ยินดีต้อนรับนะคะ วันนี้มีอะไรให้ช่วยไหมคะ");
        assert!(validate_ai_response(&resp).is_ok());
    }

    #[test]
    fn validate_accepts_narration_with_dialogue() {
        let resp = AiRoleplayResponse {
            blocks: vec![
                ResponseBlock {
                    block_type: BlockType::Narration,
                    text: "เธอยิ้มอย่างอ่อนโยน".to_string(),
                },
                ResponseBlock {
                    block_type: BlockType::Dialogue,
                    text: "สวัสดีค่า~ วันนี้อากาศดีจังเลยนะ".to_string(),
                },
            ],
            mood: Some("happy".to_string()),
            current_location: None,
            scene_time: None,
        };
        assert!(validate_ai_response(&resp).is_ok());
    }

    #[test]
    fn validate_accepts_english_response() {
        let resp = make_response("Hello there, how are you doing today?");
        assert!(validate_ai_response(&resp).is_ok());
    }

    #[test]
    fn validate_rejects_emoji_only() {
        let resp = make_response("🙏🙏🙏🙏🙏🙏🙏🙏🙏🙏");
        assert!(validate_ai_response(&resp).is_err());
    }
}
