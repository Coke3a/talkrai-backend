use std::sync::Arc;

use uuid::Uuid;

use crate::domain::repositories::{PaymentOrderRepository, UserRepository};
use crate::domain::value_objects::PaymentOrderId;
use crate::usecases::UsecaseError;

pub struct GetPaymentStatusInput {
    pub line_user_id: String,
    pub order_id: Uuid,
}

pub struct GetPaymentStatusOutput {
    pub status: String,
    pub credits_amount: i32,
    pub price_thb: i32,
}

pub struct GetPaymentStatusUseCase {
    user_repo: Arc<dyn UserRepository>,
    payment_order_repo: Arc<dyn PaymentOrderRepository>,
}

impl GetPaymentStatusUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        payment_order_repo: Arc<dyn PaymentOrderRepository>,
    ) -> Self {
        Self {
            user_repo,
            payment_order_repo,
        }
    }

    pub async fn execute(
        &self,
        input: GetPaymentStatusInput,
    ) -> Result<GetPaymentStatusOutput, UsecaseError> {
        // 1. Find user
        let user = self
            .user_repo
            .find_by_line_user_id(&input.line_user_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("User not found".into()))?;

        // 2. Find payment order
        let order_id = PaymentOrderId::from_uuid(input.order_id);
        let order = self
            .payment_order_repo
            .find_by_id(&order_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Payment order not found".into()))?;

        // 3. Verify ownership
        if order.user_id().as_uuid() != user.id().as_uuid() {
            return Err(UsecaseError::NotFound("Payment order not found".into()));
        }

        Ok(GetPaymentStatusOutput {
            status: order.status().as_str().to_string(),
            credits_amount: order.credits_amount(),
            price_thb: order.price_thb(),
        })
    }
}
