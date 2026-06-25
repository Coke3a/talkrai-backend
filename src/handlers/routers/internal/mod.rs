use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Serialize;

use crate::handlers::app::AppState;
use crate::handlers::auth::InternalAuth;
use crate::handlers::routers::error_response::ApiError;
use crate::infra::clock::bangkok_today;

pub fn router() -> Router<AppState> {
    Router::new().route("/jobs/daily-reengagement", post(daily_reengagement_handler))
}

#[derive(Serialize)]
struct DailyReengagementResponse {
    enqueued: usize,
    capped: bool,
}

/// Idempotent daily trigger for the external scheduler. Enqueues one reminder job per eligible
/// user and marks them reminded; a second call the same day enqueues zero.
async fn daily_reengagement_handler(
    State(state): State<AppState>,
    _auth: InternalAuth,
) -> Result<Json<DailyReengagementResponse>, ApiError> {
    let today = bangkok_today();
    let result = state.enqueue_reengagement_usecase.execute(today).await?;
    Ok(Json(DailyReengagementResponse {
        enqueued: result.enqueued,
        capped: result.capped,
    }))
}
