use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::handlers::app::AppState;
use crate::handlers::extractors::LiffAuth;
use crate::handlers::routers::error_response::{ApiError, ErrorResponse};
use crate::usecases::liff::end_session::EndSessionInput;
use crate::usecases::liff::get_credit_balance::GetCreditBalanceInput;
use crate::usecases::liff::get_credit_transactions::GetCreditTransactionsInput;
use crate::usecases::liff::get_current_session::GetCurrentSessionInput;
use crate::usecases::liff::get_profile::GetProfileInput;
use crate::usecases::liff::start_session::StartSessionInput;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sessions/start", post(start_session_handler))
        .route("/sessions/end", post(end_session_handler))
        .route("/sessions/current", get(get_current_session_handler))
        .route("/profile", get(get_profile_handler))
        .route("/credits/balance", get(get_credit_balance_handler))
        .route(
            "/credits/transactions",
            get(get_credit_transactions_handler),
        )
}

// ── Session Start/End ──────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub(crate) struct StartSessionRequest {
    scene_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct StartSessionResponse {
    pub session_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct EndSessionResponse {
    pub session_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/sessions/start",
    request_body = StartSessionRequest,
    responses(
        (status = 200, description = "Session started", body = StartSessionResponse),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn start_session_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
    Json(body): Json<StartSessionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let input = StartSessionInput {
        line_user_id: liff.line_user_id,
        scene_id: body.scene_id,
        rich_menu_b_id: state.config.line.rich_menu_b_id.clone(),
    };

    let output = state.start_session_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(StartSessionResponse {
            session_id: output.session_id,
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/api/sessions/end",
    responses(
        (status = 200, description = "Session ended", body = EndSessionResponse),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn end_session_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
) -> Result<impl IntoResponse, ApiError> {
    let input = EndSessionInput {
        line_user_id: liff.line_user_id,
        rich_menu_a_id: state.config.line.rich_menu_a_id.clone(),
        liff_base_url: state.config.line.liff_base_url.clone(),
    };

    let output = state.end_session_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(EndSessionResponse {
            session_id: output.session_id,
        }),
    ))
}

// ── Get Current Session ────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct CurrentSessionResponse {
    pub session: Option<CurrentSessionData>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CurrentSessionData {
    pub id: Uuid,
    pub character_name: String,
    pub character_avatar_url: Option<String>,
    pub scene_name: String,
    pub scene_image_url: Option<String>,
    pub current_location: Option<String>,
    pub scene_time: Option<String>,
    pub mood: String,
    pub relationship_level: String,
    pub message_count: i32,
    pub scene_summary: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path = "/api/sessions/current",
    responses(
        (status = 200, description = "Current active session", body = CurrentSessionResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn get_current_session_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
) -> Result<impl IntoResponse, ApiError> {
    let input = GetCurrentSessionInput {
        line_user_id: liff.line_user_id,
    };

    let output = state.get_current_session_usecase.execute(input).await?;

    let session = output.session.map(|s| CurrentSessionData {
        id: s.id,
        character_name: s.character_name,
        character_avatar_url: s.character_avatar_url,
        scene_name: s.scene_name,
        scene_image_url: s.scene_image_url,
        current_location: s.current_location,
        scene_time: s.scene_time,
        mood: s.mood,
        relationship_level: s.relationship_level,
        message_count: s.message_count,
        scene_summary: s.scene_summary,
        created_at: s.created_at,
    });

    Ok((StatusCode::OK, Json(CurrentSessionResponse { session })))
}

// ── Get Profile ────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct ProfileResponse {
    pub created_at: DateTime<Utc>,
    pub total_sessions: i64,
    pub total_messages: i64,
}

#[utoipa::path(
    get,
    path = "/api/profile",
    responses(
        (status = 200, description = "User profile stats", body = ProfileResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn get_profile_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
) -> Result<impl IntoResponse, ApiError> {
    let input = GetProfileInput {
        line_user_id: liff.line_user_id,
    };

    let output = state.get_profile_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(ProfileResponse {
            created_at: output.created_at,
            total_sessions: output.total_sessions,
            total_messages: output.total_messages,
        }),
    ))
}

// ── Get Credit Balance ─────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct CreditBalanceResponse {
    pub balance: i32,
    pub total_purchased: i32,
    pub total_consumed: i32,
}

#[utoipa::path(
    get,
    path = "/api/credits/balance",
    responses(
        (status = 200, description = "Credit balance", body = CreditBalanceResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn get_credit_balance_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
) -> Result<impl IntoResponse, ApiError> {
    let input = GetCreditBalanceInput {
        line_user_id: liff.line_user_id,
    };

    let output = state.get_credit_balance_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(CreditBalanceResponse {
            balance: output.balance,
            total_purchased: output.total_purchased,
            total_consumed: output.total_consumed,
        }),
    ))
}

// ── Get Credit Transactions ────────────────────────────────

#[derive(Deserialize, IntoParams)]
pub(crate) struct TransactionQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CreditTransactionsResponse {
    pub transactions: Vec<TransactionItemResponse>,
    pub total: i64,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct TransactionItemResponse {
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub amount: i32,
    pub balance_after: i32,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path = "/api/credits/transactions",
    params(TransactionQuery),
    responses(
        (status = 200, description = "Transaction history", body = CreditTransactionsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn get_credit_transactions_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
    Query(query): Query<TransactionQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let input = GetCreditTransactionsInput {
        line_user_id: liff.line_user_id,
        limit: query.limit.clamp(1, 100),
        offset: query.offset.max(0),
    };

    let output = state.get_credit_transactions_usecase.execute(input).await?;

    let transactions = output
        .transactions
        .into_iter()
        .map(|tx| TransactionItemResponse {
            transaction_type: tx.transaction_type,
            amount: tx.amount,
            balance_after: tx.balance_after,
            description: tx.description,
            created_at: tx.created_at,
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(CreditTransactionsResponse {
            transactions,
            total: output.total,
        }),
    ))
}
