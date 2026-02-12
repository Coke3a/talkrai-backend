use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;

pub type PgPool = Pool<AsyncPgConnection>;

pub fn create_pool(database_url: &str, max_size: usize) -> Result<PgPool, anyhow::Error> {
    let manager = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
    let pool = Pool::builder(manager).max_size(max_size).build()?;
    Ok(pool)
}
