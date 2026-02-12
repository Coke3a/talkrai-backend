use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreditTransactionType {
    Purchase,
    Consumption,
    Bonus,
    Refund,
    Adjustment,
}

impl CreditTransactionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Purchase => "purchase",
            Self::Consumption => "consumption",
            Self::Bonus => "bonus",
            Self::Refund => "refund",
            Self::Adjustment => "adjustment",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, DomainError> {
        match s {
            "purchase" => Ok(Self::Purchase),
            "consumption" => Ok(Self::Consumption),
            "bonus" => Ok(Self::Bonus),
            "refund" => Ok(Self::Refund),
            "adjustment" => Ok(Self::Adjustment),
            _ => Err(DomainError::InvalidField {
                field: "credit_transaction_type",
                reason: "invalid credit transaction type value",
            }),
        }
    }
}
