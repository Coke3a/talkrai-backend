//! Manual provider smoke test. Uses synthetic text only and incurs API usage.
use talkrai_backend::domain::services::ai_client::{
    validate_ai_response, AiClient, AiMessage, AiRoleplayRequest, AiSummaryRequest,
};
use talkrai_backend::infra::ai::openrouter_client::OpenRouterClient;

#[tokio::test]
#[ignore = "requires OPENROUTER_API_KEY and incurs paid API usage"]
async fn qwen_roleplay_and_summary() {
    let _ = dotenvy::dotenv();
    let client = OpenRouterClient::new(
        std::env::var("OPENROUTER_API_KEY").expect("set OPENROUTER_API_KEY"),
        std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| "qwen/qwen3.8-27b".into()),
    );
    let user = AiMessage {
        role: "user".into(),
        content: "สวัสดีครับ คืนนี้ผมแวะมาชมท้องฟ้า คุณอยากรู้จักผมไหม".into(),
    };
    let start = std::time::Instant::now();
    let response = client.generate_roleplay_response(AiRoleplayRequest {
        system_prompt: "คุณคือเซเรน่า แวมไพร์หญิงอายุ 500 ปี บุคลิกเย็นชาแต่เริ่มสนใจผู้มาเยือน ฉากห้องโถงปราสาทตอนดึก ตอบภาษาไทยสั้น 2-3 ประโยค มี *บรรยาย* และบทพูด ไม่เขียนการกระทำแทนผู้เล่น ผู้เล่นชื่อมิน เป็นนักเดินทางที่ชอบดูดาว ใช้ชื่อและรายละเอียดนี้อย่างเป็นธรรมชาติ ส่งคำตอบด้วย update_scene_state".into(),
        messages: vec![user.clone()],
        max_tokens: 1800,
    }).await.expect("OpenRouter roleplay request failed");
    validate_ai_response(&response).expect("invalid roleplay response");
    let text = response.content_text();
    println!("Roleplay ({:.2}s): {text}", start.elapsed().as_secs_f64());
    let summary = client
        .generate_summary(AiSummaryRequest {
            existing_summary: None,
            messages_to_summarize: vec![
                user,
                AiMessage {
                    role: "assistant".into(),
                    content: text,
                },
            ],
            max_tokens: 1800,
        })
        .await
        .expect("OpenRouter summary request failed");
    assert!(!summary.trim().is_empty());
    println!("Summary: {summary}");
}
