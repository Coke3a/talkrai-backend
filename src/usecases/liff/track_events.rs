use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::domain::entities::AnalyticsEvent;
use crate::domain::error::DomainError;
use crate::domain::repositories::{AnalyticsEventRepository, UserRepository};
use crate::domain::value_objects::LiffPage;
use crate::usecases::UsecaseError;

/// Hard cap on events accepted in a single batch. Exceeding it is a structural client-contract
/// violation (400), not something worth partially servicing.
const MAX_BATCH: usize = 50;

/// One event as received from the client, pre-validation.
pub struct IncomingEvent {
    pub name: String,
    pub page: Option<String>,
    pub properties: Option<serde_json::Value>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

pub struct TrackEventsInput {
    pub line_user_id: String,
    pub events: Vec<IncomingEvent>,
}

pub struct TrackEventsOutput {
    pub accepted: u64,
}

pub struct TrackEventsUseCase {
    user_repo: Arc<dyn UserRepository>,
    analytics_repo: Arc<dyn AnalyticsEventRepository>,
}

impl TrackEventsUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        analytics_repo: Arc<dyn AnalyticsEventRepository>,
    ) -> Self {
        Self {
            user_repo,
            analytics_repo,
        }
    }

    /// Record a batch of client-side analytics events for the LINE user behind the LIFF token.
    ///
    /// - A batch larger than [`MAX_BATCH`] is rejected outright (`UsecaseError::Validation`) — a
    ///   deterministic client-contract violation; retrying won't help.
    /// - An unknown LINE user is a no-op (`accepted = 0`), never an error: tracking must not gate
    ///   on the active-user rule the way `require_active_user` does.
    /// - Individual events with an unknown `page`, an invalid `event_name`, or an implausible
    ///   `occurred_at` (outside `[now-24h, now+5min]`) are dropped silently; they never fail the
    ///   whole batch.
    pub async fn execute(
        &self,
        input: TrackEventsInput,
    ) -> Result<TrackEventsOutput, UsecaseError> {
        if input.events.len() > MAX_BATCH {
            return Err(DomainError::InvalidField {
                field: "events",
                reason: "batch exceeds 50",
            }
            .into());
        }

        let user = match self
            .user_repo
            .find_by_line_user_id(&input.line_user_id)
            .await?
        {
            Some(user) => user,
            None => return Ok(TrackEventsOutput { accepted: 0 }),
        };

        let now = Utc::now();
        let lower = now - Duration::hours(24);
        let upper = now + Duration::minutes(5);

        let mut valid: Vec<AnalyticsEvent> = Vec::with_capacity(input.events.len());
        for ev in input.events {
            // R3: drop events with an implausible client clock rather than fail the batch.
            if ev.occurred_at < lower || ev.occurred_at > upper {
                continue;
            }

            let page = match ev.page {
                Some(raw) => match LiffPage::from_str(&raw) {
                    Ok(page) => Some(page),
                    Err(_) => continue, // unknown page string: skip this event only
                },
                None => None,
            };

            match AnalyticsEvent::new(
                user.id().clone(),
                ev.name,
                page,
                ev.properties,
                ev.client_event_id,
                ev.occurred_at,
            ) {
                Ok(event) => valid.push(event),
                Err(_) => continue, // e.g. empty/too-long name: skip this event only
            }
        }

        let accepted = self.analytics_repo.record_batch(&valid).await?;
        Ok(TrackEventsOutput { accepted })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::User;
    use crate::domain::repositories::{ReengagementTarget, RepoError};
    use crate::domain::value_objects::{UserId, UserStatus};
    use async_trait::async_trait;
    use chrono::NaiveDate;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    /// Hands back a fixed active user when `has_user` is true, `None` otherwise (unknown LINE
    /// user path). Mirrors the hand-written mock style in `accept_terms.rs`.
    struct MockUserRepo {
        has_user: bool,
    }

    impl MockUserRepo {
        fn new(has_user: bool) -> Self {
            Self { has_user }
        }

        fn user(&self) -> User {
            User::from_existing(
                UserId::new(),
                "U_test".to_string(),
                "Tester".to_string(),
                None,
                "th".to_string(),
                UserStatus::Active,
                Some(Utc::now()),
                Utc::now(),
                Utc::now(),
                0,
                0,
                None,
                None,
            )
        }
    }

    #[async_trait]
    impl UserRepository for MockUserRepo {
        async fn find_by_id(&self, _: &UserId) -> Result<Option<User>, RepoError> {
            Ok(None)
        }
        async fn find_by_line_user_id(&self, _: &str) -> Result<Option<User>, RepoError> {
            if self.has_user {
                Ok(Some(self.user()))
            } else {
                Ok(None)
            }
        }
        async fn upsert(&self, _: &User) -> Result<(), RepoError> {
            Ok(())
        }
        async fn update(&self, _: &User) -> Result<(), RepoError> {
            Ok(())
        }
        async fn find_reengagement_targets(
            &self,
            _: NaiveDate,
            _: i64,
            _: i64,
        ) -> Result<Vec<ReengagementTarget>, RepoError> {
            Ok(vec![])
        }
        async fn mark_reminded(&self, _: &[UserId], _: NaiveDate) -> Result<(), RepoError> {
            Ok(())
        }
    }

    /// Records how many times `record_batch` was called and the length of the last batch it
    /// received, and reports every event in the batch as newly inserted.
    struct MockAnalyticsRepo {
        call_count: AtomicUsize,
        last_batch_len: Mutex<Option<usize>>,
    }

    impl MockAnalyticsRepo {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
                last_batch_len: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl AnalyticsEventRepository for MockAnalyticsRepo {
        async fn record_batch(&self, events: &[AnalyticsEvent]) -> Result<u64, RepoError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let len = events.len();
            *self.last_batch_len.lock().unwrap() = Some(len);
            Ok(len as u64)
        }
    }

    fn valid_event() -> IncomingEvent {
        IncomingEvent {
            name: "page_view".to_string(),
            page: Some("scenes".to_string()),
            properties: None,
            client_event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
        }
    }

    fn input(events: Vec<IncomingEvent>) -> TrackEventsInput {
        TrackEventsInput {
            line_user_id: "U_test".to_string(),
            events,
        }
    }

    #[tokio::test]
    async fn all_valid_events_are_recorded() {
        let user_repo = Arc::new(MockUserRepo::new(true));
        let analytics_repo = Arc::new(MockAnalyticsRepo::new());
        let usecase = TrackEventsUseCase::new(user_repo, analytics_repo.clone());

        let output = usecase
            .execute(input(vec![valid_event(), valid_event(), valid_event()]))
            .await
            .expect("should succeed");

        assert_eq!(output.accepted, 3);
        assert_eq!(analytics_repo.call_count.load(Ordering::SeqCst), 1);
        assert_eq!(*analytics_repo.last_batch_len.lock().unwrap(), Some(3));
    }

    #[tokio::test]
    async fn mixed_batch_only_forwards_valid_events() {
        let user_repo = Arc::new(MockUserRepo::new(true));
        let analytics_repo = Arc::new(MockAnalyticsRepo::new());
        let usecase = TrackEventsUseCase::new(user_repo, analytics_repo.clone());

        let unknown_page = IncomingEvent {
            page: Some("how-to".to_string()), // hyphen: invalid, LiffPage uses "how_to"
            ..valid_event()
        };
        let skewed_clock = IncomingEvent {
            occurred_at: Utc::now() - Duration::hours(48),
            ..valid_event()
        };

        let output = usecase
            .execute(input(vec![unknown_page, skewed_clock, valid_event()]))
            .await
            .expect("should succeed");

        assert_eq!(
            output.accepted, 1,
            "only the one fully-valid event should pass"
        );
        assert_eq!(analytics_repo.call_count.load(Ordering::SeqCst), 1);
        assert_eq!(*analytics_repo.last_batch_len.lock().unwrap(), Some(1));
    }

    #[tokio::test]
    async fn batch_over_cap_is_rejected_and_never_reaches_repo() {
        let user_repo = Arc::new(MockUserRepo::new(true));
        let analytics_repo = Arc::new(MockAnalyticsRepo::new());
        let usecase = TrackEventsUseCase::new(user_repo, analytics_repo.clone());

        let events: Vec<IncomingEvent> = (0..51).map(|_| valid_event()).collect();

        let result = usecase.execute(input(events)).await;

        assert!(matches!(result, Err(UsecaseError::Validation(_))));
        assert_eq!(analytics_repo.call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unknown_user_is_noop_and_never_reaches_repo() {
        let user_repo = Arc::new(MockUserRepo::new(false));
        let analytics_repo = Arc::new(MockAnalyticsRepo::new());
        let usecase = TrackEventsUseCase::new(user_repo, analytics_repo.clone());

        let output = usecase
            .execute(input(vec![valid_event(), valid_event()]))
            .await
            .expect("unknown user must not error");

        assert_eq!(output.accepted, 0);
        assert_eq!(analytics_repo.call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn skewed_event_drops_out_leaving_nothing_to_record() {
        let user_repo = Arc::new(MockUserRepo::new(true));
        let analytics_repo = Arc::new(MockAnalyticsRepo::new());
        let usecase = TrackEventsUseCase::new(user_repo, analytics_repo.clone());

        let skewed = IncomingEvent {
            occurred_at: Utc::now() - Duration::hours(48),
            ..valid_event()
        };

        let output = usecase
            .execute(input(vec![skewed]))
            .await
            .expect("should succeed");

        assert_eq!(output.accepted, 0);
    }
}
