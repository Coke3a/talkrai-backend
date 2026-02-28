use serde_json::json;

/// Build a Flex Bubble for the welcome message + CTA button (follow event).
pub fn build_welcome_flex(liff_scenes_url: &str) -> serde_json::Value {
    json!({
        "type": "bubble",
        "body": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "text",
                    "text": "ยินดีต้อนรับ!",
                    "weight": "bold",
                    "size": "xl"
                },
                {
                    "type": "text",
                    "text": "ยินดีต้อนรับสู่ Talk a LINE นะคะ ✨\nที่นี่คุณสามารถแชทกับตัวละคร AI สุดพิเศษได้แบบเรียลไทม์เลยค่ะ\n\nกดปุ่มด้านล่างเพื่อเริ่มต้นใช้งาน",
                    "wrap": true,
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
        "body": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "text",
                    "text": "เครดิตหมดแล้ว!",
                    "weight": "bold",
                    "size": "lg"
                },
                {
                    "type": "text",
                    "text": "ข้อความของคุณยังไม่ได้ถูกส่งออกไปนะคะ เพราะเครดิตหมดแล้วค่ะ เติมเครดิตเพื่อแชทกับตัวละครต่อได้เลยนะคะ ✨",
                    "wrap": true,
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

/// Build a Flex Bubble for unregistered users telling them to choose a scene first.
pub fn build_registration_required_flex(liff_scenes_url: &str) -> serde_json::Value {
    json!({
        "type": "bubble",
        "body": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "text",
                    "text": "เลือกเรื่องก่อนเริ่มแชทนะคะ",
                    "weight": "bold",
                    "size": "lg"
                },
                {
                    "type": "text",
                    "text": "กดปุ่มด้านล่างเพื่อเลือกเรื่องที่ชอบแล้วเริ่มแชทกับตัวละครได้เลยค่ะ",
                    "wrap": true,
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
