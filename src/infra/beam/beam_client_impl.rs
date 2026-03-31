use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::domain::services::beam_client::{
    BeamClient, CreatePaymentLinkInput, CreatePaymentLinkOutput,
};
use crate::domain::services::beam_client_error::BeamClientError;

const BEAM_API_BASE: &str = "https://api.beamcheckout.com";

pub struct BeamClientImpl {
    client: Client,
    auth_header: String,
}

impl BeamClientImpl {
    pub fn new(merchant_id: String, api_key: String) -> Self {
        let credentials = format!("{}:{}", merchant_id, api_key);
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
        let auth_header = format!("Basic {}", encoded);

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to build Beam HTTP client");

        Self {
            client,
            auth_header,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatePaymentLinkRequest {
    order: PaymentLinkOrder,
    #[serde(skip_serializing_if = "Option::is_none")]
    redirect_url: Option<String>,
    link_settings: LinkSettings,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LinkSettings {
    qr_prompt_pay: PaymentMethodSetting,
    card: PaymentMethodSetting,
    mobile_banking: PaymentMethodSetting,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaymentMethodSetting {
    is_enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaymentLinkOrder {
    currency: String,
    net_amount: i64,
    description: String,
    reference_id: String,
}

#[derive(Deserialize)]
struct CreatePaymentLinkResponse {
    id: String,
    url: String,
}

#[derive(Deserialize)]
struct BeamErrorResponse {
    message: Option<String>,
    error: Option<BeamErrorDetail>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BeamErrorDetail {
    error_message: Option<String>,
}

#[async_trait]
impl BeamClient for BeamClientImpl {
    async fn create_payment_link(
        &self,
        input: CreatePaymentLinkInput,
    ) -> Result<CreatePaymentLinkOutput, BeamClientError> {
        let body = CreatePaymentLinkRequest {
            order: PaymentLinkOrder {
                currency: "THB".to_string(),
                net_amount: input.amount_satang,
                description: input.description,
                reference_id: input.reference_id,
            },
            redirect_url: input.redirect_url,
            link_settings: LinkSettings {
                qr_prompt_pay: PaymentMethodSetting { is_enabled: true },
                card: PaymentMethodSetting { is_enabled: true },
                mobile_banking: PaymentMethodSetting { is_enabled: true },
            },
        };

        let reference_id = body.order.reference_id.clone();

        tracing::info!(
            reference_id = %reference_id,
            amount_satang = body.order.net_amount,
            "Sending payment link request to Beam"
        );

        let response = self
            .client
            .post(format!("{}/api/v1/payment-links", BEAM_API_BASE))
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(
                    reference_id = %reference_id,
                    error = %e,
                    "Beam API network error"
                );
                BeamClientError::RequestFailed(e.to_string())
            })?;

        let status = response.status().as_u16();

        if status != 201 {
            let error_body =
                response
                    .json::<BeamErrorResponse>()
                    .await
                    .unwrap_or(BeamErrorResponse {
                        message: Some("Unknown error".to_string()),
                        error: None,
                    });

            let message = error_body
                .error
                .and_then(|e| e.error_message)
                .or(error_body.message)
                .unwrap_or_else(|| "Unknown Beam API error".to_string());

            tracing::error!(
                status,
                reference_id = %reference_id,
                error_message = %message,
                "Beam API error response"
            );

            return Err(BeamClientError::ApiError { status, message });
        }

        let result = response
            .json::<CreatePaymentLinkResponse>()
            .await
            .map_err(|e| {
                tracing::error!(
                    reference_id = %reference_id,
                    error = %e,
                    "Failed to parse Beam API success response"
                );
                BeamClientError::ParseError(e.to_string())
            })?;

        tracing::info!(
            reference_id = %reference_id,
            payment_link_id = %result.id,
            "Beam payment link created successfully"
        );

        Ok(CreatePaymentLinkOutput {
            payment_link_id: result.id,
            url: result.url,
        })
    }
}
