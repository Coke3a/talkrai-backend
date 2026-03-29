use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::handlers::app::AppState;

pub struct LiffAuth {
    pub line_user_id: String,
}

pub struct LiffAuthError(String);

impl IntoResponse for LiffAuthError {
    fn into_response(self) -> Response {
        let body = json!({
            "error": "UNAUTHORIZED",
            "message": self.0,
        });
        (StatusCode::UNAUTHORIZED, Json(body)).into_response()
    }
}

impl FromRequestParts<AppState> for LiffAuth {
    type Rejection = LiffAuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| LiffAuthError("Missing Authorization header".into()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| LiffAuthError("Invalid Authorization header format".into()))?;

        let profile = state
            .line_client
            .verify_liff_token(token)
            .await
            .map_err(|e| LiffAuthError(format!("Token verification failed: {}", e)))?;

        Ok(LiffAuth {
            line_user_id: profile.user_id,
        })
    }
}
