use std::sync::LazyLock;

use regex::Regex;
use serde_json::{json, Value};

static ACTION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*([^*]+)\*").unwrap());

/// Map a color_tone name to (accent, gradient_start, gradient_end).
pub fn color_tone_to_colors(tone: &str) -> (&'static str, &'static str, &'static str) {
    match tone {
        "warm_golden" => ("#FFB74D", "#1a1508", "#1f1a0f"),
        "cool_blue" => ("#64B5F6", "#0a0f1a", "#0f141f"),
        "soft_pink" => ("#F48FB1", "#1a0a10", "#1f0f15"),
        "deep_purple" => ("#B388FF", "#0f0a1a", "#14101f"),
        "neon_night" => ("#69F0AE", "#0a1a10", "#0f1f15"),
        "sunset_orange" => ("#FF8A65", "#1a100a", "#1f150f"),
        "moonlight_silver" => ("#B0BEC5", "#0f1012", "#141517"),
        "forest_green" => ("#81C784", "#0a1a0c", "#0f1f11"),
        "storm_gray" => ("#90A4AE", "#0f1113", "#141618"),
        "cherry_blossom" => ("#F8BBD0", "#1a0a12", "#1f0f17"),
        _ => ("#A0A0A0", "#111111", "#1a1a1a"),
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

/// Parse `*action*` markers in text into a span array for Flex Message.
pub fn parse_action_spans(text: &str, accent: &str) -> Vec<Value> {
    let mut spans = Vec::new();
    let mut last_end = 0;

    for cap in ACTION_RE.captures_iter(text) {
        let whole = cap.get(0).unwrap();
        let action = cap.get(1).unwrap();

        // Text before this action
        if whole.start() > last_end {
            let before = &text[last_end..whole.start()];
            if !before.is_empty() {
                spans.push(json!({
                    "type": "span",
                    "text": before,
                    "color": "#FFFFFF",
                    "size": "md"
                }));
            }
        }

        // Action span
        spans.push(json!({
            "type": "span",
            "text": action.as_str(),
            "color": accent,
            "size": "xs"
        }));

        last_end = whole.end();
    }

    // Remaining text after last action
    if last_end < text.len() {
        let remaining = &text[last_end..];
        if !remaining.is_empty() {
            spans.push(json!({
                "type": "span",
                "text": remaining,
                "color": "#FFFFFF",
                "size": "md"
            }));
        }
    }

    // No actions found → single span of the whole text
    if spans.is_empty() {
        spans.push(json!({
            "type": "span",
            "text": text,
            "color": "#FFFFFF",
            "size": "md"
        }));
    }

    spans
}

/// Build a combined roleplay Flex Bubble with narrator + character zones.
pub fn build_roleplay_bubble(
    narrator_text: &str,
    character_text: &str,
    character_name: &str,
    avatar_url: Option<&str>,
    location: &str,
    time_of_day: &str,
    color_tone: &str,
) -> Value {
    let (accent, gradient_start, gradient_end) = color_tone_to_colors(color_tone);
    let has_narrator = !narrator_text.is_empty();
    let has_character = !character_text.is_empty();

    let mut contents: Vec<Value> = Vec::new();

    // Narrator zone: header + narrator text
    if has_narrator {
        let time_display = time_period_display(time_of_day);
        contents.push(json!({
            "type": "box",
            "layout": "horizontal",
            "justifyContent": "space-between",
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
                    "color": "#888888",
                    "size": "xxs",
                    "flex": 0
                }
            ]
        }));
        contents.push(json!({
            "type": "text",
            "text": narrator_text,
            "color": "#A0A0A0",
            "size": "xs",
            "wrap": true,
            "margin": "md"
        }));
    }

    // Accent divider: only when both zones present
    if has_narrator && has_character {
        contents.push(json!({
            "type": "box",
            "layout": "vertical",
            "height": "3px",
            "backgroundColor": accent,
            "margin": "lg",
            "contents": []
        }));
    }

    // Character zone: name label + spans
    if has_character {
        let spans = parse_action_spans(character_text, accent);
        let name_margin = if has_narrator { "lg" } else { "none" };
        let has_avatar = avatar_url.filter(|u| !u.is_empty()).is_some();

        if has_avatar {
            contents.push(json!({
                "type": "box",
                "layout": "horizontal",
                "spacing": "sm",
                "alignItems": "center",
                "margin": name_margin,
                "contents": [
                    {
                        "type": "image",
                        "url": avatar_url.unwrap(),
                        "size": "xxs",
                        "aspectRatio": "1:1",
                        "aspectMode": "cover",
                        "flex": 0
                    },
                    {
                        "type": "text",
                        "text": character_name,
                        "color": accent,
                        "size": "sm",
                        "weight": "bold",
                        "flex": 1
                    }
                ]
            }));
        } else {
            contents.push(json!({
                "type": "text",
                "text": character_name,
                "color": accent,
                "size": "sm",
                "weight": "bold",
                "margin": name_margin
            }));
        }
        contents.push(json!({
            "type": "text",
            "text": " ",
            "contents": spans,
            "wrap": true,
            "margin": "sm"
        }));
    }

    json!({
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
            "paddingAll": "20px",
            "contents": contents
        }
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_tone_known_values() {
        let (accent, gs, ge) = color_tone_to_colors("warm_golden");
        assert_eq!(accent, "#FFB74D");
        assert_eq!(gs, "#1a1508");
        assert_eq!(ge, "#1f1a0f");
    }

    #[test]
    fn color_tone_fallback() {
        let (accent, gs, ge) = color_tone_to_colors("unknown_tone");
        assert_eq!(accent, "#A0A0A0");
        assert_eq!(gs, "#111111");
        assert_eq!(ge, "#1a1a1a");
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
    fn parse_action_spans_no_actions() {
        let spans = parse_action_spans("สวัสดีค่า", "#FFB74D");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0]["text"], "สวัสดีค่า");
        assert_eq!(spans[0]["color"], "#FFFFFF");
        assert_eq!(spans[0]["size"], "md");
    }

    #[test]
    fn parse_action_spans_with_actions() {
        let spans = parse_action_spans("เธอ*ยิ้ม*แล้วพูดว่า", "#FFB74D");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0]["text"], "เธอ");
        assert_eq!(spans[0]["color"], "#FFFFFF");
        assert_eq!(spans[1]["text"], "ยิ้ม");
        assert_eq!(spans[1]["color"], "#FFB74D");
        assert_eq!(spans[1]["size"], "xs");
        assert_eq!(spans[2]["text"], "แล้วพูดว่า");
    }

    #[test]
    fn parse_action_spans_multiple_actions() {
        let spans = parse_action_spans("*พยักหน้า*ได้เลย*ยิ้ม*", "#64B5F6");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0]["text"], "พยักหน้า");
        assert_eq!(spans[0]["color"], "#64B5F6");
        assert_eq!(spans[1]["text"], "ได้เลย");
        assert_eq!(spans[1]["color"], "#FFFFFF");
        assert_eq!(spans[2]["text"], "ยิ้ม");
        assert_eq!(spans[2]["color"], "#64B5F6");
    }

    #[test]
    fn build_roleplay_bubble_combined() {
        let bubble = build_roleplay_bubble(
            "ลมพัดเบาๆ",
            "สวัสดีค่า~ *ยิ้ม*",
            "มิโกะ",
            Some("https://example.com/miko.png"),
            "ร้านกาแฟ",
            "เช้าตรู่",
            "warm_golden",
        );
        assert_eq!(bubble["type"], "bubble");
        assert_eq!(bubble["size"], "mega");

        let body = &bubble["body"];
        assert_eq!(body["type"], "box");
        assert_eq!(body["layout"], "vertical");

        let contents = body["contents"].as_array().unwrap();
        // header + narrator + divider + character name box + spans
        assert_eq!(contents.len(), 5);

        // Header: location + time
        assert_eq!(contents[0]["contents"][0]["text"], "📍 ร้านกาแฟ");
        assert_eq!(contents[0]["contents"][1]["text"], "🌅 เช้า");

        // Narrator text (xs size)
        assert_eq!(contents[1]["text"], "ลมพัดเบาๆ");
        assert_eq!(contents[1]["size"], "xs");
        assert_eq!(contents[1]["color"], "#A0A0A0");

        // Accent divider
        assert_eq!(contents[2]["height"], "3px");
        assert_eq!(contents[2]["backgroundColor"], "#FFB74D");

        // Character name box (avatar + text)
        assert_eq!(contents[3]["type"], "box");
        assert_eq!(contents[3]["layout"], "horizontal");
        assert_eq!(contents[3]["margin"], "lg");
        let name_box = contents[3]["contents"].as_array().unwrap();
        assert_eq!(name_box[0]["type"], "image");
        assert_eq!(name_box[0]["url"], "https://example.com/miko.png");
        assert_eq!(name_box[1]["type"], "text");
        assert_eq!(name_box[1]["text"], "มิโกะ");
        assert_eq!(name_box[1]["color"], "#FFB74D");

        // Character spans
        assert_eq!(contents[4]["text"], " ");
        let spans = contents[4]["contents"].as_array().unwrap();
        assert!(spans.len() >= 2);
        assert_eq!(contents[4]["margin"], "sm");
    }

    #[test]
    fn build_roleplay_bubble_narrator_only() {
        let bubble = build_roleplay_bubble(
            "ลมพัดเบาๆ",
            "",
            "มิโกะ",
            Some("https://example.com/miko.png"),
            "ร้านกาแฟ",
            "เช้าตรู่",
            "warm_golden",
        );

        let contents = bubble["body"]["contents"].as_array().unwrap();
        // header + narrator (no divider, no spans, no footer)
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0]["contents"][0]["text"], "📍 ร้านกาแฟ");
        assert_eq!(contents[1]["text"], "ลมพัดเบาๆ");
    }

    #[test]
    fn build_roleplay_bubble_character_only() {
        let bubble =
            build_roleplay_bubble("", "สวัสดีค่า~", "มิโกะ", None, "ร้านกาแฟ", "เช้าตรู่", "cool_blue");

        let contents = bubble["body"]["contents"].as_array().unwrap();
        // character name + spans (no header, no narrator, no divider)
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0]["text"], "มิโกะ");
        assert_eq!(contents[0]["color"], "#64B5F6");
        assert_eq!(contents[0]["size"], "sm");
        assert_eq!(contents[0]["weight"], "bold");
        assert_eq!(contents[0]["margin"], "none");
        assert_eq!(contents[1]["text"], " ");
        assert_eq!(contents[1]["margin"], "sm");
    }

    #[test]
    fn build_roleplay_bubble_character_with_avatar() {
        let bubble = build_roleplay_bubble(
            "",
            "สวัสดีค่า~",
            "มิโกะ",
            Some("https://example.com/miko.png"),
            "ร้านกาแฟ",
            "เช้าตรู่",
            "cool_blue",
        );

        let contents = bubble["body"]["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 2);

        // Character name box (avatar + text)
        assert_eq!(contents[0]["type"], "box");
        assert_eq!(contents[0]["layout"], "horizontal");
        assert_eq!(contents[0]["margin"], "none");
        let name_box = contents[0]["contents"].as_array().unwrap();
        assert_eq!(name_box[0]["type"], "image");
        assert_eq!(name_box[0]["url"], "https://example.com/miko.png");
        assert_eq!(name_box[0]["size"], "xxs");
        assert_eq!(name_box[1]["type"], "text");
        assert_eq!(name_box[1]["text"], "มิโกะ");
        assert_eq!(name_box[1]["color"], "#64B5F6");
    }

    #[test]
    fn build_roleplay_bubble_empty_avatar_url_falls_back() {
        let bubble = build_roleplay_bubble(
            "",
            "สวัสดีค่า~",
            "มิโกะ",
            Some(""),
            "ร้านกาแฟ",
            "เช้าตรู่",
            "cool_blue",
        );

        let contents = bubble["body"]["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 2);

        // Falls back to plain text (not box)
        assert_eq!(contents[0]["type"], "text");
        assert_eq!(contents[0]["text"], "มิโกะ");
        assert_eq!(contents[0]["color"], "#64B5F6");
        assert_eq!(contents[0]["size"], "sm");
        assert_eq!(contents[0]["weight"], "bold");
        assert_eq!(contents[0]["margin"], "none");
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
}
