use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Json;
use axum::Router;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::handlers::app::AppState;
use crate::handlers::extractors::LiffAuth;
use crate::handlers::routers::error_response::{ApiError, ErrorResponse};
use crate::usecases::end_session::EndSessionInput;
use crate::usecases::start_session::StartSessionInput;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sessions/start", post(start_session_handler))
        .route("/sessions/end", post(end_session_handler))
}

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
    };

    let output = state.end_session_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(EndSessionResponse {
            session_id: output.session_id,
        }),
    ))
}
