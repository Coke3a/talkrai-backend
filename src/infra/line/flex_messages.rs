use serde_json::json;

// ===== TalkRai Design System =====
// Primary Coral Rose
const PRIMARY: &str = "#F96D4B";
#[allow(dead_code)]
const PRIMARY_LIGHT: &str = "#FFE8E0";

// Semantic
#[allow(dead_code)]
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

/// Build a cinematic Flex Bubble for the session opening.
/// Shows a large scene image (hero), character/scene info, and opening narrator text.
pub fn build_session_opening_flex(
    character_name: &str,
    scene_name: &str,
    scene_image_url: Option<&str>,
    opening_narrator: &str,
    color_tone: &str,
) -> serde_json::Value {
    let (accent, gradient_start, gradient_end) =
        super::roleplay_flex::color_tone_to_colors(color_tone);

    let contents = vec![
        // Status label
        json!({
            "type": "text",
            "text": "✨ เรื่องราวเริ่มต้นแล้ว",
            "size": "xs",
            "color": accent,
            "weight": "bold"
        }),
        // Character name
        json!({
            "type": "text",
            "text": character_name,
            "weight": "bold",
            "size": "lg",
            "color": accent,
            "margin": "sm"
        }),
        // Scene name
        json!({
            "type": "text",
            "text": scene_name,
            "size": "sm",
            "color": GRAY_500,
            "margin": "xs"
        }),
        // Accent divider
        json!({
            "type": "box",
            "layout": "vertical",
            "height": "3px",
            "backgroundColor": accent,
            "margin": "lg",
            "contents": []
        }),
        // Opening narrator text
        json!({
            "type": "text",
            "text": opening_narrator,
            "wrap": true,
            "size": "sm",
            "color": "#C8C0B8",
            "margin": "lg"
        }),
    ];

    let mut bubble = json!({
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
    });

    // Add hero image if available
    if let Some(url) = scene_image_url {
        if !url.is_empty() {
            bubble.as_object_mut().unwrap().insert(
                "hero".to_string(),
                json!({
                    "type": "image",
                    "url": url,
                    "size": "full",
                    "aspectRatio": "20:13",
                    "aspectMode": "cover"
                }),
            );
        }
    }

    bubble
}

/// Build a Flex Bubble for the welcome message + CTA button (follow event).
/// When `hero_image_url` is provided, shows a character collage hero image.
pub fn build_welcome_flex(
    liff_scenes_url: &str,
    hero_image_url: Option<&str>,
) -> serde_json::Value {
    let mut bubble = json!({
        "type": "bubble",
        "size": "mega",
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
                    "text": "ยินดีต้อนรับสู่ TalkRai ✨",
                    "weight": "bold",
                    "size": "lg",
                    "color": GRAY_900
                },
                {
                    "type": "text",
                    "text": "กว่า 90 ตัวละครพร้อมให้เธอได้รู้จัก — ใครจะทำให้ใจเธอเต้น?",
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
                        "label": "เลือกตัวละครที่ชอบเลย!",
                        "uri": liff_scenes_url
                    }
                }
            ]
        }
    });

    if let Some(url) = hero_image_url {
        if !url.is_empty() {
            bubble.as_object_mut().unwrap().insert(
                "hero".to_string(),
                json!({
                    "type": "image",
                    "url": url,
                    "size": "full",
                    "aspectRatio": "20:10",
                    "aspectMode": "cover"
                }),
            );
        }
    }

    bubble
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

/// Build a Flex Bubble notifying the user of a temporary system error.
pub fn build_system_error_flex() -> serde_json::Value {
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
                    "text": "ระบบขัดข้องชั่วคราว",
                    "weight": "bold",
                    "size": "lg",
                    "color": ERROR,
                    "margin": "lg"
                },
                {
                    "type": "text",
                    "text": "ขอโทษนะคะ ระบบขัดข้องชั่วคราว ลองส่งข้อความมาใหม่อีกครั้งนะคะ 🙏",
                    "wrap": true,
                    "size": "md",
                    "color": GRAY_600,
                    "margin": "md"
                }
            ]
        }
    })
}

#[cfg(test)]
// `build_session_ended_flex` / `build_registration_required_flex` below predate this test module;
// a newer clippy (items_after_test_module) now flags their position. Suppressed narrowly here
// rather than reordering an unrelated shipping file from a check-in feature branch.
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn session_opening_flex_with_image() {
        let flex = build_session_opening_flex(
            "มิโกะ",
            "คาเฟ่ลับแห่งความทรงจำ",
            Some("https://example.com/scene.png"),
            "ลมพัดเบาๆ ท้องฟ้าเปลี่ยนสี",
            "warm_golden",
        );

        // Hero image
        assert_eq!(flex["hero"]["type"], "image");
        assert_eq!(flex["hero"]["url"], "https://example.com/scene.png");
        assert_eq!(flex["hero"]["size"], "full");
        assert_eq!(flex["hero"]["aspectRatio"], "20:13");
        assert_eq!(flex["hero"]["aspectMode"], "cover");

        // Body contents: label + name + scene + divider + narrator = 5
        let contents = flex["body"]["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 5);
        assert_eq!(contents[0]["text"], "✨ เรื่องราวเริ่มต้นแล้ว");
        assert_eq!(contents[0]["color"], "#FFB74D"); // warm_golden accent
        assert_eq!(contents[1]["text"], "มิโกะ");
        assert_eq!(contents[1]["color"], "#FFB74D");
        assert_eq!(contents[2]["text"], "คาเฟ่ลับแห่งความทรงจำ");
        assert_eq!(contents[2]["color"], "#9B9186");
        assert_eq!(contents[3]["height"], "3px");
        assert_eq!(contents[3]["backgroundColor"], "#FFB74D");
        assert_eq!(contents[4]["text"], "ลมพัดเบาๆ ท้องฟ้าเปลี่ยนสี");
        assert_eq!(contents[4]["color"], "#C8C0B8");

        // Gradient background
        assert_eq!(flex["body"]["background"]["type"], "linearGradient");
        assert_eq!(flex["body"]["background"]["startColor"], "#1a1508");
        assert_eq!(flex["size"], "mega");
    }

    #[test]
    fn session_opening_flex_without_image() {
        let flex = build_session_opening_flex("มิโกะ", "คาเฟ่ลับ", None, "ลมพัดเบาๆ", "cool_blue");

        // No hero
        assert!(flex["hero"].is_null());

        // Body still has 5 elements
        let contents = flex["body"]["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 5);
        assert_eq!(contents[0]["text"], "✨ เรื่องราวเริ่มต้นแล้ว");
        assert_eq!(contents[0]["color"], "#64B5F6"); // cool_blue accent

        // Gradient present
        assert_eq!(flex["body"]["background"]["type"], "linearGradient");
    }

    #[test]
    fn session_opening_flex_empty_image_url() {
        let flex =
            build_session_opening_flex("มิโกะ", "คาเฟ่ลับ", Some(""), "ลมพัดเบาๆ", "warm_golden");

        // Empty URL treated same as None
        assert!(flex["hero"].is_null());
        let contents = flex["body"]["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 5);
    }

    #[test]
    fn session_opening_flex_fallback_color_tone() {
        let flex = build_session_opening_flex("มิโกะ", "คาเฟ่ลับ", None, "ลมพัดเบาๆ", "unknown_tone");

        let contents = flex["body"]["contents"].as_array().unwrap();
        // Fallback accent color
        assert_eq!(contents[0]["color"], "#A0A0A0");
        assert_eq!(contents[1]["color"], "#A0A0A0");

        // Fallback gradient
        assert_eq!(flex["body"]["background"]["startColor"], "#171513");
        assert_eq!(flex["body"]["background"]["endColor"], "#1c1a18");
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
    fn welcome_flex_with_hero() {
        let flex = build_welcome_flex(
            "https://liff.line.me/123/scenes",
            Some("https://example.com/hero.png"),
        );

        // Bubble size
        assert_eq!(flex["size"], "mega");

        // Hero image
        assert_eq!(flex["hero"]["type"], "image");
        assert_eq!(flex["hero"]["url"], "https://example.com/hero.png");
        assert_eq!(flex["hero"]["size"], "full");
        assert_eq!(flex["hero"]["aspectRatio"], "20:10");
        assert_eq!(flex["hero"]["aspectMode"], "cover");

        // Body contents: title + body text = 2 elements
        let body_contents = flex["body"]["contents"].as_array().unwrap();
        assert_eq!(body_contents.len(), 2);

        // Title
        assert_eq!(body_contents[0]["text"], "ยินดีต้อนรับสู่ TalkRai ✨");
        assert_eq!(body_contents[0]["weight"], "bold");
        assert_eq!(body_contents[0]["size"], "lg");
        assert_eq!(body_contents[0]["color"], "#2A2521");

        // Body text
        assert!(body_contents[1]["text"]
            .as_str()
            .unwrap()
            .contains("90 ตัวละคร"));
        assert_eq!(body_contents[1]["color"], "#7D7368");
        assert_eq!(body_contents[1]["wrap"], true);

        // CTA button label
        let button = &flex["footer"]["contents"][0];
        assert_eq!(button["action"]["label"], "เลือกตัวละครที่ชอบเลย!");
        assert_eq!(button["color"], "#F96D4B");

        // Cream backgrounds
        assert_eq!(flex["styles"]["body"]["backgroundColor"], "#FFF8F0");
        assert_eq!(flex["styles"]["footer"]["backgroundColor"], "#FFF8F0");
    }

    #[test]
    fn welcome_flex_without_hero() {
        let flex = build_welcome_flex("https://liff.line.me/123/scenes", None);

        // No hero when URL is None
        assert!(flex["hero"].is_null());

        // Still has body and footer with new wording
        let body_contents = flex["body"]["contents"].as_array().unwrap();
        assert_eq!(body_contents[0]["text"], "ยินดีต้อนรับสู่ TalkRai ✨");

        let button = &flex["footer"]["contents"][0];
        assert_eq!(button["action"]["label"], "เลือกตัวละครที่ชอบเลย!");
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
