use serde_json::json;

/// Build a Flex Bubble for the welcome message + CTA button (follow event).
pub fn build_welcome_flex(liff_onboarding_url: &str) -> serde_json::Value {
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
                        "label": "เริ่มใช้งาน",
                        "uri": liff_onboarding_url
                    }
                }
            ]
        }
    })
}

/// Build a Flex Bubble for unregistered users telling them to register first.
pub fn build_registration_required_flex(liff_onboarding_url: &str) -> serde_json::Value {
    json!({
        "type": "bubble",
        "body": {
            "type": "box",
            "layout": "vertical",
            "contents": [
                {
                    "type": "text",
                    "text": "กรุณาลงทะเบียนก่อนนะคะ",
                    "weight": "bold",
                    "size": "lg"
                },
                {
                    "type": "text",
                    "text": "คุณยังไม่ได้ยอมรับเงื่อนไขการใช้งาน กดปุ่มด้านล่างเพื่อลงทะเบียนและเริ่มใช้งานค่ะ",
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
                        "label": "ลงทะเบียน",
                        "uri": liff_onboarding_url
                    }
                }
            ]
        }
    })
}
