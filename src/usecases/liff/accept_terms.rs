use std::sync::Arc;

use crate::domain::repositories::UserRepository;
use crate::usecases::liff::require_active_user::require_active_user;
use crate::usecases::UsecaseError;

pub struct AcceptTermsInput {
    pub line_user_id: String,
}

pub struct AcceptTermsUseCase {
    user_repo: Arc<dyn UserRepository>,
}

impl AcceptTermsUseCase {
    pub fn new(user_repo: Arc<dyn UserRepository>) -> Self {
        Self { user_repo }
    }

    /// Record explicit acceptance of the Terms of Service for the current user.
    ///
    /// Idempotent: if the user has already accepted, this is a no-op (no error, no second write),
    /// so a double-submit from the client cannot trip the entity's "already accepted" invariant.
    pub async fn execute(&self, input: AcceptTermsInput) -> Result<(), UsecaseError> {
        let mut user = require_active_user(&*self.user_repo, &input.line_user_id).await?;

        if user.has_accepted_terms() {
            return Ok(());
        }

        user.accept_terms()
            .map_err(|e| UsecaseError::Validation(format!("Failed to accept terms: {}", e)))?;
        self.user_repo.update(&user).await?;

        tracing::info!(user_id = %user.id().as_uuid(), "User accepted terms");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::User;
    use crate::domain::repositories::{ReengagementTarget, RepoError};
    use crate::domain::value_objects::{UserId, UserStatus};
    use async_trait::async_trait;
    use chrono::{NaiveDate, Utc};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    /// Mock that hands back an active user whose terms state is fixed at construction, and records
    /// every `update` call plus the accepted-flag of the user written on the last one.
    struct MockUserRepo {
        already_accepted: bool,
        update_count: AtomicUsize,
        last_update_accepted: Mutex<Option<bool>>,
    }

    impl MockUserRepo {
        fn new(already_accepted: bool) -> Self {
            Self {
                already_accepted,
                update_count: AtomicUsize::new(0),
                last_update_accepted: Mutex::new(None),
            }
        }

        fn user(&self) -> User {
            User::from_existing(
                UserId::new(),
                "U_test".to_string(),
                "Tester".to_string(),
                None,
                "th".to_string(),
                UserStatus::Active,
                if self.already_accepted {
                    Some(Utc::now())
                } else {
                    None
                },
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
            Ok(Some(self.user()))
        }
        async fn upsert(&self, _: &User) -> Result<(), RepoError> {
            Ok(())
        }
        async fn update(&self, user: &User) -> Result<(), RepoError> {
            self.update_count.fetch_add(1, Ordering::SeqCst);
            *self.last_update_accepted.lock().unwrap() = Some(user.has_accepted_terms());
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

    fn input() -> AcceptTermsInput {
        AcceptTermsInput {
            line_user_id: "U_test".to_string(),
        }
    }

    #[tokio::test]
    async fn first_time_acceptance_persists_accepted_user() {
        let repo = Arc::new(MockUserRepo::new(false));
        let usecase = AcceptTermsUseCase::new(repo.clone());

        usecase.execute(input()).await.expect("should accept");

        assert_eq!(repo.update_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            *repo.last_update_accepted.lock().unwrap(),
            Some(true),
            "the persisted user must have terms accepted"
        );
    }

    #[tokio::test]
    async fn already_accepted_is_noop() {
        let repo = Arc::new(MockUserRepo::new(true));
        let usecase = AcceptTermsUseCase::new(repo.clone());

        usecase
            .execute(input())
            .await
            .expect("should be idempotent");

        assert_eq!(
            repo.update_count.load(Ordering::SeqCst),
            0,
            "an already-accepted user must not be written again"
        );
    }
}
