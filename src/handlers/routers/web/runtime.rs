use crate::{
    domain::{
        repositories::{
            AppConfigRepository, CharacterRepository, MessageRepository, RoleplaySessionRepository,
            SceneRepository,
        },
        services::ai_client::AiClient,
        web::{TurnRepository, WebDataRepository, WebRepository},
    },
    infra::{
        auth::oidc::OidcProviders,
        db::{
            postgres_connection::PgPool,
            repositories::{
                turn_postgres::TurnPostgres, web_auth_postgres::WebAuthPostgres,
                web_data_postgres::WebDataPostgres,
            },
        },
    },
    usecases::{
        roleplay::generate::GenerateTurn,
        web::{auth::WebAuth, stories::WebStories},
    },
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
pub struct WebRuntime {
    pub auth: WebAuth,
    pub payments: crate::usecases::web::payments::WebPayments,
    pub stories: WebStories,
    pub origin: String,
    pub terms_version: String,
    pub enabled: bool,
}
impl WebRuntime {
    pub fn new(
        pool: Arc<PgPool>,
        beam: Arc<dyn crate::domain::services::beam_client::BeamClient>,
    ) -> anyhow::Result<Arc<Self>> {
        let origin = std::env::var("WEB_ORIGIN")?;
        let url = reqwest::Url::parse(&origin)?;
        anyhow::ensure!(
            url.scheme() == "https"
                && url.path() == "/"
                && url.query().is_none()
                && url.fragment().is_none()
                && url.username().is_empty(),
            "WEB_ORIGIN must be an HTTPS origin"
        );
        let provider = OidcProviders {
            origin: origin.trim_end_matches('/').into(),
            google_id: std::env::var("WEB_GOOGLE_CLIENT_ID")?,
            google_secret: std::env::var("WEB_GOOGLE_CLIENT_SECRET")?,
            line_id: std::env::var("WEB_LINE_CLIENT_ID")?,
            line_secret: std::env::var("WEB_LINE_CLIENT_SECRET")?,
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(15))
                .build()?,
        };
        let repo: Arc<dyn WebRepository> = Arc::new(WebAuthPostgres::new(pool.clone()));
        let data: Arc<dyn WebDataRepository> = Arc::new(WebDataPostgres { pool: pool.clone() });
        let turns: Arc<dyn TurnRepository> = Arc::new(TurnPostgres { pool: pool.clone() });
        Ok(Arc::new(Self {
            payments: crate::usecases::web::payments::WebPayments {
                repo: Arc::new(
                    crate::infra::db::repositories::web_payment_postgres::WebPaymentPostgres {
                        pool,
                    },
                ),
                beam,
                origin: origin.trim_end_matches('/').into(),
            },
            auth: WebAuth {
                repo,
                provider: Arc::new(provider),
            },
            stories: WebStories { data, turns },
            origin: origin.trim_end_matches('/').into(),
            terms_version: std::env::var("WEB_TERMS_VERSION")?,
            enabled: std::env::var("WEB_ADMISSIONS_ENABLED").as_deref() == Ok("true"),
        }))
    }
}
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    runtime: Arc<WebRuntime>,
    sessions: Arc<dyn RoleplaySessionRepository>,
    characters: Arc<dyn CharacterRepository>,
    scenes: Arc<dyn SceneRepository>,
    messages: Arc<dyn MessageRepository>,
    config: Arc<dyn AppConfigRepository>,
    ai: Arc<dyn AiClient>,
    cancel: CancellationToken,
    pool: Arc<PgPool>,
    line: Arc<dyn crate::domain::services::line_client::LineClient>,
) -> tokio::task::JoinHandle<()> {
    let generator = Arc::new(GenerateTurn {
        turns: runtime.stories.turns.clone(),
        sessions,
        characters,
        scenes,
        messages,
        config,
        ai,
    });
    tokio::spawn(async move {
        let delivery_cancel = cancel.clone();
        let delivery = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            loop {
                tokio::select! {
                    _ = delivery_cancel.cancelled() => break,
                    _ = interval.tick() => {
                        if let Err(error) = crate::infra::line::shared_delivery::deliver_pending(pool.clone(),line.clone()).await {
                            tracing::error!(%error,"Delivery retry failed");
                        }
                    }
                }
            }
        });
        let mut tasks = tokio::task::JoinSet::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            tokio::select! {
             _=cancel.cancelled()=>break,
             Some(result)=tasks.join_next(),if !tasks.is_empty()=>{if let Err(error)=result {tracing::error!(%error,"Generation task terminated");}},
             _=interval.tick(),if tasks.len()<10=>{
              match generator.turns.pending().await {
               Ok(ids)=>for id in ids.into_iter().take(10-tasks.len()) {let engine=generator.clone(); tasks.spawn(async move {if let Err(error)=engine.execute(id).await {tracing::warn!(job_id=%id,%error,"Generation failed");}});},
               Err(error)=>tracing::error!(%error,"Generation poll failed"),
              }
             }
            }
        }
        while tasks.join_next().await.is_some() {}
        let _ = delivery.await;
    })
}
