use anyhow::{Context, Result};

use super::{
    AiConfig, BackgroundTasksConfig, DatabaseConfig, DotEnvyConfig, LineConfig, ServerConfig,
};

pub fn load() -> Result<DotEnvyConfig> {
    let server = ServerConfig {
        host: env_or("SERVER_HOST", "0.0.0.0"),
        port: env_or("SERVER_PORT", "8080")
            .parse()
            .context("Invalid SERVER_PORT")?,
        request_timeout_secs: env_or("REQUEST_TIMEOUT_SECS", "30")
            .parse()
            .context("Invalid REQUEST_TIMEOUT_SECS")?,
        body_limit_bytes: env_or("BODY_LIMIT_BYTES", "1048576")
            .parse()
            .context("Invalid BODY_LIMIT_BYTES")?,
        enable_swagger: env_or("ENABLE_SWAGGER", "false")
            .parse()
            .context("Invalid ENABLE_SWAGGER")?,
    };

    let database = DatabaseConfig {
        url: std::env::var("DATABASE_URL").context("DATABASE_URL is required")?,
        max_connections: env_or("DB_MAX_CONNECTIONS", "10")
            .parse()
            .context("Invalid DB_MAX_CONNECTIONS")?,
    };

    let line = LineConfig {
        channel_secret: std::env::var("LINE_CHANNEL_SECRET")
            .context("LINE_CHANNEL_SECRET is required")?,
        channel_access_token: std::env::var("LINE_CHANNEL_ACCESS_TOKEN")
            .context("LINE_CHANNEL_ACCESS_TOKEN is required")?,
        liff_base_url: std::env::var("LIFF_BASE_URL").context("LIFF_BASE_URL is required")?,
        rich_menu_0_id: std::env::var("RICH_MENU_0_ID").context("RICH_MENU_0_ID is required")?,
        rich_menu_a_id: std::env::var("RICH_MENU_A_ID").context("RICH_MENU_A_ID is required")?,
        rich_menu_b_id: std::env::var("RICH_MENU_B_ID").context("RICH_MENU_B_ID is required")?,
    };

    let ai = AiConfig {
        claude_api_key: std::env::var("CLAUDE_API_KEY").context("CLAUDE_API_KEY is required")?,
        openai_api_key: std::env::var("OPENAI_API_KEY").context("OPENAI_API_KEY is required")?,
        venice_api_key: std::env::var("VENICE_API_KEY").context("VENICE_API_KEY is required")?,
        together_api_key: std::env::var("TOGETHER_API_KEY")
            .context("TOGETHER_API_KEY is required")?,
        default_provider: env_or("DEFAULT_LLM_PROVIDER", "claude"),
    };

    let bg_defaults = BackgroundTasksConfig::default();
    let background_tasks = BackgroundTasksConfig {
        job_channel_capacity: env_or(
            "JOB_CHANNEL_CAPACITY",
            &bg_defaults.job_channel_capacity.to_string(),
        )
        .parse()
        .context("Invalid JOB_CHANNEL_CAPACITY")?,
        max_concurrent_jobs: env_or(
            "MAX_CONCURRENT_JOBS",
            &bg_defaults.max_concurrent_jobs.to_string(),
        )
        .parse()
        .context("Invalid MAX_CONCURRENT_JOBS")?,
        poll_interval_secs: env_or(
            "POLL_INTERVAL_SECS",
            &bg_defaults.poll_interval_secs.to_string(),
        )
        .parse()
        .context("Invalid POLL_INTERVAL_SECS")?,
        poll_batch_size: env_or("POLL_BATCH_SIZE", &bg_defaults.poll_batch_size.to_string())
            .parse()
            .context("Invalid POLL_BATCH_SIZE")?,
        cleanup_interval_secs: env_or(
            "CLEANUP_INTERVAL_SECS",
            &bg_defaults.cleanup_interval_secs.to_string(),
        )
        .parse()
        .context("Invalid CLEANUP_INTERVAL_SECS")?,
        stale_threshold_secs: env_or(
            "STALE_THRESHOLD_SECS",
            &bg_defaults.stale_threshold_secs.to_string(),
        )
        .parse()
        .context("Invalid STALE_THRESHOLD_SECS")?,
    };

    Ok(DotEnvyConfig {
        server,
        database,
        line,
        ai,
        background_tasks,
    })
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
