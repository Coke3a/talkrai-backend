use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WebError {
    #[error("{0}")]
    Rejected(&'static str),
    #[error("Internal service error")]
    Internal(#[from] anyhow::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Persona {
    pub name: String,
    pub description: String,
}
impl Persona {
    pub fn new(name: String, description: String) -> Result<Self, WebError> {
        let name = name.trim().to_owned();
        if name.is_empty() || name.chars().count() > 80 || description.chars().count() > 1000 {
            return Err(WebError::Rejected("VALIDATION_ERROR"));
        }
        Ok(Self { name, description })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebPrincipal {
    pub user_id: Uuid,
    pub csrf_token: String,
    pub token_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub provider: String,
    pub issuer: String,
    pub subject: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthFlow {
    #[serde(default)]
    pub acquisition_source: String,
    pub state_hash: String,
    pub browser_hash: String,
    pub provider: String,
    pub nonce: String,
    pub pkce_verifier: String,
    pub return_to: String,
    pub link_user_id: Option<Uuid>,
    pub link_session_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationClaim {
    pub id: Uuid,
    pub lease_token: Uuid,
    pub input_snapshot: serde_json::Value,
}

#[async_trait::async_trait]
pub trait WebRepository: Send + Sync {
    async fn principal(&self, token_hash: &str) -> Result<WebPrincipal, WebError>;
    async fn save_flow(&self, flow: &AuthFlow) -> Result<(), WebError>;
    async fn consume_flow(
        &self,
        state_hash: &str,
        browser_hash: &str,
        provider: &str,
    ) -> Result<AuthFlow, WebError>;
    async fn login(
        &self,
        identity: &Identity,
        flow: &AuthFlow,
        token_hash: &str,
        csrf: &str,
        allow_registration: bool,
    ) -> Result<Uuid, WebError>;
    async fn logout(&self, token_hash: &str) -> Result<(), WebError>;
}

#[async_trait::async_trait]
pub trait IdentityProvider: Send + Sync {
    async fn authorization_url(
        &self,
        provider: &str,
        state: &str,
        nonce: &str,
        verifier: &str,
        linking: bool,
    ) -> Result<String, WebError>;
    async fn verify(
        &self,
        provider: &str,
        code: &str,
        nonce: &str,
        verifier: &str,
    ) -> Result<Identity, WebError>;
}

#[derive(Debug, Clone)]
pub enum WebRead {
    Catalog {
        query: String,
        tag: String,
        cursor: Option<Uuid>,
        limit: i64,
    },
    Scene(Uuid),
    Me,
    Stories {
        cursor: Option<Uuid>,
        limit: i64,
    },
    Story(Uuid),
    Messages {
        session_id: Uuid,
        before: Option<i64>,
        limit: i64,
    },
    Credits,
    Transactions {
        cursor: Option<Uuid>,
        limit: i64,
    },
    Job(Uuid),
    Payment(Uuid),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryMutation {
    pub scene_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub persona: Option<Persona>,
    pub expected_version: Option<i64>,
}
#[async_trait::async_trait]
pub trait WebDataRepository: Send + Sync {
    async fn read(
        &self,
        owner: Option<Uuid>,
        query: WebRead,
    ) -> Result<serde_json::Value, WebError>;
    async fn mutate_story(
        &self,
        owner: Uuid,
        operation: &str,
        input: StoryMutation,
        key: &str,
    ) -> Result<serde_json::Value, WebError>;
    async fn accept_terms(&self, owner: Uuid, version: &str)
        -> Result<serde_json::Value, WebError>;
}

#[async_trait::async_trait]
pub trait TurnRepository: Send + Sync {
    async fn admit(
        &self,
        owner: Uuid,
        session: Uuid,
        origin: &str,
        kind: &str,
        key: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, WebError>;
    async fn pending(&self) -> Result<Vec<Uuid>, WebError>;
    async fn claim(&self, id: Uuid) -> Result<Option<serde_json::Value>, WebError>;
    async fn save_input(
        &self,
        id: Uuid,
        lease: Uuid,
        input: serde_json::Value,
    ) -> Result<bool, WebError>;
    async fn settle(
        &self,
        id: Uuid,
        lease: Uuid,
        response: serde_json::Value,
    ) -> Result<serde_json::Value, WebError>;
    async fn fail(&self, id: Uuid, lease: Uuid) -> Result<(), WebError>;
}

#[async_trait::async_trait]
pub trait WebPaymentRepository: Send + Sync {
    async fn create(
        &self,
        owner: Uuid,
        key: &str,
        package: &str,
    ) -> Result<serde_json::Value, WebError>;
    async fn attach_link(&self, id: Uuid, link_id: &str, url: &str) -> Result<(), WebError>;
}
