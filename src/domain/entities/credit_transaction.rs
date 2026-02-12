use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value_objects::{CreditTransactionId, CreditTransactionType, UserId};

pub struct CreditTransaction {
    id: CreditTransactionId,
    user_id: UserId,
    transaction_type: CreditTransactionType,
    amount: i32,
    balance_after: i32,
    reference_id: Option<Uuid>,
    description: Option<String>,
    created_at: DateTime<Utc>,
}

impl CreditTransaction {
    pub fn new(
        user_id: UserId,
        transaction_type: CreditTransactionType,
        amount: i32,
        balance_after: i32,
        reference_id: Option<Uuid>,
        description: Option<String>,
    ) -> Self {
        Self {
            id: CreditTransactionId::new(),
            user_id,
            transaction_type,
            amount,
            balance_after,
            reference_id,
            description,
            created_at: Utc::now(),
        }
    }

    pub fn from_existing(
        id: CreditTransactionId,
        user_id: UserId,
        transaction_type: CreditTransactionType,
        amount: i32,
        balance_after: i32,
        reference_id: Option<Uuid>,
        description: Option<String>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            user_id,
            transaction_type,
            amount,
            balance_after,
            reference_id,
            description,
            created_at,
        }
    }

    pub fn id(&self) -> &CreditTransactionId {
        &self.id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn transaction_type(&self) -> &CreditTransactionType {
        &self.transaction_type
    }

    pub fn amount(&self) -> i32 {
        self.amount
    }

    pub fn balance_after(&self) -> i32 {
        self.balance_after
    }

    pub fn reference_id(&self) -> Option<&Uuid> {
        self.reference_id.as_ref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }
}
