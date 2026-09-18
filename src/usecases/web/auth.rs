use crate::domain::web::{AuthFlow, IdentityProvider, WebError, WebPrincipal, WebRepository};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

pub fn hash_token(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
}
pub fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}
pub fn safe_return_to(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && !path.contains(['\\', '\r', '\n'])
        && !path.contains('%')
        && path.len() <= 1024
}
pub struct WebAuth {
    pub repo: Arc<dyn WebRepository>,
    pub provider: Arc<dyn IdentityProvider>,
}
impl WebAuth {
    pub async fn start(
        &self,
        provider: &str,
        return_to: &str,
        principal: Option<WebPrincipal>,
        acquisition_source: &str,
    ) -> Result<(String, String), WebError> {
        if !safe_return_to(return_to) {
            return Err(WebError::Rejected("VALIDATION_ERROR"));
        }
        let state = random_token();
        let nonce = random_token();
        let verifier = random_token();
        let browser = random_token();
        let url = self
            .provider
            .authorization_url(provider, &state, &nonce, &verifier, principal.is_some())
            .await?;
        self.repo
            .save_flow(&AuthFlow {
                acquisition_source: match acquisition_source {
                    "khui" | "google" | "line" | "content" | "direct" => acquisition_source,
                    _ => "other",
                }
                .to_owned(),
                state_hash: hash_token(&state),
                browser_hash: hash_token(&browser),
                provider: provider.to_owned(),
                nonce,
                pkce_verifier: verifier,
                return_to: return_to.to_owned(),
                link_user_id: principal.as_ref().map(|p| p.user_id),
                link_session_hash: principal.map(|p| p.token_hash),
            })
            .await?;
        Ok((url, browser))
    }
    pub async fn callback(
        &self,
        provider: &str,
        code: &str,
        state: &str,
        browser: &str,
        current_session: Option<&str>,
        allow_registration: bool,
    ) -> Result<(String, String), WebError> {
        let flow = self
            .repo
            .consume_flow(&hash_token(state), &hash_token(browser), provider)
            .await
            .inspect_err(|_| tracing::warn!(provider, stage = "flow_lookup", "Web login failed"))?;
        if let Some(required) = &flow.link_session_hash {
            if current_session.map(hash_token).as_ref() != Some(required) {
                return Err(WebError::Rejected("INVALID_AUTH_FLOW"));
            }
        }
        let identity = self
            .provider
            .verify(provider, code, &flow.nonce, &flow.pkce_verifier)
            .await
            .inspect_err(|_| {
                tracing::warn!(
                    provider,
                    stage = "provider_verification",
                    "Web login failed"
                )
            })?;
        let token = random_token();
        let csrf = random_token();
        self.repo
            .login(
                &identity,
                &flow,
                &hash_token(&token),
                &csrf,
                allow_registration,
            )
            .await.inspect_err(|error| tracing::warn!(provider, stage="account_resolution", error=%error, "Web login failed"))?;
        Ok((token, flow.return_to))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn returns_are_local_paths() {
        for path in [
            "//evil.com",
            "/\\evil.com",
            "/%2fevil.com",
            "https://evil.com",
            "/a\nb",
        ] {
            assert!(!safe_return_to(path));
        }
        assert!(safe_return_to("/stories/123"));
    }
}
