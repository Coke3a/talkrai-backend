use talkrai_backend::infra::db::{postgres_connection::create_pool, schema_check::ensure_ready};

#[tokio::test]
#[ignore = "requires an empty local PostgreSQL database before migrations"]
async fn refuses_database_without_web_schema() {
    let pool = create_pool(&std::env::var("TEST_DATABASE_URL").unwrap(), 1).unwrap();
    assert!(
        ensure_ready(&pool).await.is_err(),
        "a reachable database alone is not ready"
    );
}
