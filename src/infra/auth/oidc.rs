use crate::domain::web::{Identity, IdentityProvider, WebError};
use async_trait::async_trait;
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    AuthType, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
};

pub struct OidcProviders {
    pub origin: String,
    pub google_id: String,
    pub google_secret: String,
    pub line_id: String,
    pub line_secret: String,
    pub http: reqwest::Client,
}
impl OidcProviders {
    fn settings(&self, provider: &str) -> Result<(&str, &str, &str), WebError> {
        match provider {
            "google" => Ok((
                "https://accounts.google.com",
                &self.google_id,
                &self.google_secret,
            )),
            "line" => Ok(("https://access.line.me", &self.line_id, &self.line_secret)),
            _ => Err(WebError::Rejected("VALIDATION_ERROR")),
        }
    }
}
#[async_trait]
impl IdentityProvider for OidcProviders {
    async fn authorization_url(
        &self,
        provider: &str,
        state: &str,
        nonce: &str,
        verifier: &str,
        linking: bool,
    ) -> Result<String, WebError> {
        let (issuer, id, secret) = self.settings(provider)?;
        let issuer = IssuerUrl::new(issuer.to_owned()).map_err(anyhow::Error::from)?;
        let metadata = CoreProviderMetadata::discover_async(issuer, &self.http)
            .await
            .map_err(|_| WebError::Rejected("UPSTREAM_UNAVAILABLE"))?;
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(id.to_owned()),
            Some(ClientSecret::new(secret.to_owned())),
        )
        .set_redirect_uri(
            RedirectUrl::new(format!("{}/api/web/auth/{provider}/callback", self.origin))
                .map_err(anyhow::Error::from)?,
        );
        let state = state.to_owned();
        let nonce = nonce.to_owned();
        let mut auth = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                move || CsrfToken::new(state),
                move || Nonce::new(nonce),
            )
            .add_scope(Scope::new("profile".into()))
            .set_pkce_challenge(PkceCodeChallenge::from_code_verifier_sha256(
                &PkceCodeVerifier::new(verifier.to_owned()),
            ));
        if linking {
            auth = auth.add_extra_param(
                "prompt",
                if provider == "google" {
                    "select_account"
                } else {
                    "login"
                },
            );
        }
        Ok(auth.url().0.to_string())
    }
    async fn verify(
        &self,
        provider: &str,
        code: &str,
        nonce: &str,
        verifier: &str,
    ) -> Result<Identity, WebError> {
        let (issuer, id, secret) = self.settings(provider)?;
        let metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::new(issuer.to_owned()).map_err(anyhow::Error::from)?,
            &self.http,
        )
        .await
        .map_err(|_| WebError::Rejected("UPSTREAM_UNAVAILABLE"))?;
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(id.to_owned()),
            Some(ClientSecret::new(secret.to_owned())),
        )
        .set_redirect_uri(
            RedirectUrl::new(format!("{}/api/web/auth/{provider}/callback", self.origin))
                .map_err(anyhow::Error::from)?,
        );
        // LINE's token endpoint requires client credentials in the form body.
        // https://developers.line.biz/en/docs/line-login/integrate-pkce/
        let client = if provider == "line" {
            client.set_auth_type(AuthType::RequestBody)
        } else {
            client
        };
        let tokens = client
            .exchange_code(AuthorizationCode::new(code.to_owned()))
            .map_err(|_| WebError::Rejected("INVALID_AUTH_FLOW"))?
            .set_pkce_verifier(PkceCodeVerifier::new(verifier.to_owned()))
            .request_async(&self.http)
            .await
            .map_err(|error| {
                let category = match &error {
                    openidconnect::RequestTokenError::ServerResponse(response) => {
                        match response.error().as_ref() {
                            "invalid_client" => "invalid_client",
                            "invalid_grant" => "invalid_grant",
                            "unauthorized_client" => "unauthorized_client",
                            _ => "provider_rejected",
                        }
                    }
                    openidconnect::RequestTokenError::Request(_) => "network",
                    openidconnect::RequestTokenError::Parse(_, _) => "response_parse",
                    _ => "other",
                };
                tracing::warn!(
                    provider,
                    stage = "token_exchange",
                    category,
                    "OIDC verification failed"
                );
                WebError::Rejected("INVALID_AUTH_FLOW")
            })?;
        let claims = tokens
            .extra_fields()
            .id_token()
            .ok_or(WebError::Rejected("INVALID_AUTH_FLOW"))?
            .claims(&client.id_token_verifier(), &Nonce::new(nonce.to_owned()))
            .map_err(|error| {
                use openidconnect::ClaimsVerificationError as E;
                let category = match error {
                    E::Expired(_) => "expired",
                    E::InvalidAudience(_) => "audience",
                    E::InvalidIssuer(_) => "issuer",
                    E::InvalidNonce(_) => "nonce",
                    E::SignatureVerification(_) => "signature",
                    E::InvalidSubject(_) => "subject",
                    E::Unsupported(_) => "unsupported",
                    _ => "other",
                };
                tracing::warn!(
                    provider,
                    stage = "id_token_claims",
                    category,
                    "OIDC verification failed"
                );
                WebError::Rejected("INVALID_AUTH_FLOW")
            })?;
        Ok(Identity {
            provider: provider.to_owned(),
            issuer: issuer.to_owned(),
            subject: claims.subject().as_str().to_owned(),
            display_name: claims
                .name()
                .and_then(|names| names.get(None))
                .map(|s| s.as_str())
                .unwrap_or("นักอ่าน")
                .to_owned(),
        })
    }
}
