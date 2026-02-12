use anyhow::Result;
use std::sync::Arc;

use talk_a_line_backend::config::config_loader;
use talk_a_line_backend::handlers;
use talk_a_line_backend::infra::db::postgres_connection;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Application error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    dotenvy::dotenv().ok();

    let config = config_loader::load()?;
    let pool = postgres_connection::create_pool(
        &config.database.url,
        config.database.max_connections as usize,
    )?;

    handlers::app::start(Arc::new(config), Arc::new(pool)).await?;

    Ok(())
}
