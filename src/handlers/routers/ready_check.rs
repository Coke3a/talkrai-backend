use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::handlers::app::AppState;

pub async fn ready_check_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.db_pool.get().await {
        Ok(_) => (StatusCode::OK, Json(json!({"status": "ready"}))),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"status": "unavailable"})),
        ),
    }
}
