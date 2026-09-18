use super::postgres_connection::PgPool;
use diesel::{sql_query, sql_types::Integer, QueryableByName};
use diesel_async::RunQueryDsl;

#[derive(QueryableByName)]
struct SchemaVersion {
    #[diesel(sql_type = Integer)]
    version: i32,
}

pub async fn ensure_ready(pool: &PgPool) -> anyhow::Result<()> {
    let mut conn = pool.get().await?;
    let row = sql_query("SELECT public.talkrai_schema_version() AS version")
        .get_result::<SchemaVersion>(&mut conn)
        .await?;
    anyhow::ensure!(
        row.version == 23,
        "Apply the complete migration 023 before deployment"
    );
    Ok(())
}
