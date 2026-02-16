use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Active,
    Paused,
    Ended,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Ended => "ended",
        }
    }

    pub fn transition_to(&self, target: &SessionStatus) -> Result<(), DomainError> {
        let valid = matches!(
            (self, target),
            (Self::Active, Self::Paused)
                | (Self::Active, Self::Ended)
                | (Self::Paused, Self::Active)
                | (Self::Paused, Self::Ended)
        );

        if valid {
            Ok(())
        } else {
            Err(DomainError::InvalidStateTransition(format!(
                "Cannot transition session from '{}' to '{}'",
                self.as_str(),
                target.as_str()
            )))
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Ended)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

impl std::str::FromStr for SessionStatus {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "ended" => Ok(Self::Ended),
            _ => Err(DomainError::InvalidField {
                field: "session_status",
                reason: "invalid session status value",
            }),
        }
    }
}
