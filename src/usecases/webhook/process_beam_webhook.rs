use std::sync::Arc;

use base64::Engine;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

use crate::domain::repositories::PaymentOrderRepository;
use crate::domain::value_objects::PaymentOrderStatus;
use crate::usecases::UsecaseError;

type HmacSha256 = Hmac<Sha256>;

/// Payload for `payment_link.paid` — body is a PaymentLink object.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaymentLinkPaidPayload {
    payment_link_id: String,
    status: String,
}

/// Payload for `charge.succeeded` — body is a Charge object.
/// The payment link ID is in `source_id` when `source` == "PAYMENT_LINK".
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChargeSucceededPayload {
    status: String,
    source: Option<String>,
    source_id: Option<String>,
}

pub struct ProcessBeamWebhookUseCase {
    payment_order_repo: Arc<dyn PaymentOrderRepository>,
    hmac_key_base64: String,
}

impl ProcessBeamWebhookUseCase {
    pub fn new(
        payment_order_repo: Arc<dyn PaymentOrderRepository>,
        hmac_key_base64: String,
    ) -> Self {
        Self {
            payment_order_repo,
            hmac_key_base64,
        }
    }

    pub fn verify_signature(&self, body: &[u8], signature: &str) -> Result<bool, UsecaseError> {
        let key_bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.hmac_key_base64)
            .map_err(|e| {
                UsecaseError::Validation(format!("Invalid HMAC key configuration: {}", e))
            })?;

        let mut mac = HmacSha256::new_from_slice(&key_bytes)
            .map_err(|e| UsecaseError::Validation(format!("HMAC key error: {}", e)))?;

        mac.update(body);
        let supplied = base64::engine::general_purpose::STANDARD
            .decode(signature)
            .map_err(|_| UsecaseError::Validation("Invalid webhook signature".into()))?;
        Ok(mac.verify_slice(&supplied).is_ok())
    }

    pub async fn process_event(&self, event_type: &str, body: &[u8]) -> Result<(), UsecaseError> {
        let (payment_link_id, beam_status) = match event_type {
            "payment_link.paid" => {
                let payload: PaymentLinkPaidPayload =
                    serde_json::from_slice(body).map_err(|e| {
                        let body_preview = String::from_utf8_lossy(&body[..body.len().min(500)]);
                        tracing::error!(
                            error = %e,
                            event_type,
                            body_preview = %body_preview,
                            "Failed to parse payment_link.paid payload"
                        );
                        UsecaseError::Validation(format!("Invalid webhook payload: {}", e))
                    })?;
                (payload.payment_link_id, payload.status)
            }
            "charge.succeeded" => {
                let payload: ChargeSucceededPayload =
                    serde_json::from_slice(body).map_err(|e| {
                        let body_preview = String::from_utf8_lossy(&body[..body.len().min(500)]);
                        tracing::error!(
                            error = %e,
                            event_type,
                            body_preview = %body_preview,
                            "Failed to parse charge.succeeded payload"
                        );
                        UsecaseError::Validation(format!("Invalid webhook payload: {}", e))
                    })?;

                // Only process charges originating from payment links
                match (payload.source.as_deref(), payload.source_id) {
                    (Some("PAYMENT_LINK"), Some(id)) => (id, payload.status),
                    _ => {
                        tracing::info!(
                            event_type,
                            "Ignoring charge.succeeded not from a payment link"
                        );
                        return Ok(());
                    }
                }
            }
            _ => {
                tracing::info!(event_type, "Ignoring non-payment webhook event");
                return Ok(());
            }
        };

        tracing::info!(
            payment_link_id = %payment_link_id,
            beam_status = %beam_status,
            event_type,
            "Processing Beam payment webhook"
        );

        // 1. Find payment order
        let order = self
            .payment_order_repo
            .find_by_beam_payment_link_id(&payment_link_id)
            .await?
            .ok_or_else(|| {
                UsecaseError::NotFound(format!(
                    "Payment order not found for beam_id: {}",
                    payment_link_id
                ))
            })?;

        // 2. Idempotency guard — already completed
        if *order.status() == PaymentOrderStatus::Completed {
            tracing::info!(
                order_id = %order.id(),
                "Payment order already completed, skipping"
            );
            return Ok(());
        }

        self.payment_order_repo
            .settle_and_credit(order.id(), &beam_status)
            .await?;

        Ok(())
    }
}
