use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Json;
use axum::Router;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::handlers::app::AppState;
use crate::handlers::extractors::LiffAuth;
use crate::handlers::routers::error_response::ApiError;
use crate::usecases::accept_terms::AcceptTermsInput;
use crate::usecases::end_session::EndSessionInput;
use crate::usecases::start_session::StartSessionInput;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users/accept-terms", post(accept_terms_handler))
        .route("/sessions/start", post(start_session_handler))
        .route("/sessions/end", post(end_session_handler))
}

async fn accept_terms_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
) -> Result<impl IntoResponse, ApiError> {
    let input = AcceptTermsInput {
        line_user_id: liff.line_user_id,
        rich_menu_a_id: state.config.line.rich_menu_a_id.clone(),
    };

    let output = state.accept_terms_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "user_id": output.user_id,
            "terms_accepted_at": output.terms_accepted_at,
        })),
    ))
}

#[derive(Deserialize)]
struct StartSessionRequest {
    scene_id: Uuid,
}

async fn start_session_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
    Json(body): Json<StartSessionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let input = StartSessionInput {
        line_user_id: liff.line_user_id,
        scene_id: body.scene_id,
        rich_menu_b_id: state.config.line.rich_menu_b_id.clone(),
    };

    let output = state.start_session_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "session_id": output.session_id,
        })),
    ))
}

async fn end_session_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
) -> Result<impl IntoResponse, ApiError> {
    let input = EndSessionInput {
        line_user_id: liff.line_user_id,
        rich_menu_a_id: state.config.line.rich_menu_a_id.clone(),
    };

    let output = state.end_session_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "session_id": output.session_id,
        })),
    ))
}
