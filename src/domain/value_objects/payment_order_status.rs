use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentOrderStatus {
    Pending,
    Completed,
    Failed,
    Expired,
}

impl PaymentOrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Expired => "expired",
        }
    }
}

impl std::str::FromStr for PaymentOrderStatus {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "expired" => Ok(Self::Expired),
            _ => Err(DomainError::InvalidField {
                field: "payment_order_status",
                reason: "invalid payment order status value",
            }),
        }
    }
}
