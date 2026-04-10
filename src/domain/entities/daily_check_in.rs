use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

use crate::domain::value_objects::UserId;

pub struct DailyCheckIn {
    id: Uuid,
    user_id: UserId,
    checked_in_date: NaiveDate,
    streak_day: i32,
    credits_earned: i32,
    created_at: DateTime<Utc>,
}

impl DailyCheckIn {
    pub fn new(
        user_id: UserId,
        checked_in_date: NaiveDate,
        streak_day: i32,
        credits_earned: i32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            checked_in_date,
            streak_day,
            credits_earned,
            created_at: Utc::now(),
        }
    }

    pub fn from_existing(
        id: Uuid,
        user_id: UserId,
        checked_in_date: NaiveDate,
        streak_day: i32,
        credits_earned: i32,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            user_id,
            checked_in_date,
            streak_day,
            credits_earned,
            created_at,
        }
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }
    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }
    pub fn checked_in_date(&self) -> NaiveDate {
        self.checked_in_date
    }
    pub fn streak_day(&self) -> i32 {
        self.streak_day
    }
    pub fn credits_earned(&self) -> i32 {
        self.credits_earned
    }
    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }
}
