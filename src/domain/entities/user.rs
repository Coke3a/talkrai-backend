use chrono::{DateTime, NaiveDate, Utc};

use crate::domain::error::DomainError;
use crate::domain::value_objects::{CheckInConfig, CheckInOutcome, UserId, UserStatus};

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
    check_in_streak: i32,
    longest_streak: i32,
    last_check_in_on: Option<NaiveDate>,
    last_reminder_sent_on: Option<NaiveDate>,
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
            check_in_streak: 0,
            longest_streak: 0,
            last_check_in_on: None,
            last_reminder_sent_on: None,
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
        check_in_streak: i32,
        longest_streak: i32,
        last_check_in_on: Option<NaiveDate>,
        last_reminder_sent_on: Option<NaiveDate>,
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
            check_in_streak,
            longest_streak,
            last_check_in_on,
            last_reminder_sent_on,
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

    pub fn check_in_streak(&self) -> i32 {
        self.check_in_streak
    }

    pub fn longest_streak(&self) -> i32 {
        self.longest_streak
    }

    pub fn last_check_in_on(&self) -> Option<NaiveDate> {
        self.last_check_in_on
    }

    pub fn last_reminder_sent_on(&self) -> Option<NaiveDate> {
        self.last_reminder_sent_on
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

    /// Apply a daily check-in for `today` (spec §C.3). Idempotent within a day: a second call the
    /// same day is a no-op. A consecutive day extends the streak; a gap resets it to 1.
    pub fn check_in(&mut self, today: NaiveDate, cfg: &CheckInConfig) -> CheckInOutcome {
        if self.last_check_in_on == Some(today) {
            return CheckInOutcome {
                already_checked_in: true,
                credits_awarded: 0,
                new_streak: self.check_in_streak,
                crossed_milestone: false,
            };
        }

        let consecutive = self
            .last_check_in_on
            .and_then(|last| today.pred_opt().map(|yesterday| last == yesterday))
            .unwrap_or(false);
        self.check_in_streak = if consecutive {
            self.check_in_streak + 1
        } else {
            1
        };
        if self.check_in_streak > self.longest_streak {
            self.longest_streak = self.check_in_streak;
        }
        self.last_check_in_on = Some(today);
        self.updated_at = Utc::now();

        let credits_awarded = cfg.credits_for_streak(self.check_in_streak);
        let crossed_milestone = cfg
            .milestone_bonuses
            .iter()
            .any(|(day, _)| *day == self.check_in_streak);

        CheckInOutcome {
            already_checked_in: false,
            credits_awarded,
            new_streak: self.check_in_streak,
            crossed_milestone,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::CheckInConfig;

    fn cfg() -> CheckInConfig {
        CheckInConfig {
            base_credits: 4,
            per_day_bonus: 1,
            max_streak_for_bonus: 10,
            milestone_bonuses: vec![(7, 20), (14, 30), (30, 60)],
            daily_cap: 30,
        }
    }

    fn user() -> User {
        User::new("U1".to_string(), "Test".to_string(), None)
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn first_check_in_starts_streak_at_one() {
        let mut u = user();
        let out = u.check_in(date(2026, 6, 24), &cfg());
        assert!(!out.already_checked_in);
        assert_eq!(out.new_streak, 1);
        assert_eq!(out.credits_awarded, 4);
        assert_eq!(u.check_in_streak(), 1);
        assert_eq!(u.longest_streak(), 1);
        assert_eq!(u.last_check_in_on(), Some(date(2026, 6, 24)));
    }

    #[test]
    fn same_day_second_check_in_is_noop() {
        let mut u = user();
        u.check_in(date(2026, 6, 24), &cfg());
        let out = u.check_in(date(2026, 6, 24), &cfg());
        assert!(out.already_checked_in);
        assert_eq!(out.credits_awarded, 0);
        assert_eq!(u.check_in_streak(), 1);
    }

    #[test]
    fn consecutive_day_increments_streak() {
        let mut u = user();
        u.check_in(date(2026, 6, 24), &cfg());
        let out = u.check_in(date(2026, 6, 25), &cfg());
        assert_eq!(out.new_streak, 2);
        assert_eq!(u.check_in_streak(), 2);
        assert_eq!(u.longest_streak(), 2);
    }

    #[test]
    fn gap_resets_streak_but_keeps_longest() {
        let mut u = user();
        u.check_in(date(2026, 6, 24), &cfg());
        u.check_in(date(2026, 6, 25), &cfg()); // streak 2
        let out = u.check_in(date(2026, 6, 28), &cfg()); // gap of 2 days
        assert_eq!(out.new_streak, 1);
        assert_eq!(u.check_in_streak(), 1);
        assert_eq!(u.longest_streak(), 2);
    }

    #[test]
    fn seven_day_streak_crosses_milestone() {
        let mut u = user();
        let mut out = None;
        for day in 1..=7 {
            out = Some(u.check_in(date(2026, 7, day), &cfg()));
        }
        let out = out.unwrap();
        assert_eq!(out.new_streak, 7);
        assert!(out.crossed_milestone);
        assert_eq!(out.credits_awarded, 30); // ramp 10 + milestone 20, capped at 30
    }
}
