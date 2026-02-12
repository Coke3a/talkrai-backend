use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, DomainError> {
        match s {
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(DomainError::InvalidField {
                field: "job_status",
                reason: "invalid job status value",
            }),
        }
    }

    pub fn transition_to(&self, target: &JobStatus) -> Result<(), DomainError> {
        let valid = matches!(
            (self, target),
            (Self::Pending, Self::Processing)
                | (Self::Processing, Self::Completed)
                | (Self::Processing, Self::Failed)
        );

        if valid {
            Ok(())
        } else {
            Err(DomainError::InvalidStateTransition(format!(
                "Cannot transition job from '{}' to '{}'",
                self.as_str(),
                target.as_str()
            )))
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }
}
