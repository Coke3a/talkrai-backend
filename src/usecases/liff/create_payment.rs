use std::sync::Arc;

use crate::domain::entities::PaymentOrder;
use crate::domain::repositories::{PaymentOrderRepository, UserRepository};
use crate::domain::services::beam_client::{BeamClient, CreatePaymentLinkInput};
use crate::usecases::liff::require_active_user::require_active_user;
use crate::usecases::UsecaseError;

struct PackageInfo {
    credits: i32,
    price_thb: i32,
}

fn get_package(package_id: &str) -> Option<PackageInfo> {
    match package_id {
        "basic" => Some(PackageInfo {
            credits: 50,
            price_thb: 29,
        }),
        "plus" => Some(PackageInfo {
            credits: 150,
            price_thb: 69,
        }),
        "premium" => Some(PackageInfo {
            credits: 400,
            price_thb: 149,
        }),
        _ => None,
    }
}

pub struct CreatePaymentInput {
    pub line_user_id: String,
    pub package_id: String,
    pub redirect_base_url: String,
}

pub struct CreatePaymentOutput {
    pub payment_url: String,
    pub order_id: String,
}

pub struct CreatePaymentUseCase {
    user_repo: Arc<dyn UserRepository>,
    payment_order_repo: Arc<dyn PaymentOrderRepository>,
    beam_client: Arc<dyn BeamClient>,
}

impl CreatePaymentUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        payment_order_repo: Arc<dyn PaymentOrderRepository>,
        beam_client: Arc<dyn BeamClient>,
    ) -> Self {
        Self {
            user_repo,
            payment_order_repo,
            beam_client,
        }
    }

    pub async fn execute(
        &self,
        input: CreatePaymentInput,
    ) -> Result<CreatePaymentOutput, UsecaseError> {
        // 1. Validate package
        let package = get_package(&input.package_id).ok_or_else(|| {
            UsecaseError::Validation(format!("Invalid package_id: {}", input.package_id))
        })?;

        // 2. Find user
        let user = require_active_user(&*self.user_repo, &input.line_user_id).await?;

        // 3. Create payment order
        let order = PaymentOrder::new(
            user.id().clone(),
            input.package_id.clone(),
            package.credits,
            package.price_thb,
            None, // redirect_url set after we have the order ID
        );

        let order_id = order.id_uuid().to_string();

        let final_redirect_url = format!(
            "{}/credits/pending?id={}",
            input.redirect_base_url.trim_end_matches('/'),
            order_id
        );

        // 4. Save order to DB
        self.payment_order_repo.create(&order).await?;

        // 5. Call Beam API
        let beam_input = CreatePaymentLinkInput {
            amount_satang: (package.price_thb as i64) * 100,
            description: format!("TalkrAI - {} เครดิต", package.credits),
            reference_id: order_id.clone(),
            redirect_url: Some(final_redirect_url),
        };

        tracing::info!(
            order_id = %order_id,
            user_id = %user.id().as_uuid(),
            package_id = %input.package_id,
            amount_satang = beam_input.amount_satang,
            "Creating Beam payment link"
        );

        let beam_output = self
            .beam_client
            .create_payment_link(beam_input)
            .await
            .map_err(|e| {
                tracing::error!(
                    order_id = %order_id,
                    user_id = %user.id().as_uuid(),
                    package_id = %input.package_id,
                    error = %e,
                    "Beam payment link creation failed"
                );
                UsecaseError::from(e)
            })?;

        // 6. Update order with Beam payment link ID
        self.payment_order_repo
            .update_beam_payment_link_id(order.id(), &beam_output.payment_link_id)
            .await?;

        Ok(CreatePaymentOutput {
            payment_url: beam_output.url,
            order_id,
        })
    }
}
