//! Flex builders for the retention loop (spec 2026-06-24 §B.2/§B.3) — Midnight Theatre **dark**
//! style (hex approximations of the OKLCH tokens, since LINE Flex needs hex). Distinct from the
//! older cream cards in `flex_messages.rs`. No floating hearts/sparkles (PRODUCT.md ban).

use serde_json::{json, Value};

// Hex approximations of the DESIGN.md OKLCH tokens.
const INK_900: &str = "#1C1118"; // page ground
const INK_700: &str = "#473540"; // hairline / empty pip
const BONE: &str = "#EFE7DC"; // text
const BONE_MUTED: &str = "#A79D90"; // muted text
const GOLD: &str = "#D8B15A"; // champagne accent / numerals
const ROSE: &str = "#E0708A"; // ember rose — relationship / streak
const LINE_GREEN: &str = "#06C755"; // LINE brand green — the one functional CTA

const MAX_PIPS: i32 = 7;

/// Render the streak as filled/empty circle glyphs (single rose-tinted run — robust across LINE
/// renderers, the shape carries the progress).
fn streak_pips(streak: i32) -> String {
    let filled = streak.clamp(0, MAX_PIPS);
    (0..MAX_PIPS)
        .map(|i| if i < filled { '●' } else { '○' })
        .collect()
}

/// Daily check-in greeting — rides ahead of the first reply of the day. Credits are secondary to
/// the character's "glad you're back" (spec §B.3).
pub fn build_daily_checkin_flex(character_name: &str, streak: i32, credits: i32) -> Value {
    json!({
        "type": "bubble",
        "size": "kilo",
        "body": {
            "type": "box",
            "layout": "vertical",
            "backgroundColor": INK_900,
            "paddingAll": "20px",
            "spacing": "sm",
            "contents": [
                { "type": "text", "text": "ยินดีที่กลับมานะ 🌙", "color": BONE, "size": "lg", "weight": "bold", "wrap": true },
                { "type": "text", "text": format!("{} คิดถึงคุณอยู่พอดีเลย", character_name), "color": BONE_MUTED, "size": "sm", "style": "italic", "wrap": true },
                { "type": "separator", "margin": "lg", "color": INK_700 },
                { "type": "box", "layout": "baseline", "margin": "lg", "spacing": "sm", "contents": [
                    { "type": "text", "text": "ต่อเนื่อง", "color": BONE_MUTED, "size": "xs", "flex": 0 },
                    { "type": "text", "text": streak_pips(streak), "color": ROSE, "size": "sm", "flex": 0 },
                    { "type": "text", "text": format!("วันที่ {}", streak), "color": GOLD, "size": "sm", "weight": "bold", "align": "end" }
                ]},
                { "type": "box", "layout": "baseline", "margin": "sm", "spacing": "sm", "contents": [
                    { "type": "text", "text": "เติมพลังให้คุยต่อแล้ว", "color": BONE_MUTED, "size": "sm", "flex": 0 },
                    { "type": "text", "text": format!("+{}", credits), "color": GOLD, "size": "lg", "weight": "bold", "align": "end" }
                ]}
            ]
        }
    })
}

/// Relationship level-up celebration — appended after the reply (the felt payoff). Portrait carries
/// the warmth; a single gold kicker, no sparkles (spec §B.2).
pub fn build_levelup_flex(
    character_name: &str,
    level_label: &str,
    portrait_url: Option<&str>,
) -> Value {
    let mut bubble = json!({
        "type": "bubble",
        "size": "kilo",
        "body": {
            "type": "box",
            "layout": "vertical",
            "backgroundColor": INK_900,
            "paddingAll": "20px",
            "spacing": "sm",
            "contents": [
                { "type": "text", "text": "✦ ความสัมพันธ์ลึกขึ้น", "color": GOLD, "size": "xs", "weight": "bold" },
                { "type": "text", "text": format!("ตอนนี้คุณเป็น “{}” ของ{}แล้ว", level_label, character_name), "color": BONE, "size": "lg", "weight": "bold", "wrap": true, "margin": "sm" },
                { "type": "text", "text": format!("{}เริ่มไว้ใจคุณมากขึ้น...", character_name), "color": BONE_MUTED, "size": "sm", "style": "italic", "wrap": true, "margin": "sm" }
            ]
        }
    });

    if let Some(url) = portrait_url.filter(|u| !u.is_empty()) {
        bubble["hero"] = json!({
            "type": "image",
            "url": url,
            "size": "full",
            "aspectRatio": "20:13",
            "aspectMode": "cover"
        });
    }

    bubble
}

/// Evening re-engagement reminder — the loop's re-entry (spec §B.5). Reads like the character
/// reaching out; the CTA sends a short opener that resumes the story (and triggers the daily
/// check-in refill). Credits are a footnote, never the headline.
pub fn build_reengagement_flex(character_name: &str, portrait_url: Option<&str>) -> Value {
    let mut bubble = json!({
        "type": "bubble",
        "size": "kilo",
        "body": {
            "type": "box",
            "layout": "vertical",
            "backgroundColor": INK_900,
            "paddingAll": "20px",
            "spacing": "md",
            "contents": [
                { "type": "text", "text": format!("คืนนี้{}ยังนั่งรออยู่ที่เดิม...", character_name), "color": BONE, "size": "md", "style": "italic", "wrap": true },
                { "type": "text", "text": "เรื่องเมื่อวานยังค้างอยู่เลยนะ", "color": BONE_MUTED, "size": "sm", "style": "italic", "wrap": true }
            ]
        },
        "footer": {
            "type": "box",
            "layout": "vertical",
            "backgroundColor": INK_900,
            "paddingAll": "20px",
            "paddingTop": "0px",
            "spacing": "sm",
            "contents": [
                { "type": "button", "style": "primary", "color": LINE_GREEN, "height": "sm",
                  "action": { "type": "message", "label": "คุยต่อ", "text": "กลับมาแล้ว" } },
                { "type": "text", "text": "พลังพร้อมแล้วเมื่อคุณกลับมา", "color": BONE_MUTED, "size": "xs", "align": "center", "wrap": true }
            ]
        }
    });

    if let Some(url) = portrait_url.filter(|u| !u.is_empty()) {
        bubble["hero"] = json!({
            "type": "image",
            "url": url,
            "size": "full",
            "aspectRatio": "20:13",
            "aspectMode": "cover"
        });
    }

    bubble
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pips_fill_and_cap() {
        assert_eq!(streak_pips(0), "○○○○○○○");
        assert_eq!(streak_pips(3), "●●●○○○○");
        assert_eq!(streak_pips(7), "●●●●●●●");
        assert_eq!(streak_pips(99), "●●●●●●●"); // capped at MAX_PIPS
    }

    #[test]
    fn checkin_flex_is_a_bubble() {
        let v = build_daily_checkin_flex("แนน", 3, 6);
        assert_eq!(v["type"], "bubble");
    }

    #[test]
    fn levelup_flex_adds_hero_only_with_portrait() {
        let with = build_levelup_flex("แนน", "คนรู้จัก", Some("https://x/y.png"));
        assert_eq!(with["hero"]["type"], "image");
        let without = build_levelup_flex("แนน", "คนรู้จัก", None);
        assert!(without.get("hero").is_none());
        let empty = build_levelup_flex("แนน", "คนรู้จัก", Some(""));
        assert!(empty.get("hero").is_none());
    }

    #[test]
    fn reengagement_flex_has_footer_button() {
        let v = build_reengagement_flex("แนน", None);
        assert_eq!(v["type"], "bubble");
        assert_eq!(v["footer"]["contents"][0]["type"], "button");
    }
}
