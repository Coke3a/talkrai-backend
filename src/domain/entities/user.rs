use chrono::{DateTime, Utc};

use crate::domain::error::DomainError;
use crate::domain::value_objects::{UserId, UserStatus};

pub struct User {
    id: UserId,
    line_user_id: String,
    display_name: String,
    picture_url: Option<String>,
    language: String,
    status: UserStatus,
    terms_accepted_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl User {
    pub fn new(line_user_id: String, display_name: String, picture_url: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: UserId::new(),
            line_user_id,
            display_name,
            picture_url,
            language: "th".to_string(),
            status: UserStatus::Active,
            terms_accepted_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_existing(
        id: UserId,
        line_user_id: String,
        display_name: String,
        picture_url: Option<String>,
        language: String,
        status: UserStatus,
        terms_accepted_at: Option<DateTime<Utc>>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            line_user_id,
            display_name,
            picture_url,
            language,
            status,
            terms_accepted_at,
            created_at,
            updated_at,
        }
    }

    pub fn id(&self) -> &UserId {
        &self.id
    }

    pub fn line_user_id(&self) -> &str {
        &self.line_user_id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn picture_url(&self) -> Option<&str> {
        self.picture_url.as_deref()
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn terms_accepted_at(&self) -> Option<&DateTime<Utc>> {
        self.terms_accepted_at.as_ref()
    }

    pub fn status(&self) -> &UserStatus {
        &self.status
    }

    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    pub fn has_accepted_terms(&self) -> bool {
        self.terms_accepted_at.is_some()
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }

    pub fn deactivate(&mut self) {
        self.status = UserStatus::Inactive;
        self.updated_at = Utc::now();
    }

    pub fn activate(&mut self) {
        self.status = UserStatus::Active;
        self.updated_at = Utc::now();
    }

    pub fn update_profile(&mut self, display_name: String, picture_url: Option<String>) {
        self.display_name = display_name;
        self.picture_url = picture_url;
        self.updated_at = Utc::now();
    }

    pub fn accept_terms(&mut self) -> Result<(), DomainError> {
        if self.terms_accepted_at.is_some() {
            return Err(DomainError::BusinessRuleViolation(
                "Terms already accepted".into(),
            ));
        }
        self.terms_accepted_at = Some(Utc::now());
        self.updated_at = Utc::now();
        Ok(())
    }
}
