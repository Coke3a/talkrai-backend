use chrono::{DateTime, Utc};

use crate::domain::error::DomainError;
use crate::domain::value_objects::{JobId, JobMode, JobStatus, SessionId, UserId};

pub struct Job {
    id: JobId,
    mode: JobMode,
    session_id: Option<SessionId>,
    user_id: UserId,
    line_user_id: String,
    user_message: String,
    status: JobStatus,
    attempts: i32,
    max_attempts: i32,
    locked_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    failed_reason: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl Job {
    pub fn new(
        mode: JobMode,
        session_id: Option<SessionId>,
        user_id: UserId,
        line_user_id: String,
        user_message: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: JobId::new(),
            mode,
            session_id,
            user_id,
            line_user_id,
            user_message,
            status: JobStatus::Pending,
            attempts: 0,
            max_attempts: 3,
            locked_at: None,
            completed_at: None,
            failed_reason: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn from_existing(
        id: JobId,
        mode: JobMode,
        session_id: Option<SessionId>,
        user_id: UserId,
        line_user_id: String,
        user_message: String,
        status: JobStatus,
        attempts: i32,
        max_attempts: i32,
        locked_at: Option<DateTime<Utc>>,
        completed_at: Option<DateTime<Utc>>,
        failed_reason: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            mode,
            session_id,
            user_id,
            line_user_id,
            user_message,
            status,
            attempts,
            max_attempts,
            locked_at,
            completed_at,
            failed_reason,
            created_at,
            updated_at,
        }
    }

    pub fn id(&self) -> &JobId {
        &self.id
    }

    pub fn mode(&self) -> &JobMode {
        &self.mode
    }

    pub fn session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref()
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn line_user_id(&self) -> &str {
        &self.line_user_id
    }

    pub fn user_message(&self) -> &str {
        &self.user_message
    }

    pub fn status(&self) -> &JobStatus {
        &self.status
    }

    pub fn attempts(&self) -> i32 {
        self.attempts
    }

    pub fn max_attempts(&self) -> i32 {
        self.max_attempts
    }

    pub fn locked_at(&self) -> Option<&DateTime<Utc>> {
        self.locked_at.as_ref()
    }

    pub fn completed_at(&self) -> Option<&DateTime<Utc>> {
        self.completed_at.as_ref()
    }

    pub fn failed_reason(&self) -> Option<&str> {
        self.failed_reason.as_deref()
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }

    pub fn can_retry(&self) -> bool {
        self.attempts < self.max_attempts
    }

    pub fn lock(&mut self) -> Result<(), DomainError> {
        self.status.transition_to(&JobStatus::Processing)?;
        self.status = JobStatus::Processing;
        self.locked_at = Some(Utc::now());
        self.attempts += 1;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn complete(&mut self) -> Result<(), DomainError> {
        self.status.transition_to(&JobStatus::Completed)?;
        self.status = JobStatus::Completed;
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn fail(&mut self, reason: String) -> Result<(), DomainError> {
        self.status.transition_to(&JobStatus::Failed)?;
        self.status = JobStatus::Failed;
        self.failed_reason = Some(reason);
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn reset_to_pending(&mut self) -> Result<(), DomainError> {
        if !self.can_retry() {
            return Err(DomainError::BusinessRuleViolation(
                "Job has exceeded max attempts".to_string(),
            ));
        }
        self.status = JobStatus::Pending;
        self.locked_at = None;
        self.updated_at = Utc::now();
        Ok(())
    }
}
