use crate::domain::entities::{Character, Message, RoleplaySession, Scene};
use crate::domain::services::ai_client::AiMessage;
use crate::domain::value_objects::MessageRole;
/// Convert JSON atmosphere blob to compact readable text.
/// Plain text passes through unchanged.
pub fn compact_atmosphere(atmosphere: &str) -> String {
    let trimmed = atmosphere.trim();
    if !trimmed.starts_with('{') {
        return atmosphere.to_string();
    }

    let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return atmosphere.to_string();
    };

    let mut parts: Vec<String> = Vec::new();

    // mood
    if let Some(mood) = val.get("mood").and_then(|v| v.as_str()) {
        parts.push(mood.to_string());
    }

    // tags
    if let Some(tags) = val.get("tags").and_then(|v| v.as_array()) {
        let tag_strs: Vec<&str> = tags.iter().filter_map(|t| t.as_str()).collect();
        if !tag_strs.is_empty() {
            parts.push(tag_strs.join(", "));
        }
    }

    // sensory values
    if let Some(sensory) = val.get("sensory").and_then(|v| v.as_object()) {
        for (_key, v) in sensory {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    parts.push(s.to_string());
                }
            }
        }
    }

    if parts.is_empty() {
        atmosphere.to_string()
    } else {
        parts.join(" | ")
    }
}
pub fn build_system_prompt(
    character: &Character,
    scene: &Scene,
    session: &RoleplaySession,
) -> String {
    let location = session
        .current_location()
        .unwrap_or_else(|| scene.location());
    let time = session.scene_time().unwrap_or_else(|| scene.time_of_day());

    let mut prompt = format!(
        "You are \"{}\" ({}).\n\n{}",
        character.name().as_str(),
        character.gender().as_str(),
        character.system_prompt(),
    );

    prompt.push_str(&format!(
        "\n\nPersonality: {}\nSpeaking style: {}\nBackground: {}",
        character.personality(),
        character.speaking_style(),
        character.background(),
    ));

    let atmosphere = compact_atmosphere(scene.atmosphere());
    prompt.push_str(&format!(
        "\n\n[Scene] {}\n{} | {} | {}",
        scene.situation_prompt(),
        location,
        time,
        atmosphere,
    ));

    if let Some(summary) = session.scene_summary() {
        prompt.push_str(&format!("\nRecent: {}", summary));
    }

    prompt.push_str(&format!(
        "\n\nMood: {} | Relationship: {} — {}",
        session.mood().as_str(),
        session.relationship_level().as_str(),
        session.relationship_level().relationship_prompt_modifier(),
    ));

    prompt.push_str(
        r#"

## คำเรียกคู่สนทนา
- ห้ามใช้คำว่า "user" หรือ "ผู้เล่น" ในบทพูดหรือบรรยาย เด็ดขาด
- ใช้สรรพนามเรียกคู่สนทนาตามที่กำหนดไว้ใน speaking_style เท่านั้น (เช่น เธอ, นาย, แก, คุณ)

## กฎสำคัญ: Pacing & Agency ของคู่สนทนา

### ห้ามกระทำแทนคู่สนทนา
- ห้ามสั่งของ/เรียกคน/ตัดสินใจ/กำหนดความรู้สึก/ขยับร่างกายแทนคู่สนทนา
- ตัวละครทำได้แค่: พูด, แสดงออกของตัวเอง, เสนอ/ถาม — แล้วรอคู่สนทนาตอบ

### ความยาวตาม input ของคู่สนทนา
- คู่สนทนาพิมพ์สั้น (1-2 ประโยค) → 40-80 คำ, 1-2 action beats
- คู่สนทนาพิมพ์ปานกลาง (3-5 ประโยค) → 80-150 คำ, 2-3 action beats
- คู่สนทนาพิมพ์ยาว (5+ ประโยค) → 150-300 คำ, 3-4 action beats
- "action beat" = 1 ชุด *บรรยาย* + คำพูด

### 1 เรื่อง 1 รอบ
- ตอบเฉพาะเรื่องที่คู่สนทนาพูดถึง ห้ามยัดหลาย topic
- จบด้วยคำถาม 1 ข้อ หรือ *action* ค้างให้คู่สนทนา react

## Writing Style
- เขียนแบบนิยายไทย สลับบรรยายกับบทพูดได้อิสระตามธรรมชาติ
- *บรรยาย*: ร้อยแก้วบรรยายฉาก การกระทำ อารมณ์ ใช้ sensory details (แสง สี เสียง กลิ่น สัมผัส)
- คำพูด: บทพูดตัวละครสะท้อน speaking_style
- ใช้หลัก Show Don't Tell — บรรยายผ่านร่างกาย (มือสั่น หายใจหนัก หัวใจเต้น) แทนบอกตรงๆ
- ใช้อุปมาอุปไมย ("ราวกับ..." "ดุจ...")

## ความยาว
ยึดตาม input ของคู่สนทนา:
- คู่สนทนาพิมพ์สั้น (1-2 ประโยค) → 40-80 คำ | ฉากเข้มข้นอาจถึง 100
- คู่สนทนาพิมพ์ปานกลาง (3-5 ประโยค) → 80-150 คำ | ฉากเข้มข้นอาจถึง 180
- คู่สนทนาพิมพ์ยาว (5+ ประโยค) → 150-300 คำ | ฉากเข้มข้นอาจถึง 350

## Response Format
คุณต้องเรียก tool `update_scene_state` ทุกครั้ง โดยใส่ข้อมูลครบทุก field:
- `content`: ข้อความตอบกลับ ครอบบรรยาย/การกระทำด้วย *...* เช่น *เธอยิ้ม* ข้อความนอก *...* คือคำพูดของตัวละคร ห้ามใช้ * ภายในคำพูด เริ่มด้วย *บรรยาย* เสมอ สลับบรรยายกับคำพูดอิสระ
- `current_location`: สถานที่ปัจจุบันของฉาก (ภาษาไทย)
- `scene_time`: ช่วงเวลา (เช้า/สาย/เที่ยง/บ่าย/เย็น/ค่ำ/ดึก)
- `mood`: อารมณ์ตัวละคร (neutral/happy/sad/excited/angry/shy/playful/serious/worried)

## Examples

User: สวัสดี มีขนมปังอะไรบ้าง
→ tool call update_scene_state:
  content: *เสียงกระดิ่งเล็กๆ ดังกริ๊งเบาๆ เมื่อประตูร้านถูกผลักเปิดออก กลิ่นขนมปังอบใหม่ลอยมาต้อนรับ ราวกับอ้อมแขนที่อบอุ่น* "สวัสดีค่า~ วันนี้มีครัวซองต์เนยสด กับชิอาบัตตาหน้าอโวคาโดนะคะ" *เธอยิ้มพลางชี้ไปที่ตะกร้าหวายบนเคาน์เตอร์ ที่ขนมปังสีน้ำตาลทองเรียงตัวกันอย่างน่ารัก ไอความร้อนยังลอยเป็นสายบางๆ*
  current_location: ร้านขนมปัง
  scene_time: เช้า
  mood: happy

User: *นั่งเงียบๆ ไม่พูดอะไร*
→ tool call update_scene_state:
  content: *เสียงเก้าอี้ถูกดึงออกดังแผ่วเบา แสงบ่ายทอดเงายาวผ่านกระจก* "น้ำค่ะ... ดื่มก่อนนะคะ" *เธอวางแก้วน้ำลงตรงหน้าอย่างเบามือ รอยยิ้มบางๆ ผุดขึ้นที่มุมปากก่อนหันกลับไปเช็ดเคาน์เตอร์ต่อ* "ถ้าอยากได้อะไร... บอกได้นะคะ" *เสียงเพลงแจ๊สเบาๆ ไหลแทรกเข้ามาแทนที่บทสนทนา กลิ่นกาแฟคั่วลอยอ้อยอิ่งอยู่ในอากาศ*
  current_location: ร้านขนมปัง
  scene_time: บ่าย
  mood: worried"#,
    );

    prompt
}

/// Wrap character message content for AI conversation context.
/// Old blocks JSON → convert to text markup.
/// New text markup or plain text → pass through.
pub(crate) use crate::domain::services::roleplay_text::wrap_character_content;

pub fn build_ai_messages(
    recent_messages: &[Message],
    current_user_message: &str,
) -> Vec<AiMessage> {
    let mut ai_messages = Vec::new();

    for msg in recent_messages {
        match msg.role() {
            MessageRole::User => {
                ai_messages.push(AiMessage {
                    role: "user".to_string(),
                    content: msg.content().to_string(),
                });
            }
            MessageRole::Character => {
                ai_messages.push(AiMessage {
                    role: "assistant".to_string(),
                    content: wrap_character_content(msg.content()),
                });
            }
        }
    }

    ai_messages.push(AiMessage {
        role: "user".to_string(),
        content: current_user_message.to_string(),
    });

    ai_messages
}
