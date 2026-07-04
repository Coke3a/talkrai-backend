use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::error::DomainError;
use crate::domain::value_objects::{LiffPage, UserId};

pub struct AnalyticsEvent {
    user_id: UserId,
    event_name: String,
    page: Option<LiffPage>,
    properties: Option<serde_json::Value>,
    client_event_id: Uuid,
    occurred_at: DateTime<Utc>,
}

impl AnalyticsEvent {
    pub fn new(
        user_id: UserId,
        event_name: String,
        page: Option<LiffPage>,
        properties: Option<serde_json::Value>,
        client_event_id: Uuid,
        occurred_at: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        let len = event_name.chars().count();
        if len == 0 {
            return Err(DomainError::InvalidField {
                field: "event_name",
                reason: "event name must not be empty",
            });
        }
        if len > 64 {
            return Err(DomainError::InvalidField {
                field: "event_name",
                reason: "event name exceeds 64 chars",
            });
        }

        Ok(Self {
            user_id,
            event_name,
            page,
            properties,
            client_event_id,
            occurred_at,
        })
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn event_name(&self) -> &str {
        &self.event_name
    }

    pub fn page(&self) -> Option<&LiffPage> {
        self.page.as_ref()
    }

    pub fn properties(&self) -> Option<&serde_json::Value> {
        self.properties.as_ref()
    }

    pub fn client_event_id(&self) -> &Uuid {
        &self.client_event_id
    }

    pub fn occurred_at(&self) -> &DateTime<Utc> {
        &self.occurred_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_event(name: &str, page: Option<LiffPage>) -> Result<AnalyticsEvent, DomainError> {
        AnalyticsEvent::new(
            UserId::new(),
            name.to_string(),
            page,
            None,
            Uuid::new_v4(),
            Utc::now(),
        )
    }

    #[test]
    fn valid_event_builds_ok() {
        assert!(new_event("page_view", Some(LiffPage::Scenes)).is_ok());
    }

    #[test]
    fn empty_name_is_err() {
        assert!(matches!(
            new_event("", None),
            Err(DomainError::InvalidField {
                field: "event_name",
                ..
            })
        ));
    }

    #[test]
    fn name_over_64_chars_is_err() {
        let name = "a".repeat(65);
        assert!(matches!(
            new_event(&name, None),
            Err(DomainError::InvalidField {
                field: "event_name",
                ..
            })
        ));
    }

    #[test]
    fn name_exactly_64_chars_is_ok() {
        let name = "a".repeat(64);
        assert!(new_event(&name, None).is_ok());
    }

    #[test]
    fn none_page_builds_ok() {
        let event = new_event("page_view", None).unwrap();
        assert!(event.page().is_none());
    }

    #[test]
    fn some_credits_page_builds_ok() {
        let event = new_event("page_view", Some(LiffPage::Credits)).unwrap();
        assert_eq!(event.page(), Some(&LiffPage::Credits));
    }
}
