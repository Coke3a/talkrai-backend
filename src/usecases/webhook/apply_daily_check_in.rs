use std::sync::Arc;

use chrono::NaiveDate;

use crate::domain::entities::{CreditBalance, User};
use crate::domain::repositories::{AppConfigRepository, CreditRepository, UserRepository};
use crate::domain::value_objects::{CheckInConfig, CheckInOutcome, CreditTransactionType};
use crate::usecases::UsecaseError;

const DEFAULT_BASE_CREDITS: i32 = 4;
const DEFAULT_PER_DAY_BONUS: i32 = 1;
const DEFAULT_MAX_STREAK_FOR_BONUS: i32 = 10;
const DEFAULT_DAILY_CAP: i32 = 30;

/// Applies the daily check-in at the top of the roleplay pipeline, BEFORE the credit check
/// (spec §C.4). Idempotent per day via `User::check_in`.
pub struct ApplyDailyCheckInUseCase {
    user_repo: Arc<dyn UserRepository>,
    credit_repo: Arc<dyn CreditRepository>,
    config_repo: Arc<dyn AppConfigRepository>,
}

impl ApplyDailyCheckInUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        credit_repo: Arc<dyn CreditRepository>,
        config_repo: Arc<dyn AppConfigRepository>,
    ) -> Self {
        Self {
            user_repo,
            credit_repo,
            config_repo,
        }
    }

    /// Returns the outcome on the first message of the day (drives the greeting flex), or `None` if
    /// already checked in. On a grant, both the DB and the passed-in `balance` are topped up so the
    /// downstream credit check sees the new balance — this is the 0-credit deadlock fix.
    pub async fn run(
        &self,
        user: &mut User,
        balance: &mut CreditBalance,
        today: NaiveDate,
    ) -> Result<Option<CheckInOutcome>, UsecaseError> {
        let cfg = self.load_config().await?;
        let outcome = user.check_in(today, &cfg);
        if outcome.already_checked_in {
            return Ok(None);
        }

        // Persist the streak FIRST (safe ordering): a crash before the grant under-grants — the user
        // misses one day's bonus — rather than double-granting on the next message. Same mark-first
        // philosophy as the re-engagement enqueue; no cross-table transaction needed.
        self.user_repo.update(user).await?;

        if outcome.credits_awarded > 0 {
            let transaction = balance.add(
                outcome.credits_awarded,
                CreditTransactionType::Bonus,
                None,
                Some(format!("เช็คอินรายวัน +{}", outcome.credits_awarded)),
            )?;
            self.credit_repo
                .add_and_log(user.id(), outcome.credits_awarded, &transaction)
                .await?;
        }

        Ok(Some(outcome))
    }

    async fn load_config(&self) -> Result<CheckInConfig, UsecaseError> {
        let keys = &[
            "daily_checkin_base_credits",
            "daily_checkin_per_day_bonus",
            "daily_checkin_max_streak_for_bonus",
            "daily_checkin_milestone_bonuses",
            "daily_checkin_daily_cap",
        ];
        let map = self.config_repo.get_many(keys).await?;
        let get_i32 =
            |key: &str, default: i32| map.get(key).and_then(|v| v.parse().ok()).unwrap_or(default);

        Ok(CheckInConfig {
            base_credits: get_i32("daily_checkin_base_credits", DEFAULT_BASE_CREDITS),
            per_day_bonus: get_i32("daily_checkin_per_day_bonus", DEFAULT_PER_DAY_BONUS),
            max_streak_for_bonus: get_i32(
                "daily_checkin_max_streak_for_bonus",
                DEFAULT_MAX_STREAK_FOR_BONUS,
            ),
            milestone_bonuses: map
                .get("daily_checkin_milestone_bonuses")
                .map(|s| CheckInConfig::parse_milestones(s))
                .unwrap_or_default(),
            daily_cap: get_i32("daily_checkin_daily_cap", DEFAULT_DAILY_CAP),
        })
    }
}
