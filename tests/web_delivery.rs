use async_trait::async_trait;
use diesel::{
    sql_query,
    sql_types::{Text, Uuid as SqlUuid},
    QueryableByName,
};
use diesel_async::RunQueryDsl;
use std::sync::{Arc, Mutex};
use talkrai_backend::{
    domain::services::{
        line_client::{LineClient, LineMessage, LineProfile},
        line_client_error::LineClientError,
    },
    infra::{db::postgres_connection::create_pool, line::shared_delivery::deliver_pending},
};
use tokio::sync::Semaphore;
use uuid::Uuid;

struct ControlledLine {
    keys: Mutex<Vec<Uuid>>,
    entered: Semaphore,
    release: Semaphore,
}
#[async_trait]
impl LineClient for ControlledLine {
    fn verify_signature(&self, _: &[u8], _: &str) -> Result<bool, LineClientError> {
        unreachable!()
    }
    async fn push_messages(&self, _: &str, _: Vec<LineMessage>) -> Result<(), LineClientError> {
        panic!("outbox must use its persisted retry key")
    }
    async fn push_messages_with_retry_key(
        &self,
        _: &str,
        _: Vec<LineMessage>,
        key: Uuid,
    ) -> Result<(), LineClientError> {
        self.keys.lock().unwrap().push(key);
        self.entered.add_permits(1);
        self.release.acquire().await.unwrap().forget();
        Ok(())
    }
    async fn reply_messages(&self, _: &str, _: Vec<LineMessage>) -> Result<(), LineClientError> {
        unreachable!()
    }
    async fn get_profile(&self, _: &str) -> Result<LineProfile, LineClientError> {
        unreachable!()
    }
    async fn link_rich_menu(&self, _: &str, _: &str) -> Result<(), LineClientError> {
        unreachable!()
    }
    async fn unlink_rich_menu(&self, _: &str) -> Result<(), LineClientError> {
        unreachable!()
    }
    async fn show_loading_animation(&self, _: &str, _: Option<u32>) -> Result<(), LineClientError> {
        unreachable!()
    }
    async fn verify_liff_token(&self, _: &str) -> Result<LineProfile, LineClientError> {
        unreachable!()
    }
}
#[derive(QueryableByName)]
struct Status {
    #[diesel(sql_type=Text)]
    delivery_status: String,
}

#[tokio::test]
#[ignore = "requires isolated migrated PostgreSQL via TEST_DATABASE_URL"]
async fn outbox_fences_stale_workers_and_reuses_retry_key() {
    let pool = Arc::new(create_pool(&std::env::var("TEST_DATABASE_URL").unwrap(), 4).unwrap());
    talkrai_backend::infra::db::schema_check::ensure_ready(&pool)
        .await
        .unwrap();
    let mut conn = pool.get().await.unwrap();
    let owner = Uuid::new_v4();
    let character = Uuid::new_v4();
    let scene = Uuid::new_v4();
    let story = Uuid::new_v4();
    let job = Uuid::new_v4();
    sql_query("INSERT INTO users(id,line_user_id,display_name,status) VALUES($1,$1::text,'Delivery test','active')").bind::<SqlUuid,_>(owner).execute(&mut conn).await.unwrap();
    sql_query("INSERT INTO characters(id,name,personality,speaking_style,background,system_prompt,gender) VALUES($1,'Test','Test','Test','Test','Test','female')").bind::<SqlUuid,_>(character).execute(&mut conn).await.unwrap();
    sql_query("INSERT INTO scenes(id,character_id,name,location,time_of_day,atmosphere,situation_prompt,opening_narrator,opening_dialogue) VALUES($1,$2,'Test','Test','Test','Test','Test','Test','Test')").bind::<SqlUuid,_>(scene).bind::<SqlUuid,_>(character).execute(&mut conn).await.unwrap();
    sql_query(
        "INSERT INTO roleplay_sessions(id,user_id,character_id,scene_id) VALUES($1,$2,$3,$4)",
    )
    .bind::<SqlUuid, _>(story)
    .bind::<SqlUuid, _>(owner)
    .bind::<SqlUuid, _>(character)
    .bind::<SqlUuid, _>(scene)
    .execute(&mut conn)
    .await
    .unwrap();
    sql_query("INSERT INTO jobs(id,session_id,user_id,line_user_id,user_message,origin,status,delivery_status,completed_at,result) VALUES($1,$2,$3,$3::text,'hello','line','completed','pending',now(),'{\"content\":\"Hello\"}')").bind::<SqlUuid,_>(job).bind::<SqlUuid,_>(story).bind::<SqlUuid,_>(owner).execute(&mut conn).await.unwrap();
    drop(conn);
    let line = Arc::new(ControlledLine {
        keys: Mutex::new(vec![]),
        entered: Semaphore::new(0),
        release: Semaphore::new(0),
    });
    let first = tokio::spawn(deliver_pending(pool.clone(), line.clone()));
    tokio::time::timeout(std::time::Duration::from_secs(5), line.entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    // Another worker cannot take the in-flight job.
    deliver_pending(pool.clone(), line.clone()).await.unwrap();
    assert_eq!(*line.keys.lock().unwrap(), vec![job]);
    let mut conn = pool.get().await.unwrap();
    sql_query("UPDATE jobs SET delivery_retry_at=now()-interval '1 second' WHERE id=$1")
        .bind::<SqlUuid, _>(job)
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    let second = tokio::spawn(deliver_pending(pool.clone(), line.clone()));
    tokio::time::timeout(std::time::Duration::from_secs(5), line.entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    // The old worker finishes after a replacement has claimed the job.
    line.release.add_permits(1);
    first.await.unwrap().unwrap();
    let mut conn = pool.get().await.unwrap();
    let status = sql_query("SELECT delivery_status FROM jobs WHERE id=$1")
        .bind::<SqlUuid, _>(job)
        .get_result::<Status>(&mut conn)
        .await
        .unwrap();
    assert_eq!(
        status.delivery_status, "pending",
        "stale completion cannot acknowledge the replacement lease"
    );
    drop(conn);
    line.release.add_permits(1);
    second.await.unwrap().unwrap();
    assert_eq!(*line.keys.lock().unwrap(), vec![job, job]);
    let mut conn = pool.get().await.unwrap();
    let status = sql_query("SELECT delivery_status FROM jobs WHERE id=$1")
        .bind::<SqlUuid, _>(job)
        .get_result::<Status>(&mut conn)
        .await
        .unwrap();
    assert_eq!(status.delivery_status, "delivered");
    // A process crash on the last claim must become terminal after lease expiry.
    sql_query("UPDATE jobs SET delivery_status='pending',delivery_attempts=10,delivery_retry_at=now()-interval '1 second' WHERE id=$1").bind::<SqlUuid,_>(job).execute(&mut conn).await.unwrap();
    drop(conn);
    deliver_pending(pool.clone(), line).await.unwrap();
    let mut conn = pool.get().await.unwrap();
    let status = sql_query("SELECT delivery_status FROM jobs WHERE id=$1")
        .bind::<SqlUuid, _>(job)
        .get_result::<Status>(&mut conn)
        .await
        .unwrap();
    assert_eq!(status.delivery_status, "failed");
}
