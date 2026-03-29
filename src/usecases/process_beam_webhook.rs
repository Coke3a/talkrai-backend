use std::sync::Arc;

use base64::Engine;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

use crate::domain::entities::CreditTransaction;
use crate::domain::repositories::{CreditRepository, PaymentOrderRepository};
use crate::domain::value_objects::{CreditTransactionType, PaymentOrderStatus};
use crate::usecases::UsecaseError;

type HmacSha256 = Hmac<Sha256>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BeamWebhookPayload {
    payment_link_id: String,
    status: String,
}

pub struct ProcessBeamWebhookUseCase {
    payment_order_repo: Arc<dyn PaymentOrderRepository>,
    credit_repo: Arc<dyn CreditRepository>,
    hmac_key_base64: String,
}

impl ProcessBeamWebhookUseCase {
    pub fn new(
        payment_order_repo: Arc<dyn PaymentOrderRepository>,
        credit_repo: Arc<dyn CreditRepository>,
        hmac_key_base64: String,
    ) -> Self {
        Self {
            payment_order_repo,
            credit_repo,
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
        let result = mac.finalize();
        let computed = base64::engine::general_purpose::STANDARD.encode(result.into_bytes());

        Ok(computed == signature)
    }

    pub async fn process_event(&self, event_type: &str, body: &[u8]) -> Result<(), UsecaseError> {
        if event_type != "payment_link.paid" {
            tracing::info!(event_type, "Ignoring non-payment webhook event");
            return Ok(());
        }

        let payload: BeamWebhookPayload = serde_json::from_slice(body)
            .map_err(|e| UsecaseError::Validation(format!("Invalid webhook payload: {}", e)))?;

        tracing::info!(
            payment_link_id = %payload.payment_link_id,
            beam_status = %payload.status,
            "Processing payment_link.paid webhook"
        );

        // 1. Find payment order
        let order = self
            .payment_order_repo
            .find_by_beam_payment_link_id(&payload.payment_link_id)
            .await?
            .ok_or_else(|| {
                UsecaseError::NotFound(format!(
                    "Payment order not found for beam_id: {}",
                    payload.payment_link_id
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

        // 3. Update order status to completed
        self.payment_order_repo
            .update_status(
                order.id(),
                &PaymentOrderStatus::Completed,
                Some(&payload.status),
            )
            .await?;

        // 4. Add credits to user balance
        let credits = order.credits_amount();
        let balance = self
            .credit_repo
            .find_balance_by_user_id(order.user_id())
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Credit balance not found".into()))?;

        let new_balance_after = balance.balance() + credits;

        let transaction = CreditTransaction::new(
            order.user_id().clone(),
            CreditTransactionType::Purchase,
            credits,
            new_balance_after,
            Some(*order.id_uuid()),
            Some(format!(
                "เติมเครดิต {} เครดิต ({})",
                credits,
                order.package_id()
            )),
        );

        self.credit_repo
            .add_and_log(order.user_id(), credits, &transaction)
            .await?;

        tracing::info!(
            order_id = %order.id(),
            user_id = %order.user_id(),
            credits = credits,
            "Credits added successfully"
        );

        Ok(())
    }
}
