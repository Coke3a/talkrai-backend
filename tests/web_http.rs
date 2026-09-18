use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use diesel::{sql_query, sql_types::Jsonb, QueryableByName};
use diesel_async::RunQueryDsl;
use serde_json::{json, Value};
use std::sync::Arc;
use talkrai_backend::domain::web::WebError;
use talkrai_backend::{
    domain::{
        services::{
            beam_client::{BeamClient, CreatePaymentLinkInput, CreatePaymentLinkOutput},
            beam_client_error::BeamClientError,
        },
        web::{AuthFlow, Identity, WebRepository},
    },
    handlers::routers::web::{routes, runtime::WebRuntime},
    infra::{
        auth::oidc::OidcProviders,
        db::{
            postgres_connection::create_pool,
            repositories::{
                turn_postgres::TurnPostgres, web_auth_postgres::WebAuthPostgres,
                web_data_postgres::WebDataPostgres, web_payment_postgres::WebPaymentPostgres,
            },
        },
    },
    usecases::web::{
        auth::{hash_token, WebAuth},
        payments::WebPayments,
        stories::WebStories,
    },
};
use tower::ServiceExt;
struct UnusedBeam;
#[async_trait::async_trait]
impl BeamClient for UnusedBeam {
    async fn create_payment_link(
        &self,
        _: CreatePaymentLinkInput,
    ) -> Result<CreatePaymentLinkOutput, BeamClientError> {
        panic!("Payment provider must not be called in authorization tests")
    }
}
#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type=Jsonb)]
    value: Value,
}
fn assert_contract(name: &str, value: &Value) {
    let mut contract: Value =
        serde_json::from_str(include_str!("../docs/web-openapi.json")).unwrap();
    contract["$ref"] = json!(format!("#/components/schemas/{name}"));
    let validator = jsonschema::validator_for(&contract).unwrap();
    let errors: Vec<_> = validator
        .iter_errors(value)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{name}: {errors:?}");
}
#[tokio::test]
#[ignore = "requires isolated migrated PostgreSQL via TEST_DATABASE_URL"]
async fn web_http_owner_csrf_and_public_contracts() {
    let pool = Arc::new(
        create_pool(
            &std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL required"),
            5,
        )
        .unwrap(),
    );
    let auth = Arc::new(WebAuthPostgres::new(pool.clone()));
    let token = uuid::Uuid::new_v4().to_string();
    let flow = AuthFlow {
        acquisition_source: "khui".into(),
        state_hash: "unused".into(),
        browser_hash: "unused".into(),
        provider: "google".into(),
        nonce: "unused".into(),
        pkce_verifier: "unused".into(),
        return_to: "/stories".into(),
        link_user_id: None,
        link_session_hash: None,
    };
    let owner = auth
        .login(
            &Identity {
                provider: "google".into(),
                issuer: "https://accounts.google.com".into(),
                subject: token.clone(),
                display_name: "HTTP test".into(),
            },
            &flow,
            &hash_token(&token),
            "csrf-test",
            true,
        )
        .await
        .unwrap();
    auth.save_flow(&flow).await.unwrap();
    assert!(matches!(
        auth.consume_flow(&flow.state_hash, "wrong-browser", &flow.provider)
            .await,
        Err(WebError::Rejected("INVALID_AUTH_FLOW"))
    ));
    auth.consume_flow(&flow.state_hash, &flow.browser_hash, &flow.provider)
        .await
        .unwrap();
    assert!(matches!(
        auth.consume_flow(&flow.state_hash, &flow.browser_hash, &flow.provider)
            .await,
        Err(WebError::Rejected("INVALID_AUTH_FLOW"))
    ));
    let beam = Arc::new(UnusedBeam);
    let runtime = Arc::new(WebRuntime {
        auth: WebAuth {
            repo: auth.clone(),
            provider: Arc::new(OidcProviders {
                origin: "https://app.test".into(),
                google_id: "unused".into(),
                google_secret: "unused".into(),
                line_id: "unused".into(),
                line_secret: "unused".into(),
                http: reqwest::Client::new(),
            }),
        },
        stories: WebStories {
            data: Arc::new(WebDataPostgres { pool: pool.clone() }),
            turns: Arc::new(TurnPostgres { pool: pool.clone() }),
        },
        payments: WebPayments {
            repo: Arc::new(WebPaymentPostgres { pool: pool.clone() }),
            beam,
            origin: "https://app.test".into(),
        },
        origin: "https://app.test".into(),
        terms_version: "1".into(),
        enabled: true,
    });
    let app = routes::router().with_state(runtime);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/web/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let cookie = format!("__Host-talkrai={token}");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/web/me")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_contract("Me", &value);
    assert_eq!(value["id"], owner.to_string());
    assert_eq!(value["csrf_token"], "csrf-test");
    for (origin, csrf, expected) in [
        ("https://evil.test", "csrf-test", 403),
        ("https://app.test", "wrong", 403),
        ("https://app.test", "csrf-test", 200),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/web/terms/accept")
                    .header("cookie", &cookie)
                    .header("origin", origin)
                    .header("x-csrf-token", csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"version":"1"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), expected);
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/web/jobs/{}", uuid::Uuid::new_v4()))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/public/scenes?limit=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_contract("ScenePage", &value);
    assert!(value["items"].is_array());
    assert!(value.get("next_cursor").is_some());
    for (path, schema) in [
        ("credits", "Credits"),
        ("credit-transactions", "TransactionPage"),
        ("sessions", "StoryPage"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/web/{path}"))
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let value: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
        assert_contract(schema, &value);
    }

    let scene_id = uuid::Uuid::new_v4();
    let mut setup = pool.get().await.unwrap();
    sql_query("WITH c AS (INSERT INTO characters(name,personality,speaking_style,background,system_prompt,gender) VALUES('HTTP test','Test','Test','Test','Private','female') RETURNING id) INSERT INTO scenes(id,character_id,name,location,time_of_day,atmosphere,situation_prompt,opening_narrator,opening_dialogue) SELECT ($1->>'id')::uuid,id,'HTTP scene','Test','Test','Test','Private','Opening','Hello' FROM c").bind::<Jsonb,_>(json!({"id":scene_id})).execute(&mut setup).await.unwrap();
    drop(setup);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/web/sessions")
                .header("cookie", &cookie)
                .header("origin", "https://app.test")
                .header("x-csrf-token", "csrf-test")
                .header("idempotency-key", "http-story")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"scene_id":scene_id,"persona":{"name":"Reader","description":""}})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let story: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_contract("Story", &story);
    let story_id = story["id"].as_str().unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/web/sessions/{story_id}/messages"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let messages: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_contract("MessagePage", &messages);
    assert_eq!(messages["items"].as_array().unwrap().len(), 1);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/web/sessions/{story_id}/turns"))
                .header("cookie", &cookie)
                .header("origin", "https://app.test")
                .header("x-csrf-token", "csrf-test")
                .header("idempotency-key", "http-turn")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"content":"Hello","expected_version":0}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let job: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_contract("Job", &job);
    let job_id = job["id"].as_str().unwrap();
    let mut conn = pool.get().await.unwrap();
    sql_query("UPDATE jobs SET status='failed',failed_reason='GENERATION_FAILED',reservation=0 WHERE id=($1->>'id')::uuid").bind::<Jsonb,_>(json!({"id":job_id})).execute(&mut conn).await.unwrap();
    sql_query("UPDATE credit_balances SET reserved=0 WHERE user_id=($1->>'id')::uuid")
        .bind::<Jsonb, _>(json!({"id":owner}))
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/web/jobs/{job_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "failed job is readable, not a missing resource"
    );
    let failed: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_contract("Job", &failed);
    assert_eq!(failed["status"], "failed");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/web/jobs/not-a-uuid")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let invalid: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert_contract("Error", &invalid);
    let mut conn = pool.get().await.unwrap();
    let row = sql_query(
        "SELECT jsonb_build_object('id',id,'source',acquisition_source) AS value FROM users WHERE id=($1->>'id')::uuid",
    )
    .bind::<Jsonb, _>(json!({"id":owner}))
    .get_result::<Row>(&mut conn)
    .await
    .unwrap();
    assert_eq!(row.value["id"], owner.to_string());
    assert_eq!(row.value["source"], "khui");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/web/auth/logout")
                .header("cookie", &cookie)
                .header("origin", "https://app.test")
                .header("x-csrf-token", "csrf-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/web/me")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
