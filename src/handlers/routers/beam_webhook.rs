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
pub(crate) struct BeamWebhookResponse {
    pub status: String,
}

#[utoipa::path(
    post,
    path = "/beam-webhook",
    params(
        ("X-Beam-Signature" = String, Header, description = "Beam HMAC-SHA256 signature"),
        ("X-Beam-Event" = String, Header, description = "Beam event type")
    ),
    request_body(content = String, description = "Beam webhook event payload (JSON)"),
    responses(
        (status = 200, description = "Webhook accepted", body = BeamWebhookResponse),
        (status = 400, description = "Invalid signature", body = ErrorResponse)
    )
)]
pub(crate) async fn beam_webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, ApiError> {
    let signature = headers
        .get("x-beam-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| UsecaseError::Validation("Missing X-Beam-Signature".into()))?;

    let event_type = headers
        .get("x-beam-event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    // Verify HMAC-SHA256 signature
    let valid = state
        .process_beam_webhook_usecase
        .verify_signature(&body, signature)?;

    if !valid {
        return Err(UsecaseError::Validation("Invalid webhook signature".into()).into());
    }

    // Process event in background (return 200 quickly)
    let usecase = Arc::clone(&state.process_beam_webhook_usecase);
    let event = event_type.to_string();
    let body_vec = body.to_vec();

    tokio::spawn(async move {
        if let Err(e) = usecase.process_event(&event, &body_vec).await {
            tracing::error!(error = %e, event_type = %event, "Beam webhook processing failed");
        }
    });

    Ok((
        StatusCode::OK,
        Json(BeamWebhookResponse {
            status: "accepted".to_string(),
        }),
    ))
}
