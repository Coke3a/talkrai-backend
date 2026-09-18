use diesel::{
    sql_query,
    sql_types::{Jsonb, Uuid as SqlUuid},
    QueryableByName,
};
use diesel_async::RunQueryDsl;
use serde_json::{json, Value};
use std::sync::Arc;
use talkrai_backend::{
    domain::web::TurnRepository,
    infra::db::{postgres_connection::create_pool, repositories::turn_postgres::TurnPostgres},
};
use uuid::Uuid;
#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type=Jsonb)]
    value: Value,
}
#[derive(Default)]
struct TestAi {
    requests:
        std::sync::Mutex<Vec<talkrai_backend::domain::services::ai_client::AiRoleplayRequest>>,
    fail: std::sync::atomic::AtomicBool,
}
#[async_trait::async_trait]
impl talkrai_backend::domain::services::ai_client::AiClient for TestAi {
    async fn generate_roleplay_response(
        &self,
        request: talkrai_backend::domain::services::ai_client::AiRoleplayRequest,
    ) -> Result<
        talkrai_backend::domain::services::ai_client::AiRoleplayResponse,
        talkrai_backend::domain::services::ai_client_error::AiClientError,
    > {
        use talkrai_backend::domain::services::{ai_client::*, ai_client_error::AiClientError};
        self.requests.lock().unwrap().push(request);
        if std::sync::atomic::AtomicBool::load(&self.fail, std::sync::atomic::Ordering::Relaxed) {
            return Err(AiClientError::RateLimited);
        }
        Ok(AiRoleplayResponse {
            blocks: vec![ResponseBlock {
                block_type: BlockType::Dialogue,
                text: "คำตอบทดสอบจากตัวละครในเรื่องของเรา".into(),
            }],
            mood: Some("happy".into()),
            current_location: None,
            scene_time: None,
        })
    }
    async fn generate_summary(
        &self,
        _: talkrai_backend::domain::services::ai_client::AiSummaryRequest,
    ) -> Result<String, talkrai_backend::domain::services::ai_client_error::AiClientError> {
        Ok("สรุปทดสอบ".into())
    }
}
#[tokio::test]
#[ignore = "requires isolated migrated PostgreSQL via TEST_DATABASE_URL"]
async fn simultaneous_turns_and_payment_callbacks_settle_once() {
    let pool = Arc::new(create_pool(&std::env::var("TEST_DATABASE_URL").unwrap(), 8).unwrap());
    let owner = Uuid::new_v4();
    let character = Uuid::new_v4();
    let scene = Uuid::new_v4();
    let story = Uuid::new_v4();
    let mut conn = pool.get().await.unwrap();
    sql_query("INSERT INTO users(id,line_user_id,display_name,terms_accepted_at,last_check_in_on) VALUES($1,null,'Concurrency test',now(),(now() AT TIME ZONE 'Asia/Bangkok')::date)").bind::<SqlUuid,_>(owner).execute(&mut conn).await.unwrap();
    sql_query("INSERT INTO credit_balances(user_id,balance) VALUES($1,2)")
        .bind::<SqlUuid, _>(owner)
        .execute(&mut conn)
        .await
        .unwrap();
    sql_query("INSERT INTO characters(id,name,personality,speaking_style,background,system_prompt,gender) VALUES($1,'Test','Test','Test','Test','Secret','female')").bind::<SqlUuid,_>(character).execute(&mut conn).await.unwrap();
    sql_query("INSERT INTO scenes(id,character_id,name,location,time_of_day,atmosphere,situation_prompt,opening_narrator,opening_dialogue) VALUES($1,$2,'Test','Test','Test','Test','Secret','Test','Test')").bind::<SqlUuid,_>(scene).bind::<SqlUuid,_>(character).execute(&mut conn).await.unwrap();
    sql_query("INSERT INTO roleplay_sessions(id,user_id,character_id,scene_id,interaction_channel) VALUES($1,$2,$3,$4,'web')").bind::<SqlUuid,_>(story).bind::<SqlUuid,_>(owner).bind::<SqlUuid,_>(character).bind::<SqlUuid,_>(scene).execute(&mut conn).await.unwrap();
    drop(conn);
    let turns = Arc::new(TurnPostgres { pool: pool.clone() });
    let barrier = Arc::new(tokio::sync::Barrier::new(8));
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let turns = turns.clone();
        let barrier = barrier.clone();
        tasks.spawn(async move {
            barrier.wait().await;
            turns
                .admit(
                    owner,
                    story,
                    "web",
                    "turn",
                    "same-request",
                    json!({"content":"hello","expected_version":0}),
                )
                .await
                .unwrap()
        });
    }
    let mut ids = std::collections::HashSet::new();
    while let Some(result) = tasks.join_next().await {
        ids.insert(result.unwrap()["id"].as_str().unwrap().to_owned());
    }
    assert_eq!(ids.len(), 1);
    let job = ids.into_iter().next().unwrap().parse::<Uuid>().unwrap();
    let claim = turns.claim(job).await.unwrap().unwrap();
    let lease = serde_json::from_value(claim["lease_token"].clone()).unwrap();
    let response = json!({"content":"*ยิ้ม* สวัสดี","mood":"happy"});
    let (a, b) = tokio::join!(
        turns.settle(job, lease, response.clone()),
        turns.settle(job, lease, response)
    );
    assert_eq!(a.unwrap(), b.unwrap());
    let mut conn = pool.get().await.unwrap();
    let row=sql_query("SELECT jsonb_build_object('balance',balance,'reserved',reserved,'debits',(SELECT count(*) FROM credit_transactions WHERE reference_id=$2)) AS value FROM credit_balances WHERE user_id=$1").bind::<SqlUuid,_>(owner).bind::<SqlUuid,_>(job).get_result::<Row>(&mut conn).await.unwrap();
    assert_eq!(row.value, json!({"balance":0,"reserved":0,"debits":1}));
    let order = Uuid::new_v4();
    sql_query("INSERT INTO payment_orders(id,user_id,beam_payment_link_id,package_id,credits_amount,price_thb) VALUES($1,$2,'','basic',50,29)").bind::<SqlUuid,_>(order).bind::<SqlUuid,_>(owner).execute(&mut conn).await.unwrap();
    drop(conn);
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let pool = pool.clone();
        tasks.spawn(async move {
            let mut conn = pool.get().await.unwrap();
            sql_query("SELECT settle_payment($1,'PAID')")
                .bind::<SqlUuid, _>(order)
                .execute(&mut conn)
                .await
                .unwrap();
        });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }
    let mut conn = pool.get().await.unwrap();
    let row=sql_query("SELECT jsonb_build_object('balance',balance,'purchases',(SELECT count(*) FROM credit_transactions WHERE reference_id=$2)) AS value FROM credit_balances WHERE user_id=$1").bind::<SqlUuid,_>(owner).bind::<SqlUuid,_>(order).get_result::<Row>(&mut conn).await.unwrap();
    assert_eq!(row.value, json!({"balance":50,"purchases":1}));
    drop(conn);
    use talkrai_backend::infra::db::repositories::{
        app_config_postgres::AppConfigPostgres, character_postgres::CharacterPostgres,
        message_postgres::MessagePostgres, roleplay_session_postgres::RoleplaySessionPostgres,
        scene_postgres::ScenePostgres,
    };
    let ai = Arc::new(TestAi::default());
    let engine = talkrai_backend::usecases::roleplay::generate::GenerateTurn {
        turns: turns.clone(),
        sessions: Arc::new(RoleplaySessionPostgres::new(pool.clone())),
        characters: Arc::new(CharacterPostgres::new(pool.clone())),
        scenes: Arc::new(ScenePostgres::new(pool.clone())),
        messages: Arc::new(MessagePostgres::new(pool.clone())),
        config: Arc::new(AppConfigPostgres::new(pool.clone())),
        ai: ai.clone(),
    };
    let accepted = turns
        .admit(
            owner,
            story,
            "web",
            "turn",
            "engine-turn",
            json!({"content":"เล่าเรื่องของวันนี้ให้ฟังหน่อย","expected_version":1}),
        )
        .await
        .unwrap();
    let engine_job: Uuid = serde_json::from_value(accepted["id"].clone()).unwrap();
    talkrai_backend::usecases::roleplay::generate::GenerateTurn::execute(&engine, engine_job)
        .await
        .unwrap();
    talkrai_backend::usecases::roleplay::generate::GenerateTurn::execute(&engine, engine_job)
        .await
        .unwrap();
    assert_eq!(
        ai.requests.lock().unwrap().len(),
        1,
        "completed replay must not call AI"
    );
    let mut conn = pool.get().await.unwrap();
    let settled = sql_query("SELECT result AS value FROM jobs WHERE id=$1")
        .bind::<SqlUuid, _>(engine_job)
        .get_result::<Row>(&mut conn)
        .await
        .unwrap();
    drop(conn);
    let regen = turns
        .admit(
            owner,
            story,
            "web",
            "regeneration",
            "engine-regen",
            json!({"message_id":settled.value["message_id"],"expected_version":2}),
        )
        .await
        .unwrap();
    talkrai_backend::usecases::roleplay::generate::GenerateTurn::execute(
        &engine,
        serde_json::from_value(regen["id"].clone()).unwrap(),
    )
    .await
    .unwrap();
    {
        let requests = ai.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0].messages.last().unwrap().content,
            requests[1].messages.last().unwrap().content,
            "regeneration repeats original user input"
        );
        assert!(
            !requests[1]
                .messages
                .iter()
                .any(|m| m.content.contains("คำตอบทดสอบจากตัวละคร")),
            "regeneration cannot see rejected answer"
        );
    }
    ai.fail.store(true, std::sync::atomic::Ordering::Relaxed);
    let failed = turns
        .admit(
            owner,
            story,
            "web",
            "turn",
            "engine-failed",
            json!({"content":"ข้อความที่ AI ตอบไม่ได้","expected_version":3}),
        )
        .await
        .unwrap();
    assert!(
        talkrai_backend::usecases::roleplay::generate::GenerateTurn::execute(
            &engine,
            serde_json::from_value(failed["id"].clone()).unwrap()
        )
        .await
        .is_err()
    );
    let mut conn = pool.get().await.unwrap();
    let state=sql_query("SELECT jsonb_build_object('balance',balance,'reserved',reserved,'count',(SELECT message_count FROM roleplay_sessions WHERE id=$2)) AS value FROM credit_balances WHERE user_id=$1").bind::<SqlUuid,_>(owner).bind::<SqlUuid,_>(story).get_result::<Row>(&mut conn).await.unwrap();
    assert_eq!(state.value, json!({"balance":46,"reserved":0,"count":2}));
    let line_story = Uuid::new_v4();
    sql_query("INSERT INTO roleplay_sessions(id,user_id,character_id,scene_id,interaction_channel) VALUES($1,$2,$3,$4,'line')").bind::<SqlUuid,_>(line_story).bind::<SqlUuid,_>(owner).bind::<SqlUuid,_>(character).bind::<SqlUuid,_>(scene).execute(&mut conn).await.unwrap();
    sql_query("UPDATE users SET last_check_in_on=null,check_in_streak=0 WHERE id=$1")
        .bind::<SqlUuid, _>(owner)
        .execute(&mut conn)
        .await
        .unwrap();
    sql_query("UPDATE credit_balances SET balance=0 WHERE user_id=$1")
        .bind::<SqlUuid, _>(owner)
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    let (web, line) = tokio::join!(
        turns.admit(
            owner,
            story,
            "web",
            "turn",
            "cross-web",
            json!({"content":"hello","expected_version":3})
        ),
        turns.admit(
            owner,
            line_story,
            "line",
            "turn",
            "cross-line",
            json!({"content":"hello"})
        )
    );
    assert_ne!(
        web.is_ok(),
        line.is_ok(),
        "only one channel can reserve the final credits"
    );
    let mut conn = pool.get().await.unwrap();
    let state=sql_query("SELECT jsonb_build_object('balance',balance,'reserved',reserved,'grants',(SELECT count(*) FROM credit_grants WHERE user_id=$1 AND grant_kind='daily')) AS value FROM credit_balances WHERE user_id=$1").bind::<SqlUuid,_>(owner).get_result::<Row>(&mut conn).await.unwrap();
    assert_eq!(state.value, json!({"balance":2,"reserved":2,"grants":1}));
}
