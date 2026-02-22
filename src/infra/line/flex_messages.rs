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
                },
                {
                    "type": "text",
                    "text": "TODO: หน้า Accept Terms ยังไม่ได้ implement กรุณาใช้ simulation_webhook_test script แทน",
                    "wrap": true,
                    "margin": "md",
                    "size": "xs",
                    "color": "#999999"
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
