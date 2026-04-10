use std::sync::Arc;

use chrono::NaiveDate;

use crate::domain::repositories::{AppConfigRepository, CheckInRepository, UserRepository};
use crate::usecases::UsecaseError;

pub struct GetCheckInStatusInput {
    pub line_user_id: String,
}

pub struct StreakDayItem {
    pub day: i32,
    pub completed: bool,
    pub credits: i32,
}

pub struct GetCheckInStatusOutput {
    pub checked_in_today: bool,
    pub current_streak: i32,
    pub streak_day: i32,
    pub credits_to_earn: i32,
    pub streak_history: Vec<StreakDayItem>,
}

pub struct GetCheckInStatusUseCase {
    user_repo: Arc<dyn UserRepository>,
    check_in_repo: Arc<dyn CheckInRepository>,
    config_repo: Arc<dyn AppConfigRepository>,
}

impl GetCheckInStatusUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        check_in_repo: Arc<dyn CheckInRepository>,
        config_repo: Arc<dyn AppConfigRepository>,
    ) -> Self {
        Self {
            user_repo,
            check_in_repo,
            config_repo,
        }
    }

    pub async fn execute(
        &self,
        input: GetCheckInStatusInput,
        today: NaiveDate,
    ) -> Result<GetCheckInStatusOutput, UsecaseError> {
        let user = self
            .user_repo
            .find_by_line_user_id(&input.line_user_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("User not found".to_string()))?;

        let streak = self.check_in_repo.find_streak_by_user_id(user.id()).await?;

        let (checked_in_today, current_streak, next_streak_day) = match &streak {
            Some(s) => {
                let checked_in = !s.can_check_in_today(today);
                let current = s.current_streak();
                let next = s.calculate_next_streak_day(today);
                (checked_in, current, next)
            }
            None => (false, 0, 1),
        };

        let config_map = self
            .config_repo
            .get_many(&[
                "checkin_base_credits",
                "checkin_bonus_day",
                "checkin_bonus_credits",
            ])
            .await?;

        let base_credits: i32 = config_map
            .get("checkin_base_credits")
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);
        let bonus_day: i32 = config_map
            .get("checkin_bonus_day")
            .and_then(|v| v.parse().ok())
            .unwrap_or(7);
        let bonus_credits: i32 = config_map
            .get("checkin_bonus_credits")
            .and_then(|v| v.parse().ok())
            .unwrap_or(6);

        let credits_for_day = |day: i32| -> i32 {
            if day == bonus_day {
                bonus_credits
            } else {
                base_credits
            }
        };

        let credits_to_earn = credits_for_day(next_streak_day);

        // streak_day is what we present to the UI:
        // - if checked in today: the day we just completed (current_streak)
        // - if not yet: the day we're about to complete (next_streak_day)
        let streak_day = if checked_in_today {
            current_streak
        } else {
            next_streak_day
        };

        // Build 7-item history: days 1..=7
        // completed days = days already done in current cycle
        //   checked in today  → days 1..=current_streak are done
        //   not checked in    → days 1..next_streak_day-1 are done
        let completed_up_to = if checked_in_today {
            current_streak
        } else {
            next_streak_day - 1
        };

        let streak_history = (1..=7)
            .map(|day| StreakDayItem {
                day,
                completed: day <= completed_up_to,
                credits: credits_for_day(day),
            })
            .collect();

        Ok(GetCheckInStatusOutput {
            checked_in_today,
            current_streak,
            streak_day,
            credits_to_earn,
            streak_history,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{DailyCheckIn, User, UserStreak};
    use crate::domain::repositories::RepoError;
    use crate::domain::value_objects::UserId;
    use async_trait::async_trait;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // ── Mock repos ────────────────────────────────────────────────────────────

    struct MockUserRepo {
        user: Option<User>,
    }

    impl MockUserRepo {
        fn with_user(user: User) -> Self {
            Self { user: Some(user) }
        }
        fn empty() -> Self {
            Self { user: None }
        }
    }

    #[async_trait]
    impl UserRepository for MockUserRepo {
        async fn find_by_id(&self, _id: &UserId) -> Result<Option<User>, RepoError> {
            Ok(None)
        }
        async fn find_by_line_user_id(&self, _id: &str) -> Result<Option<User>, RepoError> {
            Ok(self.user.as_ref().cloned())
        }
        async fn upsert(&self, _user: &User) -> Result<(), RepoError> {
            Ok(())
        }
        async fn update(&self, _user: &User) -> Result<(), RepoError> {
            Ok(())
        }
    }

    // User doesn't derive Clone, provide a helper
    impl Clone for User {
        fn clone(&self) -> Self {
            User::from_existing(
                self.id().clone(),
                self.line_user_id().to_string(),
                self.display_name().to_string(),
                self.picture_url().map(|s| s.to_string()),
                self.language().to_string(),
                self.terms_accepted_at().cloned(),
                *self.created_at(),
                *self.updated_at(),
            )
        }
    }

    struct MockCheckInRepo {
        streak: Mutex<Option<UserStreak>>,
    }

    impl MockCheckInRepo {
        fn with_streak(streak: UserStreak) -> Self {
            Self {
                streak: Mutex::new(Some(streak)),
            }
        }
        fn no_streak() -> Self {
            Self {
                streak: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl CheckInRepository for MockCheckInRepo {
        async fn find_streak_by_user_id(
            &self,
            _user_id: &UserId,
        ) -> Result<Option<UserStreak>, RepoError> {
            Ok(self.streak.lock().unwrap().take())
        }
        async fn upsert_streak(&self, _streak: &UserStreak) -> Result<(), RepoError> {
            Ok(())
        }
        async fn create_check_in(&self, _check_in: &DailyCheckIn) -> Result<(), RepoError> {
            Ok(())
        }
        async fn find_check_ins_by_user_id(
            &self,
            _user_id: &UserId,
            _limit: i64,
        ) -> Result<Vec<DailyCheckIn>, RepoError> {
            Ok(vec![])
        }
    }

    struct MockConfigRepo {
        values: HashMap<String, String>,
    }

    impl MockConfigRepo {
        fn default_credits() -> Self {
            let mut values = HashMap::new();
            values.insert("checkin_base_credits".to_string(), "2".to_string());
            values.insert("checkin_bonus_day".to_string(), "7".to_string());
            values.insert("checkin_bonus_credits".to_string(), "6".to_string());
            Self { values }
        }
    }

    #[async_trait]
    impl AppConfigRepository for MockConfigRepo {
        async fn get(&self, key: &str) -> Result<Option<String>, RepoError> {
            Ok(self.values.get(key).cloned())
        }
        async fn get_many(&self, keys: &[&str]) -> Result<HashMap<String, String>, RepoError> {
            let result = keys
                .iter()
                .filter_map(|k| self.values.get(*k).map(|v| (k.to_string(), v.clone())))
                .collect();
            Ok(result)
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_user() -> User {
        User::from_existing(
            UserId::new(),
            "U_test".to_string(),
            "Test".to_string(),
            None,
            "th".to_string(),
            None,
            Utc::now(),
            Utc::now(),
        )
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn make_usecase(user: User, streak: Option<UserStreak>) -> GetCheckInStatusUseCase {
        let check_in_repo: Arc<dyn CheckInRepository> = match streak {
            Some(s) => Arc::new(MockCheckInRepo::with_streak(s)),
            None => Arc::new(MockCheckInRepo::no_streak()),
        };
        GetCheckInStatusUseCase::new(
            Arc::new(MockUserRepo::with_user(user)),
            check_in_repo,
            Arc::new(MockConfigRepo::default_credits()),
        )
    }

    fn input() -> GetCheckInStatusInput {
        GetCheckInStatusInput {
            line_user_id: "U_test".to_string(),
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn new_user_no_streak() {
        let uc = make_usecase(make_user(), None);
        let out = uc.execute(input(), date(2026, 4, 10)).await.unwrap();

        assert!(!out.checked_in_today);
        assert_eq!(out.current_streak, 0);
        assert_eq!(out.streak_day, 1);
        assert_eq!(out.credits_to_earn, 2); // base_credits = 2
        assert_eq!(out.streak_history.len(), 7);
        assert!(out.streak_history.iter().all(|item| !item.completed));
        // days 1-6 = 2 credits, day 7 = 6 credits
        assert_eq!(out.streak_history[0].credits, 2);
        assert_eq!(out.streak_history[6].credits, 6);
    }

    #[tokio::test]
    async fn already_checked_in_today_streak_3() {
        let user = make_user();
        let streak =
            UserStreak::from_existing(user.id().clone(), 3, Some(date(2026, 4, 10)), Utc::now());
        let uc = make_usecase(user, Some(streak));
        let out = uc.execute(input(), date(2026, 4, 10)).await.unwrap();

        assert!(out.checked_in_today);
        assert_eq!(out.current_streak, 3);
        assert_eq!(out.streak_day, 3);
        // next_streak_day = calculate_next_streak_day when last_date == today:
        //   last_date (Apr 10) vs today.pred (Apr 9) → NOT consecutive → resets to 1
        // So credits_to_earn = credits_for_day(1) = 2 (base credits)
        assert_eq!(out.credits_to_earn, 2);
        // completed days: 1, 2, 3
        let completed: Vec<bool> = out.streak_history.iter().map(|i| i.completed).collect();
        assert_eq!(
            completed,
            vec![true, true, true, false, false, false, false]
        );
    }

    #[tokio::test]
    async fn not_checked_in_streak_2_yesterday() {
        let user = make_user();
        let streak =
            UserStreak::from_existing(user.id().clone(), 2, Some(date(2026, 4, 9)), Utc::now());
        let uc = make_usecase(user, Some(streak));
        let out = uc.execute(input(), date(2026, 4, 10)).await.unwrap();

        assert!(!out.checked_in_today);
        assert_eq!(out.current_streak, 2);
        assert_eq!(out.streak_day, 3); // next day to complete
        assert_eq!(out.credits_to_earn, 2); // day 3 = base_credits = 2
                                            // completed days: 1, 2 (next is 3, so completed_up_to = 3-1 = 2)
        let completed: Vec<bool> = out.streak_history.iter().map(|i| i.completed).collect();
        assert_eq!(
            completed,
            vec![true, true, false, false, false, false, false]
        );
    }

    #[tokio::test]
    async fn streak_reset_after_gap() {
        let user = make_user();
        // Last check-in was 2 days ago → streak resets on next check-in
        let streak =
            UserStreak::from_existing(user.id().clone(), 5, Some(date(2026, 4, 8)), Utc::now());
        let uc = make_usecase(user, Some(streak));
        let out = uc.execute(input(), date(2026, 4, 10)).await.unwrap();

        assert!(!out.checked_in_today);
        assert_eq!(out.streak_day, 1); // resets to day 1
        assert_eq!(out.credits_to_earn, 2); // day 1 = base_credits = 2
        assert!(out.streak_history.iter().all(|item| !item.completed));
    }

    #[tokio::test]
    async fn user_not_found_returns_error() {
        let uc = GetCheckInStatusUseCase::new(
            Arc::new(MockUserRepo::empty()),
            Arc::new(MockCheckInRepo::no_streak()),
            Arc::new(MockConfigRepo::default_credits()),
        );
        let result = uc.execute(input(), date(2026, 4, 10)).await;
        assert!(matches!(result, Err(UsecaseError::NotFound(_))));
    }

    #[tokio::test]
    async fn streak_history_days_are_1_to_7() {
        let uc = make_usecase(make_user(), None);
        let out = uc.execute(input(), date(2026, 4, 10)).await.unwrap();
        let days: Vec<i32> = out.streak_history.iter().map(|i| i.day).collect();
        assert_eq!(days, vec![1, 2, 3, 4, 5, 6, 7]);
    }

    #[tokio::test]
    async fn streak_day_7_completed_all_history() {
        let user = make_user();
        let streak =
            UserStreak::from_existing(user.id().clone(), 7, Some(date(2026, 4, 10)), Utc::now());
        let uc = make_usecase(user, Some(streak));
        let out = uc.execute(input(), date(2026, 4, 10)).await.unwrap();

        assert!(out.checked_in_today);
        assert_eq!(out.streak_day, 7);
        assert!(out.streak_history.iter().all(|item| item.completed));
    }
}
