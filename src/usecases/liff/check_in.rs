use std::sync::Arc;

use chrono::NaiveDate;

use crate::domain::entities::{CreditTransaction, DailyCheckIn, UserStreak};
use crate::domain::repositories::{
    AppConfigRepository, CheckInRepository, CreditRepository, UserRepository,
};
use crate::domain::value_objects::CreditTransactionType;
use crate::usecases::UsecaseError;

pub struct CheckInInput {
    pub line_user_id: String,
}

pub struct CheckInOutput {
    pub credits_earned: i32,
    pub streak_day: i32,
    pub current_streak: i32,
    pub new_balance: i32,
}

pub struct CheckInUseCase {
    user_repo: Arc<dyn UserRepository>,
    check_in_repo: Arc<dyn CheckInRepository>,
    credit_repo: Arc<dyn CreditRepository>,
    config_repo: Arc<dyn AppConfigRepository>,
}

impl CheckInUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        check_in_repo: Arc<dyn CheckInRepository>,
        credit_repo: Arc<dyn CreditRepository>,
        config_repo: Arc<dyn AppConfigRepository>,
    ) -> Self {
        Self {
            user_repo,
            check_in_repo,
            credit_repo,
            config_repo,
        }
    }

    pub async fn execute(
        &self,
        input: CheckInInput,
        today: NaiveDate,
    ) -> Result<CheckInOutput, UsecaseError> {
        // Step 1: Find user
        let user = self
            .user_repo
            .find_by_line_user_id(&input.line_user_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("User not found".to_string()))?;

        // Step 2: Get or create streak
        let streak = self
            .check_in_repo
            .find_streak_by_user_id(user.id())
            .await?
            .unwrap_or_else(|| UserStreak::new(user.id().clone()));

        // Step 3: Guard against duplicate check-in
        if !streak.can_check_in_today(today) {
            return Err(UsecaseError::AlreadyCheckedIn);
        }

        // Step 4: Calculate streak_day
        let streak_day = streak.calculate_next_streak_day(today);

        // Step 5: Read config for credit amounts
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

        let credits_earned = if streak_day == bonus_day {
            bonus_credits
        } else {
            base_credits
        };

        // Step 6: Create DailyCheckIn record
        let check_in = DailyCheckIn::new(user.id().clone(), today, streak_day, credits_earned);

        self.check_in_repo.create_check_in(&check_in).await?;

        // Step 7: Update streak via upsert
        let mut updated_streak = streak;
        updated_streak.apply_check_in(today, streak_day);
        self.check_in_repo.upsert_streak(&updated_streak).await?;

        // Step 8: Get credit balance and calculate new_balance
        let balance = self
            .credit_repo
            .find_balance_by_user_id(user.id())
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Credit balance not found".to_string()))?;

        let new_balance = balance.balance() + credits_earned;

        // Step 9: Create CreditTransaction
        let description = format!("เช็คอินวันที่ {streak_day}");
        let transaction = CreditTransaction::new(
            user.id().clone(),
            CreditTransactionType::Bonus,
            credits_earned,
            new_balance,
            Some(*check_in.id()),
            Some(description),
        );

        // Step 10: Call add_and_log
        self.credit_repo
            .add_and_log(user.id(), credits_earned, &transaction)
            .await?;

        Ok(CheckInOutput {
            credits_earned,
            streak_day,
            current_streak: streak_day,
            new_balance,
        })
    }
}
