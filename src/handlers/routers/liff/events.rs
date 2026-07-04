use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::handlers::app::AppState;
use crate::handlers::auth::LiffAuth;
use crate::handlers::routers::error_response::{ApiError, ErrorResponse};
use crate::usecases::liff::track_events::{IncomingEvent, TrackEventsInput};
use crate::usecases::UsecaseError;

#[derive(Deserialize, ToSchema)]
pub(crate) struct TrackEventsRequest {
    events: Vec<EventDto>,
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct EventDto {
    name: String,
    #[serde(default)]
    page: Option<String>,
    #[serde(default)]
    properties: Option<serde_json::Value>,
    client_event_id: Uuid,
    occurred_at: DateTime<Utc>,
}

#[utoipa::path(
    post,
    path = "/api/events",
    request_body = TrackEventsRequest,
    responses(
        (status = 202, description = "Events accepted (fire-and-forget)"),
        (status = 400, description = "Malformed body or batch too large", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn track_events_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
    Json(body): Json<TrackEventsRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let input = TrackEventsInput {
        line_user_id: liff.line_user_id,
        events: body
            .events
            .into_iter()
            .map(|e| IncomingEvent {
                name: e.name,
                page: e.page,
                properties: e.properties,
                client_event_id: e.client_event_id,
                occurred_at: e.occurred_at,
            })
            .collect(),
    };

    // Selective swallow: the client is wrong -> surface it; we are wrong -> 202 + log.
    match state.track_events_usecase.execute(input).await {
        Ok(_) => Ok(StatusCode::ACCEPTED),
        // Batch-cap / contract violation -> 400 so we catch our own frontend bugs.
        Err(e @ UsecaseError::Validation(_)) => Err(ApiError::from(e)),
        // Infra/DB error -> never surface to the fire-and-forget client (no retry-storm on our outage).
        Err(e) => {
            tracing::warn!(error = %e, "analytics event tracking failed; swallowing to 202");
            Ok(StatusCode::ACCEPTED)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_batch_with_optional_page_and_properties() {
        // First event carries page + properties; second omits both (relies on #[serde(default)]).
        let json = r#"{
            "events": [
                {
                    "name": "page_view",
                    "page": "scenes",
                    "properties": { "scene_id": "abc" },
                    "client_event_id": "00000000-0000-0000-0000-000000000001",
                    "occurred_at": "2026-07-05T00:00:00Z"
                },
                {
                    "name": "session_start_click",
                    "client_event_id": "00000000-0000-0000-0000-000000000002",
                    "occurred_at": "2026-07-05T00:00:00Z"
                }
            ]
        }"#;

        let req: TrackEventsRequest =
            serde_json::from_str(json).expect("valid batch should deserialize");

        assert_eq!(req.events.len(), 2);
        assert_eq!(req.events[0].name, "page_view");
        assert_eq!(req.events[0].page.as_deref(), Some("scenes"));
        assert!(req.events[0].properties.is_some());
        // Missing optional fields deserialize to None, not an error.
        assert!(req.events[1].page.is_none());
        assert!(req.events[1].properties.is_none());
    }
}
