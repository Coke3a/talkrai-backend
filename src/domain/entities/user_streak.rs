use chrono::NaiveDate;

use crate::domain::value_objects::UserId;

#[derive(Clone)]
pub struct UserStreak {
    user_id: UserId,
    current_streak: i32,
    last_check_in_date: Option<NaiveDate>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl UserStreak {
    pub fn new(user_id: UserId) -> Self {
        Self {
            user_id,
            current_streak: 0,
            last_check_in_date: None,
            updated_at: chrono::Utc::now(),
        }
    }

    pub fn from_existing(
        user_id: UserId,
        current_streak: i32,
        last_check_in_date: Option<NaiveDate>,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            user_id,
            current_streak,
            last_check_in_date,
            updated_at,
        }
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }
    pub fn current_streak(&self) -> i32 {
        self.current_streak
    }
    pub fn last_check_in_date(&self) -> Option<NaiveDate> {
        self.last_check_in_date
    }
    pub fn updated_at(&self) -> &chrono::DateTime<chrono::Utc> {
        &self.updated_at
    }

    pub fn can_check_in_today(&self, today: NaiveDate) -> bool {
        self.last_check_in_date != Some(today)
    }

    pub fn calculate_next_streak_day(&self, today: NaiveDate) -> i32 {
        match self.last_check_in_date {
            Some(last_date) if today.pred_opt() == Some(last_date) => (self.current_streak % 7) + 1,
            _ => 1,
        }
    }

    pub fn apply_check_in(&mut self, today: NaiveDate, streak_day: i32) {
        self.current_streak = streak_day;
        self.last_check_in_date = Some(today);
        self.updated_at = chrono::Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_streak(current: i32, last_date: Option<NaiveDate>) -> UserStreak {
        UserStreak::from_existing(UserId::new(), current, last_date, chrono::Utc::now())
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn can_check_in_when_never_checked_in() {
        let streak = make_streak(0, None);
        assert!(streak.can_check_in_today(date(2026, 4, 10)));
    }

    #[test]
    fn cannot_check_in_same_day() {
        let streak = make_streak(1, Some(date(2026, 4, 10)));
        assert!(!streak.can_check_in_today(date(2026, 4, 10)));
    }

    #[test]
    fn can_check_in_next_day() {
        let streak = make_streak(1, Some(date(2026, 4, 10)));
        assert!(streak.can_check_in_today(date(2026, 4, 11)));
    }

    #[test]
    fn streak_continues_when_consecutive() {
        let streak = make_streak(3, Some(date(2026, 4, 9)));
        assert_eq!(streak.calculate_next_streak_day(date(2026, 4, 10)), 4);
    }

    #[test]
    fn streak_resets_when_gap() {
        let streak = make_streak(3, Some(date(2026, 4, 8)));
        assert_eq!(streak.calculate_next_streak_day(date(2026, 4, 10)), 1);
    }

    #[test]
    fn streak_resets_after_day_7() {
        let streak = make_streak(7, Some(date(2026, 4, 9)));
        assert_eq!(streak.calculate_next_streak_day(date(2026, 4, 10)), 1);
    }

    #[test]
    fn streak_first_ever_check_in() {
        let streak = make_streak(0, None);
        assert_eq!(streak.calculate_next_streak_day(date(2026, 4, 10)), 1);
    }

    #[test]
    fn streak_day_6_to_7() {
        let streak = make_streak(6, Some(date(2026, 4, 9)));
        assert_eq!(streak.calculate_next_streak_day(date(2026, 4, 10)), 7);
    }

    #[test]
    fn apply_check_in_updates_state() {
        let mut streak = make_streak(2, Some(date(2026, 4, 9)));
        streak.apply_check_in(date(2026, 4, 10), 3);
        assert_eq!(streak.current_streak(), 3);
        assert_eq!(streak.last_check_in_date(), Some(date(2026, 4, 10)));
    }
}
