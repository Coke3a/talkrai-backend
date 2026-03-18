use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::domain::repositories::{CreditRepository, UserRepository};
use crate::usecases::UsecaseError;

pub struct GetCreditTransactionsInput {
    pub line_user_id: String,
    pub limit: i64,
    pub offset: i64,
}

pub struct TransactionItem {
    pub transaction_type: String,
    pub amount: i32,
    pub balance_after: i32,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct GetCreditTransactionsOutput {
    pub transactions: Vec<TransactionItem>,
    pub total: i64,
}

pub struct GetCreditTransactionsUseCase {
    user_repo: Arc<dyn UserRepository>,
    credit_repo: Arc<dyn CreditRepository>,
}

impl GetCreditTransactionsUseCase {
    pub fn new(user_repo: Arc<dyn UserRepository>, credit_repo: Arc<dyn CreditRepository>) -> Self {
        Self {
            user_repo,
            credit_repo,
        }
    }

    pub async fn execute(
        &self,
        input: GetCreditTransactionsInput,
    ) -> Result<GetCreditTransactionsOutput, UsecaseError> {
        let user = self
            .user_repo
            .find_by_line_user_id(&input.line_user_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("User not found".to_string()))?;

        let (transactions, total) = self
            .credit_repo
            .find_transactions_by_user_id(user.id(), input.limit, input.offset)
            .await?;

        let items = transactions
            .into_iter()
            .map(|tx| TransactionItem {
                transaction_type: tx.transaction_type().as_str().to_string(),
                amount: tx.amount(),
                balance_after: tx.balance_after(),
                description: tx.description().map(|s| s.to_string()),
                created_at: *tx.created_at(),
            })
            .collect();

        Ok(GetCreditTransactionsOutput {
            transactions: items,
            total,
        })
    }
}
