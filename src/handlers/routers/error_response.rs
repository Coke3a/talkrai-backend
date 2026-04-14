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
            UsecaseError::NotFound(msg) => {
                tracing::warn!(error_code = "NOT_FOUND", details = %msg, "Not found");
                (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone())
            }
            UsecaseError::Validation(msg) => {
                tracing::warn!(error_code = "VALIDATION_ERROR", details = %msg, "Validation error");
                (StatusCode::BAD_REQUEST, "VALIDATION_ERROR", msg.clone())
            }
            UsecaseError::InsufficientCredits => {
                tracing::warn!(error_code = "INSUFFICIENT_CREDITS", "Insufficient credits");
                (
                    StatusCode::PAYMENT_REQUIRED,
                    "INSUFFICIENT_CREDITS",
                    "Insufficient credits".to_string(),
                )
            }
            UsecaseError::UserInactive => {
                tracing::warn!(error_code = "USER_INACTIVE", "User account is inactive");
                (
                    StatusCode::FORBIDDEN,
                    "USER_INACTIVE",
                    "กรุณา Add Friend TalkRai เพื่อใช้งานต่อ".to_string(),
                )
            }
            UsecaseError::LineError(msg) => {
                tracing::error!(error_code = "LINE_ERROR", details = %msg, "LINE API error");
                (StatusCode::BAD_GATEWAY, "LINE_ERROR", msg.clone())
            }
            UsecaseError::AiError(msg) => {
                tracing::error!(error_code = "AI_ERROR", details = %msg, "AI processing error");
                (StatusCode::BAD_GATEWAY, "AI_ERROR", msg.clone())
            }
            UsecaseError::AiResponseInvalid => {
                tracing::error!(
                    error_code = "AI_RESPONSE_INVALID",
                    "AI response invalid after all retries"
                );
                (
                    StatusCode::BAD_GATEWAY,
                    "AI_RESPONSE_INVALID",
                    "AI response invalid after all retries".to_string(),
                )
            }
            UsecaseError::PaymentError(msg) => {
                tracing::error!(error_code = "PAYMENT_ERROR", details = %msg, "Payment error");
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
