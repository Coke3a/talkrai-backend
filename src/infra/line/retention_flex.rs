//! Flex builders for the retention loop (spec 2026-06-24 §B.2/§B.3) — Midnight Theatre **dark**
//! style (hex approximations of the OKLCH tokens, since LINE Flex needs hex). Distinct from the
//! older cream cards in `flex_messages.rs`. No floating hearts/sparkles (PRODUCT.md ban).

use serde_json::{json, Value};

// Hex approximations of the DESIGN.md OKLCH tokens.
const INK_900: &str = "#1C1118"; // page ground
const INK_700: &str = "#473540"; // hairline / locked cell
const BONE: &str = "#EFE7DC"; // text
const BONE_MUTED: &str = "#A79D90"; // muted text
const BONE_FAINT: &str = "#857B6F"; // faint text — recedes below muted (reset-time line)
const GOLD: &str = "#D8B15A"; // champagne accent / numerals / claimed + today
const ROSE: &str = "#E0708A"; // ember rose — the weekly chest
const LINE_GREEN: &str = "#06C755"; // LINE brand green — the one functional CTA

const WEEK_LEN: i32 = 7;

/// One cell of the weekly check-in calendar: numeral label, state glyph, amount. `day` is 1..=7;
/// `cycle_day` is today's position in the week. Day 7 is the chest (rose) even when still locked —
/// it is the visible goal.
fn checkin_cell(day: i32, cycle_day: i32, amount: i32) -> Value {
    let is_chest = day == WEEK_LEN;
    let is_today = day == cycle_day;
    let is_claimed = day < cycle_day;

    let (glyph, glyph_color) = if is_chest {
        ("🎁", ROSE)
    } else if is_today {
        ("◆", GOLD)
    } else if is_claimed {
        ("✓", GOLD)
    } else {
        ("·", INK_700)
    };

    let label_color = if is_today { GOLD } else { BONE_MUTED };
    let amount_color = if is_chest {
        ROSE
    } else if is_claimed || is_today {
        GOLD
    } else {
        BONE_MUTED
    };

    json!({
        "type": "box",
        "layout": "vertical",
        "flex": 1,
        "spacing": "xs",
        "alignItems": "center",
        "contents": [
            { "type": "text", "text": day.to_string(), "size": "xs", "color": label_color, "weight": if is_today { "bold" } else { "regular" }, "align": "center" },
            { "type": "text", "text": glyph, "size": "sm", "color": glyph_color, "align": "center" },
            { "type": "text", "text": format!("+{}", amount), "size": "xs", "color": amount_color, "align": "center" }
        ]
    })
}

/// Daily check-in card — the weekly cycle calendar (spec R3.2/R3.3). Rendered by the SYSTEM
/// (TalkRai OA), not the character: the copy names the feature outright ("เช็คอินรายวัน รับเครดิตฟรี")
/// and carries no character voice, so the user reads it as a system reward, not the character speaking.
/// Sells anticipation (the chest is in sight) + loss aversion (a missed day resets to day 1). The
/// mandatory "checked in today" and midnight-reset lines always appear on a fresh check-in.
/// `weekly_credits` is the 7-entry table from the usecase grant.
pub fn build_daily_checkin_flex(
    weekly_credits: &[i32],
    new_streak: i32,
    credits_awarded: i32,
) -> Value {
    let cycle_day = (new_streak - 1).rem_euclid(WEEK_LEN) + 1;
    let days_to_chest = WEEK_LEN - cycle_day;
    let chest_credits = weekly_credits.get(6).copied().unwrap_or(10);

    let cells: Vec<Value> = (0..WEEK_LEN)
        .map(|i| {
            let amount = weekly_credits.get(i as usize).copied().unwrap_or(0);
            checkin_cell(i + 1, cycle_day, amount)
        })
        .collect();

    let countdown = if days_to_chest == 0 {
        "เปิดกล่องวันนี้แล้ว 🎁".to_string()
    } else {
        format!("อีก {} วันเปิดกล่อง {} เครดิต", days_to_chest, chest_credits)
    };

    json!({
        "type": "bubble",
        "size": "giga",
        "body": {
            "type": "box",
            "layout": "vertical",
            "backgroundColor": INK_900,
            "paddingAll": "20px",
            "spacing": "sm",
            "contents": [
                { "type": "text", "text": "เช็คอินรายวัน รับเครดิตฟรี", "color": BONE, "size": "lg", "weight": "bold", "wrap": true },
                { "type": "text", "text": "ยิ่งต่อเนื่องหลายวัน ยิ่งได้เครดิตเยอะ", "color": BONE_MUTED, "size": "sm", "wrap": true },
                { "type": "separator", "margin": "lg", "color": INK_700 },
                { "type": "box", "layout": "horizontal", "margin": "lg", "spacing": "xs", "contents": cells },
                { "type": "separator", "margin": "lg", "color": INK_700 },
                { "type": "box", "layout": "baseline", "margin": "lg", "spacing": "sm", "contents": [
                    { "type": "text", "text": "✓ เช็คอินวันนี้แล้ว", "color": BONE, "size": "sm", "flex": 0 },
                    { "type": "text", "text": format!("+{}", credits_awarded), "color": GOLD, "size": "sm", "weight": "bold", "align": "end" }
                ]},
                { "type": "text", "text": countdown, "color": BONE_MUTED, "size": "xs", "margin": "sm", "wrap": true },
                { "type": "text", "text": "เริ่มรอบใหม่เที่ยงคืน", "color": BONE_FAINT, "size": "xs", "wrap": true }
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

    const WEEKLY: [i32; 7] = [2, 3, 4, 4, 4, 4, 10];

    #[test]
    fn checkin_flex_is_a_giga_bubble() {
        let v = build_daily_checkin_flex(&WEEKLY, 3, 4);
        assert_eq!(v["type"], "bubble");
        assert_eq!(v["size"], "giga");
    }

    #[test]
    fn checkin_flex_has_mandatory_today_and_reset_lines() {
        let v = build_daily_checkin_flex(&WEEKLY, 3, 4);
        let rendered = serde_json::to_string(&v).expect("serialize");
        assert!(rendered.contains("เช็คอินวันนี้แล้ว"));
        assert!(rendered.contains("เริ่มรอบใหม่เที่ยงคืน"));
    }

    #[test]
    fn checkin_flex_marks_today_cell() {
        // new_streak = 3 → cycle-day 3 → cell index 2 is today (◆ gold).
        let v = build_daily_checkin_flex(&WEEKLY, 3, 4);
        let cells = &v["body"]["contents"][3]["contents"];
        assert_eq!(cells[2]["contents"][1]["text"], "◆");
        assert_eq!(cells[2]["contents"][1]["color"], GOLD);
        // earlier day is claimed (✓), later day is locked (·).
        assert_eq!(cells[0]["contents"][1]["text"], "✓");
        assert_eq!(cells[3]["contents"][1]["text"], "·");
    }

    #[test]
    fn checkin_flex_chest_cell_is_rose_with_amount() {
        let v = build_daily_checkin_flex(&WEEKLY, 3, 4);
        let chest = &v["body"]["contents"][3]["contents"][6];
        assert_eq!(chest["contents"][1]["text"], "🎁");
        assert_eq!(chest["contents"][1]["color"], ROSE);
        assert_eq!(chest["contents"][2]["text"], "+10");
        assert_eq!(chest["contents"][2]["color"], ROSE);
    }

    #[test]
    fn checkin_flex_chest_day_shows_open_today_copy() {
        // new_streak = 7 → cycle-day 7 → days_to_chest 0 → the "open today" copy, not "อีก 0 วัน".
        let v = build_daily_checkin_flex(&WEEKLY, 7, 10);
        let rendered = serde_json::to_string(&v).expect("serialize");
        assert!(rendered.contains("เปิดกล่องวันนี้แล้ว"));
        assert!(!rendered.contains("อีก 0 วัน"));
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
