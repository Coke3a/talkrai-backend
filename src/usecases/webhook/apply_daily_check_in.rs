use std::sync::Arc;

use chrono::NaiveDate;

use crate::domain::entities::{CreditBalance, User};
use crate::domain::repositories::{AppConfigRepository, CreditRepository, UserRepository};
use crate::domain::value_objects::{CheckInConfig, CheckInOutcome, CreditTransactionType};
use crate::usecases::UsecaseError;

/// What a fresh check-in produces, plus the weekly table the Flex needs to render the calendar
/// without a second config read (the usecase already loaded it).
pub struct CheckInGrant {
    pub outcome: CheckInOutcome,
    pub weekly_credits: Vec<i32>,
}

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

    /// Returns the grant on the first message of the day (drives the check-in calendar flex), or
    /// `None` if already checked in. The grant carries the weekly table so the Flex renders the
    /// calendar without a second config read. On a grant, both the DB and the passed-in `balance`
    /// are topped up so the downstream credit check sees the new balance — the 0-credit deadlock fix.
    pub async fn run(
        &self,
        user: &mut User,
        balance: &mut CreditBalance,
        today: NaiveDate,
    ) -> Result<Option<CheckInGrant>, UsecaseError> {
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

        Ok(Some(CheckInGrant {
            outcome,
            weekly_credits: cfg.weekly_credits.clone(),
        }))
    }

    async fn load_config(&self) -> Result<CheckInConfig, UsecaseError> {
        let raw = self.config_repo.get("daily_checkin_weekly_credits").await?;
        Ok(CheckInConfig::from_config_value(raw.as_deref()))
    }
}
