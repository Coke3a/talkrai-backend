use super::stories::validate_key;
use crate::domain::{
    services::beam_client::{BeamClient, CreatePaymentLinkInput},
    web::{WebError, WebPaymentRepository},
};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;
pub struct WebPayments {
    pub repo: Arc<dyn WebPaymentRepository>,
    pub beam: Arc<dyn BeamClient>,
    pub origin: String,
}
impl WebPayments {
    pub async fn create(&self, owner: Uuid, key: &str, package: &str) -> Result<Value, WebError> {
        validate_key(key)?;
        if !matches!(package, "basic" | "plus" | "premium") {
            return Err(WebError::Rejected("VALIDATION_ERROR"));
        }
        let order = self.repo.create(owner, key, package).await?;
        let id: Uuid =
            serde_json::from_value(order["order_id"].clone()).map_err(anyhow::Error::from)?;
        if order["is_new"] != true {
            if order["payment_url"].is_null() {
                return Err(WebError::Rejected("PAYMENT_IN_PROGRESS"));
            }
            return Ok(json!({"order_id":id,"payment_url":order["payment_url"]}));
        }
        let link = self
            .beam
            .create_payment_link(CreatePaymentLinkInput {
                amount_satang: order["amount_thb"]
                    .as_i64()
                    .ok_or(WebError::Rejected("INTERNAL_ERROR"))?
                    * 100,
                description: format!("TalkrAI {} เครดิต", order["credits"]),
                reference_id: id.to_string(),
                redirect_url: Some(format!("{}/credits?order_id={id}", self.origin)),
            })
            .await
            .map_err(anyhow::Error::from)?;
        self.repo
            .attach_link(id, &link.payment_link_id, &link.url)
            .await?;
        Ok(json!({"order_id":id,"payment_url":link.url}))
    }
}
