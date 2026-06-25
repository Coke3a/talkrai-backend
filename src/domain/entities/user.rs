use chrono::{DateTime, NaiveDate, Utc};

use crate::domain::error::DomainError;
use crate::domain::value_objects::{CheckInConfig, CheckInOutcome, UserId, UserStatus};

/// Week length, mirrored locally because `check_in.rs`'s `WEEK_LEN` is private to that module.
const WEEK_LEN_PROFILE: i32 = 7;

/// Effective per-day check-in state for /profile (computed by `User::check_in_status`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckInStatus {
    pub checked_in_today: bool,
    pub current_streak: i32,  // 0 when the streak is broken as of `today`
    pub today_cycle_day: i32, // 1..=7 — the cell for today's reward (claimed or claimable)
    pub today_credits: i32,
    pub days_to_chest: i32, // 7 - today_cycle_day (0 on chest day)
}

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
                is_weekly_chest: false,
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
        let is_weekly_chest = CheckInConfig::is_weekly_chest(self.check_in_streak);

        CheckInOutcome {
            already_checked_in: false,
            credits_awarded,
            new_streak: self.check_in_streak,
            is_weekly_chest,
        }
    }

    /// Effective check-in display state as of `today` — pure, no clock, no DB write. Used by
    /// /profile, which may be opened before the user has chatted today (so the stored streak may not
    /// yet "know" it is broken). Delegates all week-index math to the length-guarded `CheckInConfig`.
    pub fn check_in_status(&self, today: NaiveDate, cfg: &CheckInConfig) -> CheckInStatus {
        let checked_in_today = self.last_check_in_on == Some(today);
        let alive_yesterday = self
            .last_check_in_on
            .and_then(|last| today.pred_opt().map(|y| last == y))
            .unwrap_or(false);

        // effective_streak = the streak that owns TODAY's cell (claimed or claimable);
        // current_streak   = the run we show the user (0 once broken).
        let (current_streak, effective_streak) = if checked_in_today {
            (self.check_in_streak, self.check_in_streak)
        } else if alive_yesterday {
            (self.check_in_streak, self.check_in_streak + 1) // today is the next, still-claimable cell
        } else {
            (0, 1) // broken or never → today is a fresh day 1
        };

        let today_cycle_day = CheckInConfig::cycle_day(effective_streak);
        CheckInStatus {
            checked_in_today,
            current_streak,
            today_cycle_day,
            today_credits: cfg.credits_for_streak(effective_streak),
            days_to_chest: WEEK_LEN_PROFILE - today_cycle_day,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::CheckInConfig;

    fn cfg() -> CheckInConfig {
        CheckInConfig {
            weekly_credits: vec![2, 3, 4, 4, 4, 4, 10],
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
        assert_eq!(out.credits_awarded, 2);
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
        assert_eq!(out.credits_awarded, 3); // cycle-day 2
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
    fn seven_day_streak_hits_weekly_chest() {
        let mut u = user();
        let mut out = None;
        for day in 1..=7 {
            out = Some(u.check_in(date(2026, 7, day), &cfg()));
        }
        let out = out.unwrap();
        assert_eq!(out.new_streak, 7);
        assert!(out.is_weekly_chest);
        assert_eq!(out.credits_awarded, 10); // weekly chest amount
    }

    #[test]
    fn status_checked_in_today_shows_claimed_cell() {
        let mut u = user();
        // streak 3, last check-in is today
        u.check_in(date(2026, 7, 1), &cfg());
        u.check_in(date(2026, 7, 2), &cfg());
        u.check_in(date(2026, 7, 3), &cfg());
        let s = u.check_in_status(date(2026, 7, 3), &cfg());
        assert!(s.checked_in_today);
        assert_eq!(s.current_streak, 3); // stored streak
        assert_eq!(s.today_cycle_day, 3);
        assert_eq!(s.today_credits, 4);
        assert_eq!(s.days_to_chest, 4);
    }

    #[test]
    fn status_alive_yesterday_shows_next_claimable_cell() {
        let mut u = user();
        // last check-in was yesterday at streak 3; user has not chatted today yet
        u.check_in(date(2026, 7, 1), &cfg());
        u.check_in(date(2026, 7, 2), &cfg());
        u.check_in(date(2026, 7, 3), &cfg());
        let s = u.check_in_status(date(2026, 7, 4), &cfg());
        assert!(!s.checked_in_today);
        assert_eq!(s.current_streak, 3); // stored streak, not yet incremented
        assert_eq!(s.today_cycle_day, 4); // (stored 3 + 1) → cycle-day 4
        assert_eq!(s.today_credits, 4);
        assert_eq!(s.days_to_chest, 3);
    }

    #[test]
    fn status_gap_resets_to_day_one() {
        let mut u = user();
        // last check-in was several days ago → broken
        u.check_in(date(2026, 7, 1), &cfg());
        u.check_in(date(2026, 7, 2), &cfg()); // streak 2
        let s = u.check_in_status(date(2026, 7, 10), &cfg());
        assert!(!s.checked_in_today);
        assert_eq!(s.current_streak, 0);
        assert_eq!(s.today_cycle_day, 1);
        assert_eq!(s.today_credits, 2);
        assert_eq!(s.days_to_chest, 6);
    }

    #[test]
    fn status_chest_boundary_checked_in_today() {
        let mut u = user();
        for day in 1..=7 {
            u.check_in(date(2026, 7, day), &cfg());
        }
        let s = u.check_in_status(date(2026, 7, 7), &cfg());
        assert!(s.checked_in_today);
        assert_eq!(s.current_streak, 7);
        assert_eq!(s.today_cycle_day, 7);
        assert_eq!(s.today_credits, 10);
        assert_eq!(s.days_to_chest, 0);
    }
}
