use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::handlers::app::AppState;

/// Extractor guarding internal-only endpoints with a shared `X-Internal-Token` secret.
/// Not under LIFF auth; the external scheduler presents the token.
pub struct InternalAuth;

pub struct InternalAuthError(String);

impl IntoResponse for InternalAuthError {
    fn into_response(self) -> Response {
        let body = json!({
            "error": "UNAUTHORIZED",
            "message": self.0,
        });
        (StatusCode::UNAUTHORIZED, Json(body)).into_response()
    }
}

impl FromRequestParts<AppState> for InternalAuth {
    type Rejection = InternalAuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let expected = state.config.internal.api_token.as_bytes();
        if expected.is_empty() {
            // Fail closed: an unset token disables the internal API rather than matching "".
            return Err(InternalAuthError(
                "Internal API is disabled (INTERNAL_API_TOKEN unset)".into(),
            ));
        }

        let provided = parts
            .headers
            .get("x-internal-token")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| InternalAuthError("Missing X-Internal-Token header".into()))?;

        if constant_time_eq(expected, provided.as_bytes()) {
            Ok(InternalAuth)
        } else {
            Err(InternalAuthError("Invalid internal token".into()))
        }
    }
}

/// Length-aware constant-time byte comparison (avoids a timing side-channel without a new crate).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
