use async_trait::async_trait;

use crate::domain::entities::{CreditBalance, CreditTransaction};
use crate::domain::repositories::RepoError;
use crate::domain::value_objects::UserId;

#[async_trait]
pub trait CreditRepository: Send + Sync {
    async fn find_balance_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<CreditBalance>, RepoError>;
    async fn create_balance(&self, balance: &CreditBalance) -> Result<(), RepoError>;

    /// Atomic: deduct balance + insert transaction in one DB transaction
    async fn deduct_and_log(
        &self,
        user_id: &UserId,
        amount: i32,
        transaction: &CreditTransaction,
    ) -> Result<(), RepoError>;
}
