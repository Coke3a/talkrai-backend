use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::handlers::app::AppState;
use crate::handlers::auth::LiffAuth;
use crate::handlers::routers::error_response::{ApiError, ErrorResponse};
use crate::infra::clock::bangkok_today;
use crate::usecases::liff::create_payment::CreatePaymentInput;
use crate::usecases::liff::end_session::EndSessionInput;
use crate::usecases::liff::get_credit_balance::GetCreditBalanceInput;
use crate::usecases::liff::get_credit_transactions::GetCreditTransactionsInput;
use crate::usecases::liff::get_current_session::GetCurrentSessionInput;
use crate::usecases::liff::get_payment_status::GetPaymentStatusInput;
use crate::usecases::liff::get_profile::GetProfileInput;
use crate::usecases::liff::get_scenes::{SceneCharacterItem, SceneItem};
use crate::usecases::liff::get_tags::TagItem;
use crate::usecases::liff::start_session::StartSessionInput;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sessions/start", post(start_session_handler))
        .route("/sessions/end", post(end_session_handler))
        .route("/sessions/current", get(get_current_session_handler))
        .route("/profile", get(get_profile_handler))
        .route("/credits/balance", get(get_credit_balance_handler))
        .route(
            "/credits/transactions",
            get(get_credit_transactions_handler),
        )
        .route("/payments/create", post(create_payment_handler))
        .route("/payments/status", get(get_payment_status_handler))
        .route("/tags", get(get_tags_handler))
        .route("/scenes", get(get_scenes_handler))
}

// ── Session Start/End ──────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub(crate) struct StartSessionRequest {
    scene_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct StartSessionResponse {
    pub session_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct EndSessionResponse {
    pub session_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/sessions/start",
    request_body = StartSessionRequest,
    responses(
        (status = 200, description = "Session started", body = StartSessionResponse),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn start_session_handler(
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
        Json(StartSessionResponse {
            session_id: output.session_id,
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/api/sessions/end",
    responses(
        (status = 200, description = "Session ended", body = EndSessionResponse),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn end_session_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
) -> Result<impl IntoResponse, ApiError> {
    let input = EndSessionInput {
        line_user_id: liff.line_user_id,
        rich_menu_a_id: state.config.line.rich_menu_a_id.clone(),
        liff_base_url: state.config.line.liff_base_url.clone(),
    };

    let output = state.end_session_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(EndSessionResponse {
            session_id: output.session_id,
        }),
    ))
}

// ── Get Current Session ────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct CurrentSessionResponse {
    pub session: Option<CurrentSessionData>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CurrentSessionData {
    pub id: Uuid,
    pub character_name: String,
    pub character_avatar_url: Option<String>,
    pub scene_name: String,
    pub scene_image_url: Option<String>,
    pub current_location: Option<String>,
    pub scene_time: Option<String>,
    pub mood: String,
    pub relationship_level: String,
    pub message_count: i32,
    pub scene_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub relationship_progress: f32,
    pub next_level_label: Option<String>,
    pub messages_to_next: Option<i32>,
}

#[utoipa::path(
    get,
    path = "/api/sessions/current",
    responses(
        (status = 200, description = "Current active session", body = CurrentSessionResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn get_current_session_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
) -> Result<impl IntoResponse, ApiError> {
    let input = GetCurrentSessionInput {
        line_user_id: liff.line_user_id,
    };

    let output = state.get_current_session_usecase.execute(input).await?;

    let session = output.session.map(|s| CurrentSessionData {
        id: s.id,
        character_name: s.character_name,
        character_avatar_url: s.character_avatar_url,
        scene_name: s.scene_name,
        scene_image_url: s.scene_image_url,
        current_location: s.current_location,
        scene_time: s.scene_time,
        mood: s.mood,
        relationship_level: s.relationship_level,
        message_count: s.message_count,
        scene_summary: s.scene_summary,
        created_at: s.created_at,
        relationship_progress: s.relationship_progress,
        next_level_label: s.next_level_label,
        messages_to_next: s.messages_to_next,
    });

    Ok((StatusCode::OK, Json(CurrentSessionResponse { session })))
}

// ── Get Profile ────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct ProfileResponse {
    pub created_at: DateTime<Utc>,
    pub total_sessions: i64,
    pub total_messages: i64,
    pub longest_streak: i32,
    pub current_streak: i32,
    pub checked_in_today: bool,
    pub today_cycle_day: i32,
    pub today_credits: i32,
    pub days_to_chest: i32,
    pub weekly_credits: Vec<i32>,
}

#[utoipa::path(
    get,
    path = "/api/profile",
    responses(
        (status = 200, description = "User profile stats", body = ProfileResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn get_profile_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
) -> Result<impl IntoResponse, ApiError> {
    let input = GetProfileInput {
        line_user_id: liff.line_user_id,
        today: bangkok_today(),
    };

    let output = state.get_profile_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(ProfileResponse {
            created_at: output.created_at,
            total_sessions: output.total_sessions,
            total_messages: output.total_messages,
            longest_streak: output.longest_streak,
            current_streak: output.current_streak,
            checked_in_today: output.checked_in_today,
            today_cycle_day: output.today_cycle_day,
            today_credits: output.today_credits,
            days_to_chest: output.days_to_chest,
            weekly_credits: output.weekly_credits,
        }),
    ))
}

// ── Get Credit Balance ─────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct CreditBalanceResponse {
    pub balance: i32,
    pub total_purchased: i32,
    pub total_consumed: i32,
}

#[utoipa::path(
    get,
    path = "/api/credits/balance",
    responses(
        (status = 200, description = "Credit balance", body = CreditBalanceResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn get_credit_balance_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
) -> Result<impl IntoResponse, ApiError> {
    let input = GetCreditBalanceInput {
        line_user_id: liff.line_user_id,
    };

    let output = state.get_credit_balance_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(CreditBalanceResponse {
            balance: output.balance,
            total_purchased: output.total_purchased,
            total_consumed: output.total_consumed,
        }),
    ))
}

// ── Get Credit Transactions ────────────────────────────────

#[derive(Deserialize, IntoParams)]
pub(crate) struct TransactionQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CreditTransactionsResponse {
    pub transactions: Vec<TransactionItemResponse>,
    pub total: i64,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct TransactionItemResponse {
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub amount: i32,
    pub balance_after: i32,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path = "/api/credits/transactions",
    params(TransactionQuery),
    responses(
        (status = 200, description = "Transaction history", body = CreditTransactionsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn get_credit_transactions_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
    Query(query): Query<TransactionQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let input = GetCreditTransactionsInput {
        line_user_id: liff.line_user_id,
        limit: query.limit.clamp(1, 100),
        offset: query.offset.max(0),
    };

    let output = state.get_credit_transactions_usecase.execute(input).await?;

    let transactions = output
        .transactions
        .into_iter()
        .map(|tx| TransactionItemResponse {
            transaction_type: tx.transaction_type,
            amount: tx.amount,
            balance_after: tx.balance_after,
            description: tx.description,
            created_at: tx.created_at,
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(CreditTransactionsResponse {
            transactions,
            total: output.total,
        }),
    ))
}

// ── Get Tags ─────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct TagsResponse {
    pub appearance: Vec<TagItemResponse>,
    pub personality: Vec<TagItemResponse>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct TagItemResponse {
    pub key: String,
    pub display_name: String,
    pub description: Option<String>,
}

impl From<TagItem> for TagItemResponse {
    fn from(item: TagItem) -> Self {
        Self {
            key: item.key,
            display_name: item.display_name,
            description: item.description,
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/tags",
    responses(
        (status = 200, description = "Tag definitions grouped by category", body = TagsResponse),
    )
)]
pub(crate) async fn get_tags_handler(
    _liff: LiffAuth,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let output = state.get_tags_usecase.execute().await?;

    Ok((
        StatusCode::OK,
        Json(TagsResponse {
            appearance: output
                .appearance
                .into_iter()
                .map(TagItemResponse::from)
                .collect(),
            personality: output
                .personality
                .into_iter()
                .map(TagItemResponse::from)
                .collect(),
        }),
    ))
}

// ── Get Scenes ────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct ScenesResponse {
    pub scenes: Vec<SceneItemResponse>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct SceneItemResponse {
    pub id: Uuid,
    pub name: String,
    pub location: String,
    pub time_of_day: String,
    pub atmosphere_summary: String,
    pub opening_narrator: String,
    pub opening_dialogue: String,
    pub start_mood: String,
    pub image_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub character: SceneCharacterResponse,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct SceneCharacterResponse {
    pub id: Uuid,
    pub name: String,
    pub gender: String,
    pub avatar_url: Option<String>,
    pub appearance_tags: Vec<String>,
    pub personality_tags: Vec<String>,
    pub personality: String,
    pub background: String,
}

impl From<SceneItem> for SceneItemResponse {
    fn from(item: SceneItem) -> Self {
        Self {
            id: item.id,
            name: item.name,
            location: item.location,
            time_of_day: item.time_of_day,
            atmosphere_summary: item.atmosphere_summary,
            opening_narrator: item.opening_narrator,
            opening_dialogue: item.opening_dialogue,
            start_mood: item.start_mood,
            image_url: item.image_url,
            created_at: item.created_at,
            character: SceneCharacterResponse::from(item.character),
        }
    }
}

impl From<SceneCharacterItem> for SceneCharacterResponse {
    fn from(item: SceneCharacterItem) -> Self {
        Self {
            id: item.id,
            name: item.name,
            gender: item.gender,
            avatar_url: item.avatar_url,
            appearance_tags: item.appearance_tags,
            personality_tags: item.personality_tags,
            personality: item.personality,
            background: item.background,
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/scenes",
    responses(
        (status = 200, description = "All active scenes with character metadata", body = ScenesResponse),
    )
)]
pub(crate) async fn get_scenes_handler(
    _liff: LiffAuth,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let output = state.get_scenes_usecase.execute().await?;

    let scenes = output
        .scenes
        .into_iter()
        .map(SceneItemResponse::from)
        .collect();

    Ok((StatusCode::OK, Json(ScenesResponse { scenes })))
}

// ── Create Payment ────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub(crate) struct CreatePaymentRequest {
    package_id: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CreatePaymentResponse {
    pub payment_url: String,
    pub order_id: String,
}

#[utoipa::path(
    post,
    path = "/api/payments/create",
    request_body = CreatePaymentRequest,
    responses(
        (status = 200, description = "Payment link created", body = CreatePaymentResponse),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn create_payment_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
    Json(body): Json<CreatePaymentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let input = CreatePaymentInput {
        line_user_id: liff.line_user_id,
        package_id: body.package_id,
        redirect_base_url: state.config.line.liff_base_url.clone(),
    };

    let output = state.create_payment_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(CreatePaymentResponse {
            payment_url: output.payment_url,
            order_id: output.order_id,
        }),
    ))
}

// ── Get Payment Status ───────────────────────────────────

#[derive(Deserialize, IntoParams)]
pub(crate) struct PaymentStatusQuery {
    pub order_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct PaymentStatusResponse {
    pub status: String,
    pub credits_amount: i32,
    pub price_thb: i32,
}

#[utoipa::path(
    get,
    path = "/api/payments/status",
    params(PaymentStatusQuery),
    responses(
        (status = 200, description = "Payment order status", body = PaymentStatusResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Order not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub(crate) async fn get_payment_status_handler(
    State(state): State<AppState>,
    liff: LiffAuth,
    Query(query): Query<PaymentStatusQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let input = GetPaymentStatusInput {
        line_user_id: liff.line_user_id,
        order_id: query.order_id,
    };

    let output = state.get_payment_status_usecase.execute(input).await?;

    Ok((
        StatusCode::OK,
        Json(PaymentStatusResponse {
            status: output.status,
            credits_amount: output.credits_amount,
            price_thb: output.price_thb,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_response_serializes_to_expected_json() {
        let response = TagsResponse {
            appearance: vec![
                TagItemResponse {
                    key: "cute".to_string(),
                    display_name: "น่ารัก".to_string(),
                    description: Some("หน้าอ่อนหวาน ตาโต ดูน่าเอ็นดู".to_string()),
                },
                TagItemResponse {
                    key: "cool".to_string(),
                    display_name: "เท่".to_string(),
                    description: None,
                },
            ],
            personality: vec![TagItemResponse {
                key: "tsundere".to_string(),
                display_name: "ซึนเดเระ".to_string(),
                description: Some("ภายนอกเย็นชา แต่ข้างในอ่อนโยน".to_string()),
            }],
        };

        let json = serde_json::to_value(&response).unwrap();

        // Top-level keys
        assert!(json.get("appearance").is_some());
        assert!(json.get("personality").is_some());

        // Appearance array
        let appearance = json["appearance"].as_array().unwrap();
        assert_eq!(appearance.len(), 2);
        assert_eq!(appearance[0]["key"], "cute");
        assert_eq!(appearance[0]["display_name"], "น่ารัก");
        assert_eq!(appearance[0]["description"], "หน้าอ่อนหวาน ตาโต ดูน่าเอ็นดู");
        assert_eq!(appearance[1]["key"], "cool");
        assert!(appearance[1]["description"].is_null());

        // Personality array
        let personality = json["personality"].as_array().unwrap();
        assert_eq!(personality.len(), 1);
        assert_eq!(personality[0]["key"], "tsundere");
        assert_eq!(personality[0]["display_name"], "ซึนเดเระ");
    }

    #[test]
    fn tags_response_empty_categories() {
        let response = TagsResponse {
            appearance: vec![],
            personality: vec![],
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["appearance"].as_array().unwrap().len(), 0);
        assert_eq!(json["personality"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn tag_item_response_from_usecase_tag_item() {
        let item = TagItem {
            key: "elegant".to_string(),
            display_name: "สง่างาม".to_string(),
            description: Some("มีออร่า ดูมีระดับ".to_string()),
        };

        let response: TagItemResponse = item.into();
        assert_eq!(response.key, "elegant");
        assert_eq!(response.display_name, "สง่างาม");
        assert_eq!(response.description.as_deref(), Some("มีออร่า ดูมีระดับ"));
    }

    #[test]
    fn tag_item_response_from_usecase_tag_item_no_description() {
        let item = TagItem {
            key: "sporty".to_string(),
            display_name: "สปอร์ตี้".to_string(),
            description: None,
        };

        let response: TagItemResponse = item.into();
        assert_eq!(response.key, "sporty");
        assert_eq!(response.display_name, "สปอร์ตี้");
        assert!(response.description.is_none());
    }

    #[test]
    fn tags_response_has_no_extra_fields() {
        let response = TagsResponse {
            appearance: vec![TagItemResponse {
                key: "cute".to_string(),
                display_name: "น่ารัก".to_string(),
                description: None,
            }],
            personality: vec![],
        };

        let json = serde_json::to_value(&response).unwrap();
        let obj = json.as_object().unwrap();

        // Only "appearance" and "personality" keys, nothing else
        assert_eq!(obj.len(), 2);
        assert!(obj.contains_key("appearance"));
        assert!(obj.contains_key("personality"));

        // TagItemResponse only has key, display_name, description
        let item = &json["appearance"][0];
        let item_obj = item.as_object().unwrap();
        assert_eq!(item_obj.len(), 3);
        assert!(item_obj.contains_key("key"));
        assert!(item_obj.contains_key("display_name"));
        assert!(item_obj.contains_key("description"));
    }
}
