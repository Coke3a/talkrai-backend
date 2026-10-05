// diesel QueryableByName derive output trips this lint on clippy ≥1.99
#![allow(clippy::redundant_field_names)]

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
    anyhow::ensure!(row.version == 24, "Apply migration 024 before deployment");
    Ok(())
}
