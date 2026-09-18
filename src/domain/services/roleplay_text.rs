use super::ai_client::{BlockType, ResponseBlock};

pub fn wrap_character_content(raw: &str) -> String {
    if raw.trim_start().starts_with('[') {
        // Old blocks JSON format → convert to text markup
        blocks_json_to_text_markup(raw)
    } else {
        // New text markup or plain text → pass through
        raw.to_string()
    }
}

/// Convert old JSON blocks array to text markup format.
/// `[{"type":"narration","text":"N"},{"type":"dialogue","text":"D"}]` → `*N*\nD`
pub fn blocks_json_to_text_markup(json_str: &str) -> String {
    serde_json::from_str::<Vec<serde_json::Value>>(json_str)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| {
                    let text = b["text"].as_str()?;
                    let btype = b["type"].as_str().unwrap_or("dialogue");
                    Some(if btype == "narration" {
                        format!("*{}*", text)
                    } else {
                        text.to_string()
                    })
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|_| json_str.to_string())
}

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
