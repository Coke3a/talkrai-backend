use std::sync::Arc;

use crate::domain::repositories::{CreditRepository, UserRepository};
use crate::usecases::UsecaseError;

pub struct GetCreditBalanceInput {
    pub line_user_id: String,
}

pub struct GetCreditBalanceOutput {
    pub balance: i32,
    pub total_purchased: i32,
    pub total_consumed: i32,
}

pub struct GetCreditBalanceUseCase {
    user_repo: Arc<dyn UserRepository>,
    credit_repo: Arc<dyn CreditRepository>,
}

impl GetCreditBalanceUseCase {
    pub fn new(user_repo: Arc<dyn UserRepository>, credit_repo: Arc<dyn CreditRepository>) -> Self {
        Self {
            user_repo,
            credit_repo,
        }
    }

    pub async fn execute(
        &self,
        input: GetCreditBalanceInput,
    ) -> Result<GetCreditBalanceOutput, UsecaseError> {
        let user = self
            .user_repo
            .find_by_line_user_id(&input.line_user_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("User not found".to_string()))?;

        let balance = self
            .credit_repo
            .find_balance_by_user_id(user.id())
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Credit balance not found".to_string()))?;

        Ok(GetCreditBalanceOutput {
            balance: balance.balance(),
            total_purchased: balance.total_purchased(),
            total_consumed: balance.total_consumed(),
        })
    }
}
