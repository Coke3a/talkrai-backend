use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

use crate::handlers::app::AppState;

#[derive(Serialize, ToSchema)]
pub(crate) struct ReadyResponse {
    pub status: String,
}

#[utoipa::path(
    get,
    path = "/ready-check",
    responses(
        (status = 200, description = "Service is ready", body = ReadyResponse),
        (status = 503, description = "Service is unavailable", body = ReadyResponse)
    )
)]
pub async fn ready_check_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.db_pool.get().await {
        Ok(_) => (
            StatusCode::OK,
            Json(ReadyResponse {
                status: "ready".to_string(),
            }),
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadyResponse {
                status: "unavailable".to_string(),
            }),
        ),
    }
}
