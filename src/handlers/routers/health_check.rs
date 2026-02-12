use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

pub async fn health_check_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}
