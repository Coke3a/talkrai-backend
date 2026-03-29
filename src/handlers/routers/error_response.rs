use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::json;
use utoipa::ToSchema;

use crate::usecases::UsecaseError;

#[derive(Serialize, ToSchema)]
pub(crate) struct ErrorResponse {
    pub error: String,
    pub message: String,
}

pub struct ApiError(pub UsecaseError);

impl From<UsecaseError> for ApiError {
    fn from(err: UsecaseError) -> Self {
        ApiError(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self.0 {
            UsecaseError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            UsecaseError::Validation(msg) => {
                (StatusCode::BAD_REQUEST, "VALIDATION_ERROR", msg.clone())
            }
            UsecaseError::InsufficientCredits => (
                StatusCode::PAYMENT_REQUIRED,
                "INSUFFICIENT_CREDITS",
                "Insufficient credits".to_string(),
            ),
            UsecaseError::LineError(msg) => (StatusCode::BAD_GATEWAY, "LINE_ERROR", msg.clone()),
            UsecaseError::AiError(msg) => (StatusCode::BAD_GATEWAY, "AI_ERROR", msg.clone()),
            UsecaseError::AiResponseInvalid => (
                StatusCode::BAD_GATEWAY,
                "AI_RESPONSE_INVALID",
                "AI response invalid after all retries".to_string(),
            ),
            UsecaseError::PaymentError(msg) => {
                (StatusCode::BAD_GATEWAY, "PAYMENT_ERROR", msg.clone())
            }
            UsecaseError::Infra(err) => {
                tracing::error!(error = %err, "Internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "An internal error occurred".to_string(),
                )
            }
        };

        let body = json!({
            "error": code,
            "message": message,
        });

        (status, Json(body)).into_response()
    }
}
