use utoipa::openapi::security::{ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::handlers::routers::{default, liff, webhook};

#[derive(OpenApi)]
#[openapi(
    paths(
        default::health_check::health_check_handler,
        default::ready_check::ready_check_handler,
        webhook::line_message::webhook_handler,
        liff::start_session_handler,
        liff::end_session_handler,
        liff::get_current_session_handler,
        liff::get_me_handler,
        liff::accept_terms_handler,
        liff::get_profile_handler,
        liff::get_credit_balance_handler,
        liff::get_credit_transactions_handler,
        liff::get_tags_handler,
        liff::get_scenes_handler,
        liff::get_legal_doc_handler,
    ),
    components(schemas(
        default::health_check::HealthResponse,
        default::ready_check::ReadyResponse,
        webhook::line_message::WebhookResponse,
        liff::StartSessionRequest,
        liff::StartSessionResponse,
        liff::EndSessionResponse,
        liff::CurrentSessionResponse,
        liff::CurrentSessionData,
        liff::MeResponse,
        liff::ProfileResponse,
        liff::CreditBalanceResponse,
        liff::CreditTransactionsResponse,
        liff::TransactionItemResponse,
        liff::TagsResponse,
        liff::TagItemResponse,
        liff::ScenesResponse,
        liff::SceneItemResponse,
        liff::SceneCharacterResponse,
        liff::LegalDocResponse,
        liff::LegalSectionResponse,
        crate::handlers::routers::error_response::ErrorResponse,
    )),
    modifiers(&SecurityAddon),
    info(
        title = "Talk-a-Line API",
        version = "0.1.0",
        description = "LINE Messaging API backend for character roleplay"
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_default();
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
        components.add_security_scheme(
            "line_signature",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("x-line-signature"))),
        );
    }
}
