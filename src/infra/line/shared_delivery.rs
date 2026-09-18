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
}
pub async fn deliver_pending(pool: Arc<PgPool>, line: Arc<dyn LineClient>) -> anyhow::Result<()> {
    let mut conn = pool.get().await?;
    // Expired final attempts must not remain pending forever after a process crash.
    sql_query("UPDATE jobs SET delivery_status='failed' WHERE origin='line' AND delivery_status='pending' AND delivery_retry_at<now() AND (delivery_attempts>=10 OR delivery_first_attempt_at<now()-interval '23 hours')")
        .execute(&mut conn).await?;
    let rows=sql_query("WITH selected AS (SELECT j.id FROM jobs j JOIN users u ON u.id=j.user_id JOIN roleplay_sessions s ON s.id=j.session_id WHERE j.origin='line' AND j.delivery_status='pending' AND j.delivery_attempts<10 AND (j.delivery_first_attempt_at IS NULL OR j.delivery_first_attempt_at>now()-interval '23 hours') AND (j.delivery_retry_at IS NULL OR j.delivery_retry_at<now()) AND u.status='active' AND u.account_status='active' AND s.interaction_channel='line' ORDER BY j.completed_at FOR UPDATE OF j SKIP LOCKED LIMIT 1), claimed AS (UPDATE jobs SET delivery_retry_at=now()+interval '90 seconds',delivery_lease_token=gen_random_uuid(),delivery_first_attempt_at=coalesce(delivery_first_attempt_at,now()),delivery_attempts=delivery_attempts+1 WHERE id IN(SELECT id FROM selected) RETURNING *) SELECT j.id,j.line_user_id,j.delivery_lease_token,j.reply_token,j.created_at,coalesce(j.result->>'content','ขอโทษนะ ระบบยังตอบไม่ได้ และไม่ได้หักเครดิต ลองส่งข้อความใหม่อีกครั้งนะ') AS content,c.name,coalesce(c.avatar_url,'') AS avatar FROM claimed j JOIN roleplay_sessions s ON s.id=j.session_id JOIN characters c ON c.id=s.character_id").load::<Delivery>(&mut conn).await?;
    drop(conn);
    for item in rows {
        let blocks = crate::infra::ai::response::parse_text_into_blocks(&item.content);
        let contents = crate::infra::line::roleplay_flex::build_roleplay_blocks_bubble(
            &blocks, "", "", "warm",
        );
        let message = LineMessage::Flex {
            alt_text: crate::infra::line::roleplay_flex::truncate_alt_text(&item.content),
            contents,
            sender_name: item.name,
            sender_icon_url: item.avatar,
            quick_reply: None,
        };
        // One claim at a time; the entire network attempt ends before its lease expires.
        let result = tokio::time::timeout(std::time::Duration::from_secs(45), async {
            let reply = if (chrono::Utc::now() - item.created_at).num_seconds() < 50 {
                if let Some(token) = item.reply_token {
                    line.reply_messages(&token, vec![message.clone()])
                        .await
                        .is_ok()
                } else {
                    false
                }
            } else {
                false
            };
            if reply {
                Ok(())
            } else {
                line.push_messages_with_retry_key(&item.line_user_id, vec![message], item.id)
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
