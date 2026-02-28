use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use bytes::Bytes;
use serde::Serialize;
use utoipa::ToSchema;

use crate::handlers::app::AppState;
use crate::handlers::routers::error_response::{ApiError, ErrorResponse};
use crate::usecases::UsecaseError;

#[derive(Serialize, ToSchema)]
pub(crate) struct WebhookResponse {
    pub status: String,
}

#[utoipa::path(
    post,
    path = "/webhook",
    params(
        ("x-line-signature" = String, Header, description = "LINE webhook signature for request verification")
    ),
    request_body(content = String, description = "LINE webhook event payload (JSON)"),
    responses(
        (status = 200, description = "Webhook accepted", body = WebhookResponse),
        (status = 400, description = "Invalid signature", body = ErrorResponse)
    ),
    security(("line_signature" = []))
)]
pub(crate) async fn webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, ApiError> {
    let signature = headers
        .get("x-line-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| UsecaseError::Validation("Missing x-line-signature".into()))?;

    // Sync: verify signature only (<1ms, CPU-only)
    let valid = state.webhook_usecase.verify_signature(&body, signature)?;
    if !valid {
        return Err(UsecaseError::Validation("Invalid webhook signature".into()).into());
    }

    // Fire-and-forget: spawn event processing in background
    let usecase = Arc::clone(&state.webhook_usecase);
    let body_vec = body.to_vec();
    tokio::spawn(async move {
        if let Err(e) = usecase.process_events(body_vec).await {
            tracing::error!(error = %e, "Background webhook processing failed");
        }
    });

    Ok((
        StatusCode::OK,
        Json(WebhookResponse {
            status: "accepted".to_string(),
        }),
    ))
}
