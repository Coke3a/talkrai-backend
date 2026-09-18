use super::runtime::WebRuntime;
use crate::{
    domain::web::{StoryMutation, WebError, WebPrincipal, WebRead},
    usecases::web::auth::hash_token,
};
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, patch, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;
pub struct Error(WebError);
impl From<WebError> for Error {
    fn from(e: WebError) -> Self {
        Self(e)
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let code = match self.0 {
            WebError::Rejected(code) => code,
            WebError::Internal(error) => {
                tracing::error!(%error,"Web request failed");
                "INTERNAL_ERROR"
            }
        };
        let status = match code {
            "AUTH_EXPIRED" => 401,
            "INVALID_AUTH_FLOW" | "VALIDATION_ERROR" => 400,
            "INSUFFICIENT_CREDITS" => 402,
            "ACCOUNT_SUSPENDED" | "TERMS_REQUIRED" | "CSRF_FAILED" => 403,
            "NOT_FOUND" => 404,
            "RATE_LIMITED" => 429,
            "UPSTREAM_UNAVAILABLE" => 502,
            "ADMISSIONS_DISABLED" => 503,
            "INTERNAL_ERROR" => 500,
            _ => 409,
        };
        (
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            [(header::CACHE_CONTROL, "private, no-store")],
            Json(json!({"error":code,"message":code})),
        )
            .into_response()
    }
}
fn runtime(state: &Arc<WebRuntime>) -> Result<Arc<WebRuntime>, Error> {
    Ok(state.clone())
}

fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then_some(value)
        })
}
async fn principal(
    web: &WebRuntime,
    headers: &HeaderMap,
    mutation: bool,
) -> Result<WebPrincipal, Error> {
    let token = cookie(headers, "__Host-talkrai").ok_or(WebError::Rejected("AUTH_EXPIRED"))?;
    let user = web.auth.repo.principal(&hash_token(token)).await?;
    if mutation
        && (headers.get(header::ORIGIN).and_then(|h| h.to_str().ok()) != Some(web.origin.as_str())
            || headers.get("x-csrf-token").and_then(|h| h.to_str().ok())
                != Some(user.csrf_token.as_str()))
    {
        return Err(WebError::Rejected("CSRF_FAILED").into());
    }
    Ok(user)
}
fn response(value: Value) -> Response {
    ([(header::CACHE_CONTROL, "private, no-store")], Json(value)).into_response()
}
fn enabled(web: &WebRuntime) -> Result<(), Error> {
    if web.enabled {
        Ok(())
    } else {
        Err(WebError::Rejected("ADMISSIONS_DISABLED").into())
    }
}
#[derive(Default, Deserialize)]
struct Page {
    query: Option<String>,
    tag: Option<String>,
    cursor: Option<Uuid>,
    before: Option<i64>,
    limit: Option<i64>,
}
impl Page {
    fn limit(&self) -> i64 {
        self.limit.unwrap_or(20).clamp(1, 50)
    }
}
#[derive(Deserialize)]
struct Start {
    source: Option<String>,
    return_to: Option<String>,
}
#[derive(Deserialize)]
struct Callback {
    code: Option<String>,
    state: Option<String>,
}
#[derive(Deserialize)]
struct Terms {
    version: String,
}
async fn normalize_rejection(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let result = next.run(request).await;
    if matches!(
        result.status(),
        StatusCode::BAD_REQUEST
            | StatusCode::UNPROCESSABLE_ENTITY
            | StatusCode::UNSUPPORTED_MEDIA_TYPE
    ) && !result
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"))
    {
        return Error(WebError::Rejected("VALIDATION_ERROR")).into_response();
    }
    result
}
pub fn router() -> Router<Arc<WebRuntime>> {
    Router::new()
        .route("/api/public/legal/{doc}", get(legal))
        .route("/api/public/scenes", get(catalog))
        .route("/api/public/scenes/{id}", get(scene))
        .route("/api/web/auth/{provider}/start", get(auth_start))
        .route("/api/web/auth/{provider}/callback", get(auth_callback))
        .route("/api/web/auth/logout", post(logout))
        .route("/api/web/identity-links/{provider}", post(link))
        .route("/api/web/me", get(me))
        .route("/api/web/terms/accept", post(terms))
        .route("/api/web/sessions", get(stories).post(create))
        .route("/api/web/sessions/{id}", get(story))
        .route("/api/web/sessions/{id}/messages", get(messages))
        .route("/api/web/sessions/{id}/resume", post(resume))
        .route("/api/web/sessions/{id}/transfer-to-web", post(transfer))
        .route("/api/web/sessions/{id}/persona", patch(persona))
        .route("/api/web/sessions/{id}/turns", post(turn))
        .route("/api/web/sessions/{id}/regenerations", post(regenerate))
        .route("/api/web/jobs/{id}", get(job))
        .route("/api/web/credits", get(credits))
        .route("/api/web/credit-transactions", get(transactions))
        .route("/api/web/payments/{id}", get(payment))
        .route("/api/web/payments", post(create_payment))
        .layer(axum::middleware::from_fn(normalize_rejection))
}
async fn catalog(
    State(state): State<Arc<WebRuntime>>,
    Query(p): Query<Page>,
) -> Result<Response, Error> {
    let web = runtime(&state)?;
    let limit = p.limit();
    Ok(response(
        web.stories
            .data
            .read(
                None,
                WebRead::Catalog {
                    query: p.query.unwrap_or_default(),
                    tag: p.tag.unwrap_or_default(),
                    cursor: p.cursor,
                    limit,
                },
            )
            .await?,
    ))
}
async fn scene(
    State(state): State<Arc<WebRuntime>>,
    Path(id): Path<Uuid>,
) -> Result<Response, Error> {
    Ok(response(
        runtime(&state)?
            .stories
            .data
            .read(None, WebRead::Scene(id))
            .await?,
    ))
}
async fn auth_start(
    State(state): State<Arc<WebRuntime>>,
    Path(provider): Path<String>,
    Query(query): Query<Start>,
) -> Result<Response, Error> {
    let web = runtime(&state)?;
    let (url, browser) = web
        .auth
        .start(
            &provider,
            query.return_to.as_deref().unwrap_or("/stories"),
            None,
            query.source.as_deref().unwrap_or("direct"),
        )
        .await?;
    let mut result = Redirect::to(&url).into_response();
    result.headers_mut().insert(
        header::SET_COOKIE,
        format!(
            "__Host-talkrai-flow={browser}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=600"
        )
        .parse()
        .map_err(|_| WebError::Rejected("INTERNAL_ERROR"))?,
    );
    result.headers_mut().insert(
        header::CACHE_CONTROL,
        "private, no-store"
            .parse()
            .map_err(|_| WebError::Rejected("INTERNAL_ERROR"))?,
    );
    Ok(result)
}
async fn auth_callback(
    State(state): State<Arc<WebRuntime>>,
    Path(provider): Path<String>,
    Query(query): Query<Callback>,
    headers: HeaderMap,
) -> Result<Response, Error> {
    let web = runtime(&state)?;
    let outcome = match (
        query.code.as_deref(),
        query.state.as_deref(),
        cookie(&headers, "__Host-talkrai-flow"),
    ) {
        (Some(code), Some(state), Some(browser)) => {
            web.auth
                .callback(
                    &provider,
                    code,
                    state,
                    browser,
                    cookie(&headers, "__Host-talkrai"),
                    web.enabled,
                )
                .await
        }
        _ => Err(WebError::Rejected("INVALID_AUTH_FLOW")),
    };
    let (token, path) = match outcome {
        Ok(result) => result,
        Err(error) => {
            let code = match error {
                WebError::Rejected("IDENTITY_ALREADY_LINKED") => "identity_conflict",
                WebError::Rejected("ADMISSIONS_DISABLED") => "admissions_disabled",
                _ => "login_failed",
            };
            let mut result =
                Redirect::to(&format!("{}/login?error={code}", web.origin)).into_response();
            result
                .headers_mut()
                .insert(header::CACHE_CONTROL, "private, no-store".parse().unwrap());
            result.headers_mut().insert(
                header::SET_COOKIE,
                "__Host-talkrai-flow=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0"
                    .parse()
                    .unwrap(),
            );
            return Ok(result);
        }
    };
    let mut result = Redirect::to(&format!("{}{path}", web.origin)).into_response();
    for value in [
        format!("__Host-talkrai={token}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=2592000"),
        "__Host-talkrai-flow=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0".into(),
    ] {
        result.headers_mut().append(
            header::SET_COOKIE,
            value
                .parse()
                .map_err(|_| WebError::Rejected("INTERNAL_ERROR"))?,
        );
    }
    result.headers_mut().insert(
        header::CACHE_CONTROL,
        "private, no-store"
            .parse()
            .map_err(|_| WebError::Rejected("INTERNAL_ERROR"))?,
    );
    Ok(result)
}
async fn link(
    State(state): State<Arc<WebRuntime>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
) -> Result<Response, Error> {
    let web = runtime(&state)?;
    let user = principal(&web, &headers, true).await?;
    let (url, browser) = web
        .auth
        .start(&provider, "/account", Some(user), "direct")
        .await?;
    let mut result = response(json!({"authorization_url":url}));
    result.headers_mut().append(
        header::SET_COOKIE,
        format!(
            "__Host-talkrai-flow={browser}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=600"
        )
        .parse()
        .map_err(|_| WebError::Rejected("INTERNAL_ERROR"))?,
    );
    Ok(result)
}
async fn logout(
    State(state): State<Arc<WebRuntime>>,
    headers: HeaderMap,
) -> Result<Response, Error> {
    let web = runtime(&state)?;
    let user = principal(&web, &headers, true).await?;
    web.auth.repo.logout(&user.token_hash).await?;
    Ok((
        StatusCode::NO_CONTENT,
        [
            (header::CACHE_CONTROL, "private, no-store"),
            (
                header::SET_COOKIE,
                "__Host-talkrai=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0",
            ),
        ],
    )
        .into_response())
}
async fn me(State(state): State<Arc<WebRuntime>>, headers: HeaderMap) -> Result<Response, Error> {
    let web = runtime(&state)?;
    let user = principal(&web, &headers, false).await?;
    let mut value = web
        .stories
        .data
        .read(Some(user.user_id), WebRead::Me)
        .await?;
    value["csrf_token"] = json!(user.csrf_token);
    value["required_terms_version"] = json!(web.terms_version);
    Ok(response(value))
}
async fn terms(
    State(state): State<Arc<WebRuntime>>,
    headers: HeaderMap,
    Json(input): Json<Terms>,
) -> Result<Response, Error> {
    let web = runtime(&state)?;
    let user = principal(&web, &headers, true).await?;
    if input.version != web.terms_version {
        return Err(WebError::Rejected("VALIDATION_ERROR").into());
    }
    Ok(response(
        web.stories
            .data
            .accept_terms(user.user_id, &input.version)
            .await?,
    ))
}
async fn stories(
    State(state): State<Arc<WebRuntime>>,
    headers: HeaderMap,
    Query(p): Query<Page>,
) -> Result<Response, Error> {
    let web = runtime(&state)?;
    let user = principal(&web, &headers, false).await?;
    Ok(response(
        web.stories
            .data
            .read(
                Some(user.user_id),
                WebRead::Stories {
                    cursor: p.cursor,
                    limit: p.limit(),
                },
            )
            .await?,
    ))
}
async fn messages(
    State(state): State<Arc<WebRuntime>>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Query(p): Query<Page>,
) -> Result<Response, Error> {
    let web = runtime(&state)?;
    let user = principal(&web, &headers, false).await?;
    Ok(response(
        web.stories
            .data
            .read(
                Some(user.user_id),
                WebRead::Messages {
                    session_id: id,
                    before: p.before,
                    limit: p.limit(),
                },
            )
            .await?,
    ))
}
macro_rules! read_id {
    ($name:ident,$variant:ident) => {
        async fn $name(
            State(state): State<Arc<WebRuntime>>,
            Path(id): Path<Uuid>,
            headers: HeaderMap,
        ) -> Result<Response, Error> {
            let web = runtime(&state)?;
            let user = principal(&web, &headers, false).await?;
            Ok(response(
                web.stories
                    .data
                    .read(Some(user.user_id), WebRead::$variant(id))
                    .await?,
            ))
        }
    };
}
read_id!(story, Story);
read_id!(job, Job);
read_id!(payment, Payment);
async fn credits(
    State(state): State<Arc<WebRuntime>>,
    headers: HeaderMap,
) -> Result<Response, Error> {
    let web = runtime(&state)?;
    let user = principal(&web, &headers, false).await?;
    Ok(response(
        web.stories
            .data
            .read(Some(user.user_id), WebRead::Credits)
            .await?,
    ))
}
async fn transactions(
    State(state): State<Arc<WebRuntime>>,
    headers: HeaderMap,
    Query(p): Query<Page>,
) -> Result<Response, Error> {
    let web = runtime(&state)?;
    let user = principal(&web, &headers, false).await?;
    Ok(response(
        web.stories
            .data
            .read(
                Some(user.user_id),
                WebRead::Transactions {
                    cursor: p.cursor,
                    limit: p.limit(),
                },
            )
            .await?,
    ))
}
fn key(headers: &HeaderMap) -> &str {
    headers
        .get("idempotency-key")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
}
async fn require_terms(web: &WebRuntime, user: &WebPrincipal) -> Result<(), Error> {
    let value = web
        .stories
        .data
        .read(Some(user.user_id), WebRead::Me)
        .await?;
    if value["terms_accepted"] != true
        || value["terms_version"].as_str() != Some(web.terms_version.as_str())
    {
        return Err(WebError::Rejected("TERMS_REQUIRED").into());
    }
    Ok(())
}
async fn create(
    State(state): State<Arc<WebRuntime>>,
    headers: HeaderMap,
    Json(input): Json<StoryMutation>,
) -> Result<Response, Error> {
    let web = runtime(&state)?;
    enabled(&web)?;
    let user = principal(&web, &headers, true).await?;
    require_terms(&web, &user).await?;
    let mut result = response(
        web.stories
            .mutate(user.user_id, "create", input, key(&headers))
            .await?,
    );
    *result.status_mut() = StatusCode::CREATED;
    Ok(result)
}
macro_rules! mutate {
    ($name:ident,$operation:literal) => {
        async fn $name(
            State(state): State<Arc<WebRuntime>>,
            Path(id): Path<Uuid>,
            headers: HeaderMap,
            Json(mut input): Json<StoryMutation>,
        ) -> Result<Response, Error> {
            let web = runtime(&state)?;
            enabled(&web)?;
            let user = principal(&web, &headers, true).await?;
            input.session_id = Some(id);
            Ok(response(
                web.stories
                    .mutate(user.user_id, $operation, input, key(&headers))
                    .await?,
            ))
        }
    };
}
mutate!(resume, "resume");
mutate!(transfer, "transfer");
mutate!(persona, "persona");
macro_rules! generation {
    ($name:ident,$kind:literal) => {
        async fn $name(
            State(state): State<Arc<WebRuntime>>,
            Path(id): Path<Uuid>,
            headers: HeaderMap,
            Json(input): Json<Value>,
        ) -> Result<Response, Error> {
            let web = runtime(&state)?;
            enabled(&web)?;
            let user = principal(&web, &headers, true).await?;
            require_terms(&web, &user).await?;
            let mut result = response(
                web.stories
                    .turn(user.user_id, id, $kind, key(&headers), input)
                    .await?,
            );
            *result.status_mut() = StatusCode::ACCEPTED;
            Ok(result)
        }
    };
}
generation!(turn, "turn");
generation!(regenerate, "regeneration");

#[derive(Deserialize)]
struct PaymentInput {
    package_id: String,
}
async fn create_payment(
    State(state): State<Arc<WebRuntime>>,
    headers: HeaderMap,
    Json(input): Json<PaymentInput>,
) -> Result<Response, Error> {
    let web = runtime(&state)?;
    let user = principal(&web, &headers, true).await?;
    Ok(response(
        web.payments
            .create(user.user_id, key(&headers), &input.package_id)
            .await?,
    ))
}

async fn legal(Path(doc): Path<String>) -> Result<Response, Error> {
    let result = crate::usecases::liff::get_legal_doc::GetLegalDocUseCase::new()
        .execute(crate::usecases::liff::get_legal_doc::GetLegalDocInput { doc })
        .map_err(|_| WebError::Rejected("NOT_FOUND"))?;
    Ok(response(
        json!({"title":result.title,"version":result.version,"updated":result.updated,"sections":result.sections.into_iter().map(|section|json!({"title":section.title,"body":section.body})).collect::<Vec<_>>()}),
    ))
}
