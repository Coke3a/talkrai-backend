use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use bytes::Bytes;
use serde_json::json;

use crate::handlers::app::AppState;
use crate::handlers::routers::error_response::ApiError;
use crate::usecases::receive_webhook::ReceiveWebhookInput;
use crate::usecases::UsecaseError;

pub async fn webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, ApiError> {
    let signature = headers
        .get("x-line-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| UsecaseError::Validation("Missing x-line-signature".into()))?;

    let input = ReceiveWebhookInput {
        body: body.to_vec(),
        signature: signature.to_string(),
    };

    let output = state.webhook_usecase.execute(input).await?;

    Ok((StatusCode::OK, Json(json!({ "job_ids": output.job_ids }))))
}
