use serde_json::{json, Value};

use crate::domain::services::ai_client::{BlockType, ResponseBlock};

/// Map a color_tone name to (accent, gradient_start, gradient_end).
pub fn color_tone_to_colors(tone: &str) -> (&'static str, &'static str, &'static str) {
    match tone {
        "warm_golden" => ("#FFB74D", "#1a1508", "#1f1a0d"),
        "cool_blue" => ("#64B5F6", "#141519", "#191a1f"),
        "soft_pink" => ("#F48FB1", "#1a1412", "#1f1917"),
        "deep_purple" => ("#B388FF", "#181418", "#1d191e"),
        "neon_night" => ("#69F0AE", "#141a12", "#191f17"),
        "sunset_orange" => ("#FF8A65", "#1a1410", "#1f1915"),
        "moonlight_silver" => ("#B0BEC5", "#171514", "#1c1a19"),
        "forest_green" => ("#81C784", "#151a10", "#1a1f15"),
        "storm_gray" => ("#90A4AE", "#161514", "#1b1a19"),
        "cherry_blossom" => ("#F8BBD0", "#1a1413", "#1f1918"),
        _ => ("#A0A0A0", "#171513", "#1c1a18"),
    }
}

/// Convert Thai time-of-day text to emoji + label.
pub fn time_period_display(time_of_day: &str) -> String {
    if time_of_day.contains("เช้า") {
        "🌅 เช้า".to_string()
    } else if time_of_day.contains("สาย") {
        "☀️ สาย".to_string()
    } else if time_of_day.contains("เที่ยง") {
        "🌞 เที่ยง".to_string()
    } else if time_of_day.contains("บ่าย") {
        "🌤️ บ่าย".to_string()
    } else if time_of_day.contains("เย็น") {
        "🌇 เย็น".to_string()
    } else if time_of_day.contains("ค่ำ") {
        "🌙 ค่ำ".to_string()
    } else if time_of_day.contains("ดึก") {
        "🌑 ดึก".to_string()
    } else {
        let first_word = time_of_day.split_whitespace().next().unwrap_or(time_of_day);
        format!("🕐 {first_word}")
    }
}

/// Build a novel-flow Flex Bubble from response blocks.
///
/// narration blocks: gray text (xs), dialogue blocks: white text (sm, bold).
/// Header always shows location + time.
pub fn build_roleplay_blocks_bubble(
    blocks: &[ResponseBlock],
    location: &str,
    time_of_day: &str,
    color_tone: &str,
) -> Value {
    let block_details: Vec<String> = blocks
        .iter()
        .map(|b| {
            let btype = match b.block_type {
                BlockType::Narration => "narration",
                BlockType::Dialogue => "dialogue",
            };
            format!("[{}] {}", btype, b.text)
        })
        .collect();
    tracing::info!(
        block_count = blocks.len(),
        blocks = %block_details.join(" | "),
        location = %location,
        time_of_day = %time_of_day,
        color_tone = %color_tone,
        "DEBUG: build_roleplay_blocks_bubble — input"
    );

    let (accent, gradient_start, gradient_end) = color_tone_to_colors(color_tone);
    let time_display = time_period_display(time_of_day);

    let mut contents: Vec<Value> = Vec::new();

    // Brand accent bar
    contents.push(json!({
        "type": "box",
        "layout": "vertical",
        "height": "3px",
        "backgroundColor": "#F96D4B",
        "contents": []
    }));

    // Header: location + time
    contents.push(json!({
        "type": "box",
        "layout": "horizontal",
        "justifyContent": "space-between",
        "margin": "lg",
        "contents": [
            {
                "type": "text",
                "text": format!("📍 {location}"),
                "color": accent,
                "size": "xs",
                "weight": "bold",
                "wrap": true
            },
            {
                "type": "text",
                "text": time_display,
                "color": "#9B9186",
                "size": "xxs",
                "flex": 0
            }
        ]
    }));

    // Render each block
    for block in blocks {
        match block.block_type {
            BlockType::Narration => {
                contents.push(json!({
                    "type": "box",
                    "layout": "vertical",
                    "paddingStart": "8px",
                    "margin": "lg",
                    "contents": [
                        {
                            "type": "text",
                            "text": block.text,
                            "color": "#B8AFA5",
                            "size": "xs",
                            "wrap": true
                        }
                    ]
                }));
            }
            BlockType::Dialogue => {
                contents.push(json!({
                    "type": "text",
                    "text": block.text,
                    "color": "#FFFFFF",
                    "size": "sm",
                    "weight": "bold",
                    "wrap": true,
                    "margin": "lg"
                }));
            }
        }
    }

    let bubble = json!({
        "type": "bubble",
        "size": "mega",
        "styles": {
            "body": {
                "backgroundColor": "#00000000"
            }
        },
        "body": {
            "type": "box",
            "layout": "vertical",
            "background": {
                "type": "linearGradient",
                "angle": "180deg",
                "startColor": gradient_start,
                "endColor": gradient_end
            },
            "paddingAll": "24px",
            "contents": contents
        }
    });

    tracing::info!(
        bubble_json = %serde_json::to_string(&bubble).unwrap_or_default(),
        "DEBUG: build_roleplay_blocks_bubble — output Flex JSON"
    );

    bubble
}

/// Truncate text to fit LINE's 400-char altText limit.
pub fn truncate_alt_text(text: &str) -> String {
    if text.chars().count() <= 400 {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(397).collect();
        format!("{truncated}...")
    }
}

/// Extract `color_tone` field from a JSON atmosphere string. Falls back to "neutral".
pub fn extract_color_tone(atmosphere: &str) -> String {
    let trimmed = atmosphere.trim();
    if !trimmed.starts_with('{') {
        tracing::info!(
            atmosphere = %atmosphere,
            color_tone = "neutral",
            "DEBUG: extract_color_tone — plain text atmosphere, defaulting to neutral"
        );
        return "neutral".to_string();
    }
    let tone = serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|v| v.get("color_tone")?.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "neutral".to_string());

    tracing::info!(
        atmosphere = %atmosphere,
        color_tone = %tone,
        "DEBUG: extract_color_tone — extracted"
    );

    tone
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_tone_known_values() {
        let (accent, gs, ge) = color_tone_to_colors("warm_golden");
        assert_eq!(accent, "#FFB74D");
        assert_eq!(gs, "#1a1508");
        assert_eq!(ge, "#1f1a0d");
    }

    #[test]
    fn color_tone_fallback() {
        let (accent, gs, ge) = color_tone_to_colors("unknown_tone");
        assert_eq!(accent, "#A0A0A0");
        assert_eq!(gs, "#171513");
        assert_eq!(ge, "#1c1a18");
    }

    #[test]
    fn time_period_thai_keywords() {
        assert_eq!(time_period_display("เช้าตรู่"), "🌅 เช้า");
        assert_eq!(time_period_display("ช่วงสาย"), "☀️ สาย");
        assert_eq!(time_period_display("ตอนเที่ยง"), "🌞 เที่ยง");
        assert_eq!(time_period_display("บ่ายแก่ๆ"), "🌤️ บ่าย");
        assert_eq!(time_period_display("เย็น"), "🌇 เย็น");
        assert_eq!(time_period_display("ค่ำ"), "🌙 ค่ำ");
        assert_eq!(time_period_display("ดึกมาก"), "🌑 ดึก");
    }

    #[test]
    fn time_period_fallback() {
        assert_eq!(time_period_display("midnight"), "🕐 midnight");
        assert_eq!(time_period_display("dawn hour"), "🕐 dawn");
    }

    #[test]
    fn truncate_alt_text_short() {
        let short = "Hello";
        assert_eq!(truncate_alt_text(short), "Hello");
    }

    #[test]
    fn truncate_alt_text_long() {
        let long: String = "あ".repeat(500);
        let result = truncate_alt_text(&long);
        assert!(result.chars().count() <= 400);
        assert!(result.ends_with("..."));
    }

    // --- build_roleplay_blocks_bubble tests ---

    #[test]
    fn blocks_bubble_basic() {
        let blocks = vec![
            ResponseBlock {
                block_type: BlockType::Narration,
                text: "ลมพัดเบาๆ".to_string(),
            },
            ResponseBlock {
                block_type: BlockType::Dialogue,
                text: "\"สวัสดีค่า~\"".to_string(),
            },
        ];

        let bubble = build_roleplay_blocks_bubble(&blocks, "ร้านกาแฟ", "เช้าตรู่", "warm_golden");

        assert_eq!(bubble["type"], "bubble");
        assert_eq!(bubble["size"], "mega");

        let contents = bubble["body"]["contents"].as_array().unwrap();
        // accent bar + header + 2 blocks = 4
        assert_eq!(contents.len(), 4);

        // Brand accent bar
        assert_eq!(contents[0]["height"], "3px");
        assert_eq!(contents[0]["backgroundColor"], "#F96D4B");

        // Header
        assert_eq!(contents[1]["contents"][0]["text"], "📍 ร้านกาแฟ");
        assert_eq!(contents[1]["contents"][1]["text"], "🌅 เช้า");

        // Narration block (wrapped in indented box)
        assert_eq!(contents[2]["type"], "box");
        assert_eq!(contents[2]["paddingStart"], "8px");
        assert_eq!(contents[2]["margin"], "lg");
        assert_eq!(contents[2]["contents"][0]["text"], "ลมพัดเบาๆ");
        assert_eq!(contents[2]["contents"][0]["color"], "#B8AFA5");
        assert_eq!(contents[2]["contents"][0]["size"], "xs");

        // Dialogue block
        assert_eq!(contents[3]["text"], "\"สวัสดีค่า~\"");
        assert_eq!(contents[3]["color"], "#FFFFFF");
        assert_eq!(contents[3]["size"], "sm");
        assert_eq!(contents[3]["weight"], "bold");
        assert_eq!(contents[3]["margin"], "lg");
    }

    #[test]
    fn blocks_bubble_multiple_blocks() {
        let blocks = vec![
            ResponseBlock {
                block_type: BlockType::Narration,
                text: "N1".to_string(),
            },
            ResponseBlock {
                block_type: BlockType::Dialogue,
                text: "D1".to_string(),
            },
            ResponseBlock {
                block_type: BlockType::Narration,
                text: "N2".to_string(),
            },
            ResponseBlock {
                block_type: BlockType::Dialogue,
                text: "D2".to_string(),
            },
        ];

        let bubble = build_roleplay_blocks_bubble(&blocks, "สวน", "เย็น", "cool_blue");

        let contents = bubble["body"]["contents"].as_array().unwrap();
        // accent bar + header + 4 blocks = 6
        assert_eq!(contents.len(), 6);

        assert_eq!(contents[2]["contents"][0]["color"], "#B8AFA5"); // narration (in box)
        assert_eq!(contents[3]["color"], "#FFFFFF"); // dialogue
        assert_eq!(contents[4]["contents"][0]["color"], "#B8AFA5"); // narration (in box)
        assert_eq!(contents[5]["color"], "#FFFFFF"); // dialogue
    }

    #[test]
    fn blocks_bubble_uses_gradient_background() {
        let blocks = vec![
            ResponseBlock {
                block_type: BlockType::Narration,
                text: "N".to_string(),
            },
            ResponseBlock {
                block_type: BlockType::Dialogue,
                text: "D".to_string(),
            },
        ];

        let bubble = build_roleplay_blocks_bubble(&blocks, "ร้าน", "ค่ำ", "deep_purple");

        let bg = &bubble["body"]["background"];
        assert_eq!(bg["type"], "linearGradient");
        assert_eq!(bg["startColor"], "#181418");
        assert_eq!(bg["endColor"], "#1d191e");
    }
}
