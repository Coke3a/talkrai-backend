pub mod config_loader;

pub struct DotEnvyConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub line: LineConfig,
    pub ai: AiConfig,
    pub background_tasks: BackgroundTasksConfig,
}

pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub request_timeout_secs: u64,
    pub body_limit_bytes: usize,
    pub enable_swagger: bool,
}

pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

pub struct LineConfig {
    pub channel_secret: String,
    pub channel_access_token: String,
    pub line_channel_id: String,
    pub liff_base_url: String,
    pub rich_menu_0_id: String,
    pub rich_menu_a_id: String,
    pub rich_menu_b_id: String,
}

pub struct AiConfig {
    pub claude_api_key: String,
    pub openai_api_key: String,
    pub venice_api_key: String,
    pub together_api_key: String,
    pub default_provider: String,
}

pub struct BackgroundTasksConfig {
    pub job_channel_capacity: usize,
    pub max_concurrent_jobs: usize,
    pub poll_interval_secs: u64,
    pub poll_batch_size: i64,
    pub cleanup_interval_secs: u64,
    pub stale_threshold_secs: i64,
}

impl Default for BackgroundTasksConfig {
    fn default() -> Self {
        Self {
            job_channel_capacity: 100,
            max_concurrent_jobs: 10,
            poll_interval_secs: 5,
            poll_batch_size: 10,
            cleanup_interval_secs: 60,
            stale_threshold_secs: 120,
        }
    }
}
