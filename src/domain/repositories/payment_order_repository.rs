use async_trait::async_trait;

use crate::domain::entities::PaymentOrder;
use crate::domain::repositories::RepoError;
use crate::domain::value_objects::{PaymentOrderId, PaymentOrderStatus};

#[async_trait]
pub trait PaymentOrderRepository: Send + Sync {
    async fn create(&self, order: &PaymentOrder) -> Result<(), RepoError>;

    async fn find_by_id(&self, id: &PaymentOrderId) -> Result<Option<PaymentOrder>, RepoError>;

    async fn find_by_beam_payment_link_id(
        &self,
        beam_id: &str,
    ) -> Result<Option<PaymentOrder>, RepoError>;

    async fn update_status(
        &self,
        id: &PaymentOrderId,
        status: &PaymentOrderStatus,
        beam_status: Option<&str>,
    ) -> Result<(), RepoError>;

    async fn update_beam_payment_link_id(
        &self,
        id: &PaymentOrderId,
        beam_payment_link_id: &str,
    ) -> Result<(), RepoError>;
}
