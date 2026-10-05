// diesel QueryableByName derive output trips this lint on clippy ≥1.99
#![allow(clippy::redundant_field_names)]

use crate::{
    domain::services::line_client::{LineClient, LineMessage},
    infra::db::postgres_connection::PgPool,
};
use diesel::{
    sql_query,
    sql_types::{Nullable, Text, Timestamptz, Uuid as SqlUuid},
    QueryableByName,
};
use diesel_async::RunQueryDsl;
use std::sync::Arc;
use uuid::Uuid;
#[derive(QueryableByName)]
struct Delivery {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
    #[diesel(sql_type=Text)]
    line_user_id: String,
    #[diesel(sql_type=SqlUuid)]
    delivery_lease_token: Uuid,
    #[diesel(sql_type=Text)]
    content: String,
    #[diesel(sql_type=Text)]
    name: String,
    #[diesel(sql_type=Text)]
    avatar: String,
    #[diesel(sql_type=Nullable<Text>)]
    reply_token: Option<String>,
    #[diesel(sql_type=Timestamptz)]
    created_at: chrono::DateTime<chrono::Utc>,
    #[diesel(sql_type=SqlUuid)]
    session_id: Uuid,
    #[diesel(sql_type=diesel::sql_types::BigInt)]
    web_turns: i64,
    #[diesel(sql_type=Text)]
    location: String,
    #[diesel(sql_type=Text)]
    time_of_day: String,
    #[diesel(sql_type=Text)]
    atmosphere: String,
}
/// One-line LINE notice for web turns completed since LINE's previous message in the same story.
pub fn web_activity_notice(web_turns: i64, web_origin: &str, session_id: Uuid) -> Option<String> {
    (web_turns > 0).then(|| {
        format!(
            "มีการคุยต่อบนเว็บ {web_turns} ข้อความ ดูได้ที่ {}/stories/{session_id}",
            web_origin.trim_end_matches('/')
        )
    })
}
/// Same header as the LIFF opening bubble: the story's latest location and time, falling back to the scene's.
fn reply_bubble(
    content: &str,
    location: &str,
    time_of_day: &str,
    atmosphere: &str,
) -> serde_json::Value {
    let blocks = crate::infra::ai::response::parse_text_into_blocks(content);
    crate::infra::line::roleplay_flex::build_roleplay_blocks_bubble(
        &blocks,
        location,
        time_of_day,
        &crate::infra::line::roleplay_flex::extract_color_tone(atmosphere),
    )
}
pub async fn deliver_pending(
    pool: Arc<PgPool>,
    line: Arc<dyn LineClient>,
    web_origin: String,
) -> anyhow::Result<()> {
    let mut conn = pool.get().await?;
    // Expired final attempts must not remain pending forever after a process crash.
    sql_query("UPDATE jobs SET delivery_status='failed' WHERE origin='line' AND delivery_status='pending' AND delivery_retry_at<now() AND (delivery_attempts>=10 OR delivery_first_attempt_at<now()-interval '23 hours')")
        .execute(&mut conn).await?;
    let rows=sql_query("WITH selected AS (SELECT j.id FROM jobs j JOIN users u ON u.id=j.user_id WHERE j.origin='line' AND j.delivery_status='pending' AND j.delivery_attempts<10 AND (j.delivery_first_attempt_at IS NULL OR j.delivery_first_attempt_at>now()-interval '23 hours') AND (j.delivery_retry_at IS NULL OR j.delivery_retry_at<now()) AND u.status='active' AND u.account_status='active' ORDER BY j.completed_at FOR UPDATE OF j SKIP LOCKED LIMIT 1), claimed AS (UPDATE jobs SET delivery_retry_at=now()+interval '90 seconds',delivery_lease_token=gen_random_uuid(),delivery_first_attempt_at=coalesce(delivery_first_attempt_at,now()),delivery_attempts=delivery_attempts+1 WHERE id IN(SELECT id FROM selected) RETURNING *) SELECT j.id,j.line_user_id,j.delivery_lease_token,j.reply_token,j.created_at,coalesce(j.result->>'content','ขอโทษนะ ระบบยังตอบไม่ได้ และไม่ได้หักเครดิต ลองส่งข้อความใหม่อีกครั้งนะ') AS content,c.name,coalesce(c.avatar_url,'') AS avatar,j.session_id,(SELECT count(*) FROM jobs w WHERE w.session_id=j.session_id AND w.user_id=j.user_id AND w.origin='web' AND w.kind='turn' AND w.status='completed' AND w.completed_at<j.created_at AND w.completed_at>coalesce((SELECT max(p.created_at) FROM jobs p WHERE p.session_id=j.session_id AND p.user_id=j.user_id AND p.origin='line' AND p.id<>j.id AND p.created_at<j.created_at),'-infinity'::timestamptz)) AS web_turns,coalesce(s.current_location,sc.location) AS location,coalesce(s.scene_time,sc.time_of_day) AS time_of_day,sc.atmosphere FROM claimed j JOIN roleplay_sessions s ON s.id=j.session_id JOIN characters c ON c.id=s.character_id JOIN scenes sc ON sc.id=s.scene_id").load::<Delivery>(&mut conn).await?;
    drop(conn);
    for item in rows {
        let contents = reply_bubble(
            &item.content,
            &item.location,
            &item.time_of_day,
            &item.atmosphere,
        );
        let message = LineMessage::Flex {
            alt_text: crate::infra::line::roleplay_flex::truncate_alt_text(&item.content),
            contents,
            sender_name: item.name,
            sender_icon_url: item.avatar,
            quick_reply: None,
        };
        let mut messages = Vec::new();
        if let Some(text) = web_activity_notice(item.web_turns, &web_origin, item.session_id) {
            messages.push(LineMessage::Text {
                text,
                sender_name: String::new(),
                sender_icon_url: String::new(),
            });
        }
        messages.push(message);
        // One claim at a time; the entire network attempt ends before its lease expires.
        let result = tokio::time::timeout(std::time::Duration::from_secs(45), async {
            let reply = if (chrono::Utc::now() - item.created_at).num_seconds() < 50 {
                if let Some(token) = item.reply_token {
                    line.reply_messages(&token, messages.clone()).await.is_ok()
                } else {
                    false
                }
            } else {
                false
            };
            if reply {
                Ok(())
            } else {
                line.push_messages_with_retry_key(&item.line_user_id, messages, item.id)
                    .await
            }
        })
        .await;
        let succeeded = matches!(result, Ok(Ok(())));
        let mut conn = pool.get().await?;
        if succeeded {
            sql_query("UPDATE jobs SET delivery_status='delivered' WHERE id=$1 AND delivery_lease_token=$2 AND delivery_status='pending' AND delivery_retry_at>now()")
                .bind::<SqlUuid, _>(item.id)
                .bind::<SqlUuid, _>(item.delivery_lease_token)
                .execute(&mut conn)
                .await?;
        } else {
            sql_query("UPDATE jobs SET delivery_status=CASE WHEN delivery_attempts>=10 THEN 'failed' ELSE 'pending' END WHERE id=$1 AND delivery_lease_token=$2 AND delivery_status='pending' AND delivery_retry_at>now()").bind::<SqlUuid,_>(item.id).bind::<SqlUuid,_>(item.delivery_lease_token).execute(&mut conn).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_notice_without_web_turns() {
        assert_eq!(
            web_activity_notice(0, "https://app.talkrai.app", Uuid::nil()),
            None
        );
    }
    #[test]
    fn notice_counts_web_turns_and_links_story() {
        let id = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        assert_eq!(
            web_activity_notice(3, "https://app.talkrai.app/", id).as_deref(),
            Some("มีการคุยต่อบนเว็บ 3 ข้อความ ดูได้ที่ https://app.talkrai.app/stories/11111111-2222-3333-4444-555555555555")
        );
    }
    #[test]
    fn bubble_header_shows_story_location_time_and_scene_tone() {
        let bubble = reply_bubble(
            "*ยิ้ม* สวัสดีค่ะ",
            "ร้านหนังสือ 'หน้าถัดไป'",
            "บ่าย",
            r#"{"color_tone":"soft_pink"}"#,
        );
        let header = &bubble["body"]["contents"][1]["contents"];
        assert_eq!(header[0]["text"], "📍 ร้านหนังสือ 'หน้าถัดไป'");
        assert_eq!(header[1]["text"], "🌤️ บ่าย");
        assert_eq!(header[0]["color"], "#F48FB1");
    }
}
