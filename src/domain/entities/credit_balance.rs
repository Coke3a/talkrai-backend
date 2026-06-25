use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::entities::CreditTransaction;
use crate::domain::error::DomainError;
use crate::domain::value_objects::{CreditTransactionType, UserId};

pub struct CreditBalance {
    id: Uuid,
    user_id: UserId,
    balance: i32,
    total_purchased: i32,
    total_consumed: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl CreditBalance {
    pub fn new(user_id: UserId, initial_balance: i32) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id,
            balance: initial_balance,
            total_purchased: 0,
            total_consumed: 0,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn from_existing(
        id: Uuid,
        user_id: UserId,
        balance: i32,
        total_purchased: i32,
        total_consumed: i32,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            user_id,
            balance,
            total_purchased,
            total_consumed,
            created_at,
            updated_at,
        }
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn balance(&self) -> i32 {
        self.balance
    }

    pub fn total_purchased(&self) -> i32 {
        self.total_purchased
    }

    pub fn total_consumed(&self) -> i32 {
        self.total_consumed
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }

    pub fn has_sufficient_credits(&self, amount: i32) -> bool {
        self.balance >= amount
    }

    /// Deduct credits — returns CreditTransaction for logging
    pub fn deduct(
        &mut self,
        amount: i32,
        reference_id: Option<Uuid>,
    ) -> Result<CreditTransaction, DomainError> {
        if amount <= 0 {
            return Err(DomainError::InvalidField {
                field: "amount",
                reason: "deduction amount must be positive",
            });
        }
        if !self.has_sufficient_credits(amount) {
            return Err(DomainError::InsufficientCredits);
        }
        self.balance -= amount;
        self.total_consumed += amount;
        self.updated_at = Utc::now();

        Ok(CreditTransaction::new(
            self.user_id.clone(),
            CreditTransactionType::Consumption,
            -amount,
            self.balance,
            reference_id,
            Some("Roleplay message credit".to_string()),
        ))
    }

    /// Add credits (grant) — returns the CreditTransaction for logging. Mirrors `deduct` so the
    /// in-memory balance stays consistent with what `CreditRepository::add_and_log` writes to the
    /// DB, letting a downstream credit check see a just-granted top-up without a re-fetch.
    pub fn add(
        &mut self,
        amount: i32,
        transaction_type: CreditTransactionType,
        reference_id: Option<Uuid>,
        description: Option<String>,
    ) -> Result<CreditTransaction, DomainError> {
        if amount <= 0 {
            return Err(DomainError::InvalidField {
                field: "amount",
                reason: "credit amount must be positive",
            });
        }
        self.balance += amount;
        // total_purchased counts paid credits only — bonus/refund/adjustment leave it untouched.
        if transaction_type == CreditTransactionType::Purchase {
            self.total_purchased += amount;
        }
        self.updated_at = Utc::now();

        Ok(CreditTransaction::new(
            self.user_id.clone(),
            transaction_type,
            amount,
            self.balance,
            reference_id,
            description,
        ))
    }
}
