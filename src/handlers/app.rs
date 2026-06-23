use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::routing::{get, post};
use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowHeaders, AllowMethods, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::DotEnvyConfig;
use crate::domain::repositories::{
    AppConfigRepository, CharacterRepository, CreditRepository, JobRepository, MessageRepository,
    PaymentOrderRepository, RoleplaySessionRepository, SceneRepository, TagDefinitionRepository,
    UserRepository,
};
use crate::domain::services::ai_client::AiClient;
use crate::domain::services::line_client::LineClient;
use crate::domain::value_objects::JobId;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::domain::services::beam_client::BeamClient;
use crate::handlers::openapi::ApiDoc;
use crate::handlers::routers::{default, liff, webhook};
use crate::infra::db::postgres_connection::PgPool;
use crate::usecases::background_jobs::{JobPollerUseCase, StaleJobCleanupUseCase};
use crate::usecases::liff::create_payment::CreatePaymentUseCase;
use crate::usecases::liff::end_session::EndSessionUseCase;
use crate::usecases::liff::get_credit_balance::GetCreditBalanceUseCase;
use crate::usecases::liff::get_credit_transactions::GetCreditTransactionsUseCase;
use crate::usecases::liff::get_current_session::GetCurrentSessionUseCase;
use crate::usecases::liff::get_payment_status::GetPaymentStatusUseCase;
use crate::usecases::liff::get_profile::GetProfileUseCase;
use crate::usecases::liff::get_scenes::GetScenesUseCase;
use crate::usecases::liff::get_tags::GetTagsUseCase;
use crate::usecases::liff::start_session::StartSessionUseCase;
use crate::usecases::webhook::process_beam_webhook::ProcessBeamWebhookUseCase;
use crate::usecases::webhook::process_roleplay_message::ProcessRoleplayMessageUseCase;
use crate::usecases::webhook::receive_webhook::ReceiveWebhookUseCase;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: Arc<PgPool>,
    pub config: Arc<DotEnvyConfig>,
    pub job_sender: mpsc::Sender<JobId>,
    pub line_client: Arc<dyn LineClient>,
    pub ai_client: Arc<dyn AiClient>,
    pub webhook_usecase: Arc<ReceiveWebhookUseCase>,
    pub start_session_usecase: Arc<StartSessionUseCase>,
    pub end_session_usecase: Arc<EndSessionUseCase>,
    pub get_profile_usecase: Arc<GetProfileUseCase>,
    pub get_credit_balance_usecase: Arc<GetCreditBalanceUseCase>,
    pub get_credit_transactions_usecase: Arc<GetCreditTransactionsUseCase>,
    pub get_current_session_usecase: Arc<GetCurrentSessionUseCase>,
    pub get_tags_usecase: Arc<GetTagsUseCase>,
    pub get_scenes_usecase: Arc<GetScenesUseCase>,
    pub create_payment_usecase: Arc<CreatePaymentUseCase>,
    pub get_payment_status_usecase: Arc<GetPaymentStatusUseCase>,
    pub process_beam_webhook_usecase: Arc<ProcessBeamWebhookUseCase>,
}

pub async fn start(config: Arc<DotEnvyConfig>, db_pool: Arc<PgPool>) -> Result<()> {
    init_tracing();

    let infra = create_infrastructure(&config, &db_pool);
    let Infrastructure {
        repos,
        line_client,
        ai_client,
        beam_client,
        config_repo,
    } = infra;

    let (job_sender, job_receiver) =
        mpsc::channel::<JobId>(config.background_tasks.job_channel_capacity);

    let webhook_usecase = Arc::new(ReceiveWebhookUseCase::new(
        Arc::clone(&line_client),
        Arc::clone(&repos.user_repo),
        Arc::clone(&repos.session_repo),
        Arc::clone(&repos.job_repo),
        Arc::clone(&repos.credit_repo),
        Arc::clone(&config_repo),
        job_sender.clone(),
        config.line.liff_base_url.clone(),
        config.line.rich_menu_0_id.clone(),
    ));

    let start_session_usecase = Arc::new(StartSessionUseCase::new(
        Arc::clone(&repos.user_repo),
        Arc::clone(&repos.session_repo),
        Arc::clone(&repos.scene_repo),
        Arc::clone(&repos.character_repo),
        Arc::clone(&repos.message_repo),
        Arc::clone(&line_client),
    ));

    let end_session_usecase = Arc::new(EndSessionUseCase::new(
        Arc::clone(&repos.user_repo),
        Arc::clone(&repos.session_repo),
        Arc::clone(&repos.scene_repo),
        Arc::clone(&repos.character_repo),
        Arc::clone(&line_client),
    ));

    let get_profile_usecase = Arc::new(GetProfileUseCase::new(
        Arc::clone(&repos.user_repo),
        Arc::clone(&repos.session_repo),
        Arc::clone(&repos.message_repo),
    ));

    let get_credit_balance_usecase = Arc::new(GetCreditBalanceUseCase::new(
        Arc::clone(&repos.user_repo),
        Arc::clone(&repos.credit_repo),
    ));

    let get_credit_transactions_usecase = Arc::new(GetCreditTransactionsUseCase::new(
        Arc::clone(&repos.user_repo),
        Arc::clone(&repos.credit_repo),
    ));

    let get_current_session_usecase = Arc::new(GetCurrentSessionUseCase::new(
        Arc::clone(&repos.user_repo),
        Arc::clone(&repos.session_repo),
        Arc::clone(&repos.character_repo),
        Arc::clone(&repos.scene_repo),
    ));

    let get_tags_usecase = Arc::new(GetTagsUseCase::new(Arc::clone(&repos.tag_def_repo)));

    let get_scenes_usecase = Arc::new(GetScenesUseCase::new(
        Arc::clone(&repos.scene_repo),
        Arc::clone(&repos.character_repo),
    ));

    let create_payment_usecase = Arc::new(CreatePaymentUseCase::new(
        Arc::clone(&repos.user_repo),
        Arc::clone(&repos.payment_order_repo),
        Arc::clone(&beam_client),
    ));

    let get_payment_status_usecase = Arc::new(GetPaymentStatusUseCase::new(
        Arc::clone(&repos.user_repo),
        Arc::clone(&repos.payment_order_repo),
    ));

    let process_beam_webhook_usecase = Arc::new(ProcessBeamWebhookUseCase::new(
        Arc::clone(&repos.payment_order_repo),
        Arc::clone(&repos.credit_repo),
        config.beam.hmac_key.clone(),
    ));

    let state = AppState {
        db_pool: Arc::clone(&db_pool),
        config: Arc::clone(&config),
        job_sender: job_sender.clone(),
        line_client: Arc::clone(&line_client),
        ai_client: Arc::clone(&ai_client),
        webhook_usecase,
        start_session_usecase,
        end_session_usecase,
        get_profile_usecase,
        get_credit_balance_usecase,
        get_credit_transactions_usecase,
        get_current_session_usecase,
        get_tags_usecase,
        get_scenes_usecase,
        create_payment_usecase,
        get_payment_status_usecase,
        process_beam_webhook_usecase,
    };

    let app = build_router(state, &config);

    let cancel = CancellationToken::new();
    let handles = spawn_background_tasks(
        &repos,
        &line_client,
        &ai_client,
        &config_repo,
        &config,
        job_sender,
        job_receiver,
        cancel.clone(),
    );

    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!("Listening on {}", addr);
    let listener = TcpListener::bind(&addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(cancel.clone()))
        .await?;

    tracing::info!("Server stopped, cancelling background tasks...");
    cancel.cancel();

    for handle in handles {
        let _ = handle.await;
    }

    tracing::info!("All background tasks stopped");
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .init();
}

fn build_router(state: AppState, config: &DotEnvyConfig) -> Router {
    let allowed_origin = config
        .line
        .cors_origin
        .trim_end_matches('/')
        .parse::<axum::http::HeaderValue>()
        .expect("CORS_ORIGIN must be a valid header value");

    let cors = CorsLayer::new()
        .allow_origin(allowed_origin)
        .allow_methods(AllowMethods::mirror_request())
        .allow_headers(AllowHeaders::mirror_request())
        .allow_credentials(true);

    let middleware = ServiceBuilder::new()
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(config.server.body_limit_bytes))
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(config.server.request_timeout_secs),
        ));

    let router = Router::new()
        .route(
            "/webhook/line-message",
            post(webhook::line_message::webhook_handler),
        )
        .route(
            "/webhook/beam-payment",
            post(webhook::beam_payment::beam_webhook_handler),
        )
        .nest("/api", liff::router())
        .route(
            "/health-check",
            get(default::health_check::health_check_handler),
        )
        .route(
            "/ready-check",
            get(default::ready_check::ready_check_handler),
        );

    let router = if config.server.enable_swagger {
        tracing::info!("Swagger UI enabled at /swagger-ui/");
        router.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
    } else {
        router
    };

    router.layer(middleware).layer(cors).with_state(state)
}

#[allow(clippy::too_many_arguments)]
fn spawn_background_tasks(
    repos: &Repositories,
    line_client: &Arc<dyn LineClient>,
    ai_client: &Arc<dyn AiClient>,
    config_repo: &Arc<dyn AppConfigRepository>,
    config: &DotEnvyConfig,
    job_sender: mpsc::Sender<JobId>,
    job_receiver: mpsc::Receiver<JobId>,
    cancel: CancellationToken,
) -> Vec<JoinHandle<()>> {
    let process_usecase = Arc::new(ProcessRoleplayMessageUseCase::new(
        Arc::clone(&repos.job_repo),
        Arc::clone(&repos.session_repo),
        Arc::clone(&repos.character_repo),
        Arc::clone(&repos.scene_repo),
        Arc::clone(&repos.message_repo),
        Arc::clone(&repos.credit_repo),
        Arc::clone(ai_client),
        Arc::clone(line_client),
        Arc::clone(config_repo),
        config.line.liff_base_url.clone(),
    ));

    let dispatcher = Arc::new(
        crate::handlers::background_jobs::job_dispatcher::JobDispatcher::new(
            Arc::clone(&repos.job_repo),
            process_usecase,
        ),
    );

    let poller_usecase = Arc::new(JobPollerUseCase::new(Arc::clone(&repos.job_repo)));
    let cleanup_usecase = Arc::new(StaleJobCleanupUseCase::new(Arc::clone(&repos.job_repo)));

    let h1 = crate::handlers::background_jobs::job_processor::spawn(
        dispatcher,
        job_receiver,
        cancel.clone(),
        config.background_tasks.max_concurrent_jobs,
    );

    let h2 = crate::handlers::background_jobs::job_poller::spawn(
        poller_usecase,
        job_sender,
        cancel.clone(),
        config.background_tasks.poll_interval_secs,
        config.background_tasks.poll_batch_size,
    );

    let h3 = crate::handlers::background_jobs::stale_job_cleanup::spawn(
        cleanup_usecase,
        cancel.clone(),
        config.background_tasks.cleanup_interval_secs,
        config.background_tasks.stale_threshold_secs,
    );

    vec![h1, h2, h3]
}

async fn shutdown_signal(cancel: CancellationToken) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    let cancelled = cancel.cancelled();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received Ctrl+C"),
        _ = terminate => tracing::info!("Received SIGTERM"),
        _ = cancelled => tracing::info!("Cancellation token triggered"),
    }
}

struct Repositories {
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    job_repo: Arc<dyn JobRepository>,
    character_repo: Arc<dyn CharacterRepository>,
    scene_repo: Arc<dyn SceneRepository>,
    message_repo: Arc<dyn MessageRepository>,
    credit_repo: Arc<dyn CreditRepository>,
    payment_order_repo: Arc<dyn PaymentOrderRepository>,
    tag_def_repo: Arc<dyn TagDefinitionRepository>,
}

struct Infrastructure {
    repos: Repositories,
    line_client: Arc<dyn LineClient>,
    ai_client: Arc<dyn AiClient>,
    beam_client: Arc<dyn BeamClient>,
    config_repo: Arc<dyn AppConfigRepository>,
}

fn create_infrastructure(config: &DotEnvyConfig, db_pool: &Arc<PgPool>) -> Infrastructure {
    use crate::infra::ai::claude_client::ClaudeClient;
    use crate::infra::ai::openai_client::OpenAiClient;
    use crate::infra::ai::together_client::TogetherClient;
    use crate::infra::ai::venice_client::VeniceClient;
    use crate::infra::ai::LlmRouter;
    use crate::infra::db::repositories::{
        AppConfigPostgres, CachedAppConfigRepository, CachedCharacterRepository,
        CachedSceneRepository, CharacterPostgres, CreditPostgres, JobPostgres, MessagePostgres,
        PaymentOrderPostgres, RoleplaySessionPostgres, ScenePostgres, TagDefinitionPostgres,
        UserPostgres,
    };

    // Character/scene form a rarely-changing catalog read on every roleplay turn;
    // cache them to drop two DB round-trips (and pool checkouts) per message.
    let catalog_cache_ttl = std::time::Duration::from_secs(300);

    let repos = Repositories {
        user_repo: Arc::new(UserPostgres::new(Arc::clone(db_pool))),
        character_repo: Arc::new(CachedCharacterRepository::new(
            Arc::new(CharacterPostgres::new(Arc::clone(db_pool))),
            catalog_cache_ttl,
        )),
        scene_repo: Arc::new(CachedSceneRepository::new(
            Arc::new(ScenePostgres::new(Arc::clone(db_pool))),
            catalog_cache_ttl,
        )),
        session_repo: Arc::new(RoleplaySessionPostgres::new(Arc::clone(db_pool))),
        message_repo: Arc::new(MessagePostgres::new(Arc::clone(db_pool))),
        job_repo: Arc::new(JobPostgres::new(Arc::clone(db_pool))),
        credit_repo: Arc::new(CreditPostgres::new(Arc::clone(db_pool))),
        payment_order_repo: Arc::new(PaymentOrderPostgres::new(Arc::clone(db_pool))),
        tag_def_repo: Arc::new(TagDefinitionPostgres::new(Arc::clone(db_pool))),
    };

    let line_client: Arc<dyn LineClient> = Arc::new(crate::infra::line::LineClientImpl::new(
        config.line.channel_secret.clone(),
        config.line.channel_access_token.clone(),
        config.line.line_channel_id.clone(),
    ));

    let config_repo_raw: Arc<dyn crate::domain::repositories::AppConfigRepository> =
        Arc::new(AppConfigPostgres::new(Arc::clone(db_pool)));
    let config_repo: Arc<dyn crate::domain::repositories::AppConfigRepository> =
        Arc::new(CachedAppConfigRepository::new(config_repo_raw));

    let claude: Arc<dyn AiClient> = Arc::new(ClaudeClient::new(config.ai.claude_api_key.clone()));
    let openai: Arc<dyn AiClient> = Arc::new(OpenAiClient::new(config.ai.openai_api_key.clone()));
    let venice: Arc<dyn AiClient> = Arc::new(VeniceClient::new(config.ai.venice_api_key.clone()));
    let together: Arc<dyn AiClient> =
        Arc::new(TogetherClient::new(config.ai.together_api_key.clone()));

    let ai_client: Arc<dyn AiClient> = Arc::new(LlmRouter::new(
        claude,
        openai,
        venice,
        together,
        Arc::clone(&config_repo),
        config.ai.default_provider.clone(),
    ));

    let beam_client: Arc<dyn BeamClient> = Arc::new(crate::infra::beam::BeamClientImpl::new(
        config.beam.merchant_id.clone(),
        config.beam.api_key.clone(),
    ));

    Infrastructure {
        repos,
        line_client,
        ai_client,
        beam_client,
        config_repo,
    }
}
