use async_trait::async_trait;

use crate::domain::services::beam_client_error::BeamClientError;

pub struct CreatePaymentLinkInput {
    pub amount_satang: i64,
    pub description: String,
    pub reference_id: String,
    pub redirect_url: Option<String>,
}

pub struct CreatePaymentLinkOutput {
    pub payment_link_id: String,
    pub url: String,
}

#[async_trait]
pub trait BeamClient: Send + Sync {
    async fn create_payment_link(
        &self,
        input: CreatePaymentLinkInput,
    ) -> Result<CreatePaymentLinkOutput, BeamClientError>;
}
