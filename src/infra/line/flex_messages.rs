use serde_json::json;

// ===== TalkRai Design System =====
// Primary Coral Rose
const PRIMARY: &str = "#F96D4B";
#[allow(dead_code)]
const PRIMARY_LIGHT: &str = "#FFE8E0";

// Semantic
const SUCCESS: &str = "#18B47A";
const WARNING: &str = "#F5C882";
const ERROR: &str = "#E5542F";
const INFO: &str = "#8B6BF0";

// Neutral Warm Gray
const GRAY_500: &str = "#9B9186";
const GRAY_600: &str = "#7D7368";
#[allow(dead_code)]
const GRAY_700: &str = "#5E564D";
const GRAY_900: &str = "#2A2521";

// Backgrounds
const BG_CREAM: &str = "#FFF8F0";

// Separator
const SEPARATOR_COLOR: &str = "#E8E4DF";

/// Build a Flex Bubble for session started notification.
/// Shows character name, scene name, and avatar as a system-style notification.
pub fn build_session_started_flex(
    character_name: &str,
    scene_name: &str,
    character_avatar_url: Option<&str>,
) -> serde_json::Value {
    let mut contents = Vec::new();

    // Character avatar (if available)
    if let Some(avatar_url) = character_avatar_url {
        if !avatar_url.is_empty() {
            contents.push(json!({
                "type": "image",
                "url": avatar_url,
                "size": "80px",
                "aspectMode": "cover",
                "aspectRatio": "1:1"
            }));
        }
    }

    // Character name
    contents.push(json!({
        "type": "text",
        "text": character_name,
        "weight": "bold",
        "size": "xl",
        "color": PRIMARY,
        "margin": "md"
    }));

    // Scene name
    contents.push(json!({
        "type": "text",
        "text": scene_name,
        "size": "sm",
        "color": GRAY_500,
        "margin": "sm"
    }));

    // Separator
    contents.push(json!({
        "type": "separator",
        "margin": "lg",
        "color": SEPARATOR_COLOR
    }));

    // Status text
    contents.push(json!({
        "type": "text",
        "text": "✨ เรื่องราวเริ่มต้นแล้ว",
        "size": "md",
        "color": SUCCESS,
        "weight": "bold",
        "margin": "lg",
        "align": "center"
    }));

    json!({
        "type": "bubble",
        "styles": {
            "body": {
                "backgroundColor": BG_CREAM
            }
        },
        "body": {
            "type": "box",
            "layout": "vertical",
            "contents": contents,
            "alignItems": "center",
            "paddingAll": "20px"
        }
    })
}

/// Build a Flex Bubble for the welcome message + CTA button (follow event).
pub fn build_welcome_flex(liff_scenes_url: &str) -> serde_json::Value {
    json!({
        "type": "bubble",
        "styles": {
            "body": {
                "backgroundColor": BG_CREAM
            },
            "footer": {
                "backgroundColor": BG_CREAM
            }
        },
        "body": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "box",
                    "layout": "vertical",
                    "height": "3px",
                    "backgroundColor": PRIMARY,
                    "contents": []
                },
                {
                    "type": "text",
                    "text": "TalkRai",
                    "size": "xs",
                    "weight": "bold",
                    "color": PRIMARY,
                    "margin": "lg"
                },
                {
                    "type": "text",
                    "text": "ยินดีต้อนรับ!",
                    "weight": "bold",
                    "size": "xl",
                    "color": GRAY_900,
                    "margin": "sm"
                },
                {
                    "type": "text",
                    "text": "ยินดีต้อนรับสู่ TalkRai นะคะ ✨\nที่นี่คุณสามารถแชทกับตัวละคร AI สุดพิเศษได้แบบเรียลไทม์เลยค่ะ\n\nกดปุ่มด้านล่างเพื่อเริ่มต้นใช้งาน",
                    "wrap": true,
                    "size": "md",
                    "color": GRAY_600,
                    "margin": "md"
                }
            ]
        },
        "footer": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "button",
                    "style": "primary",
                    "color": PRIMARY,
                    "action": {
                        "type": "uri",
                        "label": "เลือกเรื่องที่ชอบเลย!",
                        "uri": liff_scenes_url
                    }
                }
            ]
        }
    })
}

/// Build a Flex Bubble notifying the user that their credits have run out,
/// with a CTA button linking to the credits top-up page.
pub fn build_insufficient_credits_flex(liff_credits_url: &str) -> serde_json::Value {
    json!({
        "type": "bubble",
        "styles": {
            "body": {
                "backgroundColor": BG_CREAM
            },
            "footer": {
                "backgroundColor": BG_CREAM
            }
        },
        "body": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "box",
                    "layout": "vertical",
                    "height": "3px",
                    "backgroundColor": WARNING,
                    "contents": []
                },
                {
                    "type": "text",
                    "text": "เครดิตหมดแล้ว!",
                    "weight": "bold",
                    "size": "lg",
                    "color": ERROR,
                    "margin": "lg"
                },
                {
                    "type": "text",
                    "text": "ข้อความของคุณยังไม่ได้ถูกส่งออกไปนะคะ เพราะเครดิตหมดแล้วค่ะ เติมเครดิตเพื่อแชทกับตัวละครต่อได้เลยนะคะ ✨",
                    "wrap": true,
                    "size": "md",
                    "color": GRAY_600,
                    "margin": "md"
                }
            ]
        },
        "footer": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "button",
                    "style": "primary",
                    "color": PRIMARY,
                    "action": {
                        "type": "uri",
                        "label": "เติมเครดิต",
                        "uri": liff_credits_url
                    }
                }
            ]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_started_flex_with_avatar() {
        let flex = build_session_started_flex(
            "มิโกะ",
            "คาเฟ่ลับแห่งความทรงจำ",
            Some("https://example.com/avatar.png"),
        );
        let body = &flex["body"];
        let contents = body["contents"].as_array().unwrap();

        // image + name + scene + separator + status = 5 elements
        assert_eq!(contents.len(), 5);
        assert_eq!(contents[0]["type"], "image");
        assert_eq!(contents[0]["url"], "https://example.com/avatar.png");
        assert_eq!(contents[1]["text"], "มิโกะ");
        assert_eq!(contents[1]["color"], "#F96D4B");
        assert_eq!(contents[2]["text"], "คาเฟ่ลับแห่งความทรงจำ");
        assert_eq!(contents[4]["text"], "✨ เรื่องราวเริ่มต้นแล้ว");

        // Cream background
        assert_eq!(flex["styles"]["body"]["backgroundColor"], "#FFF8F0");
    }

    #[test]
    fn session_started_flex_without_avatar() {
        let flex = build_session_started_flex("มิโกะ", "คาเฟ่ลับ", None);
        let contents = flex["body"]["contents"].as_array().unwrap();

        // name + scene + separator + status = 4 elements (no image)
        assert_eq!(contents.len(), 4);
        assert_eq!(contents[0]["text"], "มิโกะ");
        assert_eq!(contents[0]["color"], "#F96D4B");
    }

    #[test]
    fn session_started_flex_empty_avatar_url() {
        let flex = build_session_started_flex("มิโกะ", "คาเฟ่ลับ", Some(""));
        let contents = flex["body"]["contents"].as_array().unwrap();

        // Empty URL should be treated same as None
        assert_eq!(contents.len(), 4);
    }

    #[test]
    fn session_ended_flex_with_stats() {
        let flex = build_session_ended_flex(
            "มิโกะ",
            "คาเฟ่ลับแห่งความทรงจำ",
            42,
            "https://liff.line.me/123/scenes",
        );
        let body = &flex["body"];
        let contents = body["contents"].as_array().unwrap();

        // title + subtitle + separator + stats = 4 elements
        assert_eq!(contents.len(), 4);
        assert_eq!(contents[0]["text"], "เรื่องราวจบลงแล้ว");
        assert_eq!(contents[0]["color"], "#9B9186");
        assert_eq!(contents[1]["text"], "มิโกะ - คาเฟ่ลับแห่งความทรงจำ");
        assert_eq!(contents[3]["text"], "ข้อความทั้งหมด: 42");

        // Footer button
        let button = &flex["footer"]["contents"][0];
        assert_eq!(button["action"]["uri"], "https://liff.line.me/123/scenes");
        assert_eq!(button["color"], "#F96D4B");

        // Cream background
        assert_eq!(flex["styles"]["body"]["backgroundColor"], "#FFF8F0");
    }

    #[test]
    fn session_ended_flex_zero_messages() {
        let flex =
            build_session_ended_flex("ยูกิ", "โรงเรียนลับ", 0, "https://liff.line.me/123/scenes");
        let contents = flex["body"]["contents"].as_array().unwrap();
        assert_eq!(contents[3]["text"], "ข้อความทั้งหมด: 0");
    }

    #[test]
    fn welcome_flex_structure() {
        let flex = build_welcome_flex("https://liff.line.me/123/scenes");
        let body_contents = flex["body"]["contents"].as_array().unwrap();

        // accent bar + brand label + title + description = 4 elements
        assert_eq!(body_contents.len(), 4);

        // Accent bar
        assert_eq!(body_contents[0]["height"], "3px");
        assert_eq!(body_contents[0]["backgroundColor"], "#F96D4B");

        // Brand label
        assert_eq!(body_contents[1]["text"], "TalkRai");
        assert_eq!(body_contents[1]["color"], "#F96D4B");

        // Title
        assert_eq!(body_contents[2]["text"], "ยินดีต้อนรับ!");
        assert_eq!(body_contents[2]["color"], "#2A2521");

        // Button color
        let button = &flex["footer"]["contents"][0];
        assert_eq!(button["color"], "#F96D4B");

        // Cream backgrounds
        assert_eq!(flex["styles"]["body"]["backgroundColor"], "#FFF8F0");
        assert_eq!(flex["styles"]["footer"]["backgroundColor"], "#FFF8F0");
    }

    #[test]
    fn insufficient_credits_flex_structure() {
        let flex = build_insufficient_credits_flex("https://liff.line.me/123/credits");
        let body_contents = flex["body"]["contents"].as_array().unwrap();

        // accent bar + title + description = 3 elements
        assert_eq!(body_contents.len(), 3);

        // Warning accent bar
        assert_eq!(body_contents[0]["height"], "3px");
        assert_eq!(body_contents[0]["backgroundColor"], "#F5C882");

        // Title in error color
        assert_eq!(body_contents[1]["text"], "เครดิตหมดแล้ว!");
        assert_eq!(body_contents[1]["color"], "#E5542F");

        // Button color
        let button = &flex["footer"]["contents"][0];
        assert_eq!(button["color"], "#F96D4B");

        // Cream backgrounds
        assert_eq!(flex["styles"]["body"]["backgroundColor"], "#FFF8F0");
        assert_eq!(flex["styles"]["footer"]["backgroundColor"], "#FFF8F0");
    }

    #[test]
    fn registration_required_flex_structure() {
        let flex = build_registration_required_flex("https://liff.line.me/123/scenes");
        let body_contents = flex["body"]["contents"].as_array().unwrap();

        // accent bar + title + description = 3 elements
        assert_eq!(body_contents.len(), 3);

        // Info accent bar
        assert_eq!(body_contents[0]["height"], "3px");
        assert_eq!(body_contents[0]["backgroundColor"], "#8B6BF0");

        // Title color
        assert_eq!(body_contents[1]["text"], "เลือกเรื่องก่อนเริ่มแชทนะคะ");
        assert_eq!(body_contents[1]["color"], "#2A2521");

        // Button color
        let button = &flex["footer"]["contents"][0];
        assert_eq!(button["color"], "#F96D4B");

        // Cream backgrounds
        assert_eq!(flex["styles"]["body"]["backgroundColor"], "#FFF8F0");
        assert_eq!(flex["styles"]["footer"]["backgroundColor"], "#FFF8F0");
    }
}

/// Build a Flex Bubble for session ended notification.
/// Shows character name, scene name, message stats, and a CTA to choose a new scene.
pub fn build_session_ended_flex(
    character_name: &str,
    scene_name: &str,
    message_count: i32,
    liff_scenes_url: &str,
) -> serde_json::Value {
    json!({
        "type": "bubble",
        "styles": {
            "body": {
                "backgroundColor": BG_CREAM
            },
            "footer": {
                "backgroundColor": BG_CREAM
            }
        },
        "body": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "text",
                    "text": "เรื่องราวจบลงแล้ว",
                    "weight": "bold",
                    "size": "xl",
                    "color": GRAY_500,
                    "align": "center"
                },
                {
                    "type": "text",
                    "text": format!("{} - {}", character_name, scene_name),
                    "size": "sm",
                    "color": "#B8AFA5",
                    "align": "center",
                    "margin": "sm"
                },
                {
                    "type": "separator",
                    "margin": "lg",
                    "color": SEPARATOR_COLOR
                },
                {
                    "type": "text",
                    "text": format!("ข้อความทั้งหมด: {}", message_count),
                    "size": "sm",
                    "color": GRAY_600,
                    "margin": "lg",
                    "align": "center"
                }
            ],
            "paddingAll": "20px"
        },
        "footer": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "button",
                    "style": "primary",
                    "color": PRIMARY,
                    "action": {
                        "type": "uri",
                        "label": "เลือกเรื่องใหม่",
                        "uri": liff_scenes_url
                    }
                }
            ]
        }
    })
}

/// Build a Flex Bubble for unregistered users telling them to choose a scene first.
pub fn build_registration_required_flex(liff_scenes_url: &str) -> serde_json::Value {
    json!({
        "type": "bubble",
        "styles": {
            "body": {
                "backgroundColor": BG_CREAM
            },
            "footer": {
                "backgroundColor": BG_CREAM
            }
        },
        "body": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "box",
                    "layout": "vertical",
                    "height": "3px",
                    "backgroundColor": INFO,
                    "contents": []
                },
                {
                    "type": "text",
                    "text": "เลือกเรื่องก่อนเริ่มแชทนะคะ",
                    "weight": "bold",
                    "size": "lg",
                    "color": GRAY_900,
                    "margin": "lg"
                },
                {
                    "type": "text",
                    "text": "กดปุ่มด้านล่างเพื่อเลือกเรื่องที่ชอบแล้วเริ่มแชทกับตัวละครได้เลยค่ะ",
                    "wrap": true,
                    "size": "md",
                    "color": GRAY_600,
                    "margin": "md"
                }
            ]
        },
        "footer": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "button",
                    "style": "primary",
                    "color": PRIMARY,
                    "action": {
                        "type": "uri",
                        "label": "เลือกเรื่องที่ชอบเลย!",
                        "uri": liff_scenes_url
                    }
                }
            ]
        }
    })
}
