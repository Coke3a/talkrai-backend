//! Daily check-in value objects (spec 2026-06-25 §R2, weekly cycle).
//!
//! Pure: no IO, no clock. The usecase builds [`CheckInConfig`] from `app_config` and supplies it
//! to [`crate::domain::entities::User::check_in`].

const DEFAULT_WEEKLY_CREDITS: [i32; 7] = [2, 3, 4, 4, 4, 4, 10];
const WEEK_LEN: i32 = 7;

/// Weekly check-in reward cycle, built from `app_config` in the usecase layer. The 7-entry table
/// repeats every 7 consecutive days; reward = weekly_credits[(streak - 1) mod 7].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckInConfig {
    pub weekly_credits: Vec<i32>, // exactly 7 entries; malformed config falls back to DEFAULT_WEEKLY_CREDITS
}

impl CheckInConfig {
    /// Build from the raw `daily_checkin_weekly_credits` config value (None / malformed → default).
    pub fn from_config_value(raw: Option<&str>) -> Self {
        let weekly_credits = match raw {
            Some(s) => Self::parse_weekly_credits(s),
            None => DEFAULT_WEEKLY_CREDITS.to_vec(),
        };
        Self { weekly_credits }
    }

    /// Credits for the check-in that produced `streak` (1-based). Cycles every 7 days.
    pub fn credits_for_streak(&self, streak: i32) -> i32 {
        self.week()[(streak - 1).rem_euclid(WEEK_LEN) as usize]
    }

    /// Position in the current week, 1..=7.
    pub fn cycle_day(streak: i32) -> i32 {
        (streak - 1).rem_euclid(WEEK_LEN) + 1
    }

    /// True on the weekly chest day (cycle-day 7).
    pub fn is_weekly_chest(streak: i32) -> bool {
        streak % WEEK_LEN == 0
    }

    /// Parse "2,3,4,4,4,4,10"; fall back to default unless exactly 7 valid ints.
    pub fn parse_weekly_credits(raw: &str) -> Vec<i32> {
        let v: Vec<i32> = raw
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if v.len() == 7 {
            v
        } else {
            DEFAULT_WEEKLY_CREDITS.to_vec()
        }
    }

    /// Length-guarded read so a malformed stored Vec can never index out of range.
    fn week(&self) -> &[i32] {
        if self.weekly_credits.len() == 7 {
            &self.weekly_credits
        } else {
            &DEFAULT_WEEKLY_CREDITS
        }
    }
}

/// Result of a check-in attempt for one day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckInOutcome {
    pub already_checked_in: bool,
    pub credits_awarded: i32,
    pub new_streak: i32,
    pub is_weekly_chest: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credits_cycle_every_seven_days() {
        let cfg = CheckInConfig::from_config_value(Some("2,3,4,4,4,4,10"));
        assert_eq!(cfg.credits_for_streak(1), 2);
        assert_eq!(cfg.credits_for_streak(3), 4);
        assert_eq!(cfg.credits_for_streak(7), 10);
        assert_eq!(cfg.credits_for_streak(8), 2); // wraps to cycle-day 1
        assert_eq!(cfg.credits_for_streak(15), 2); // wraps again
    }

    #[test]
    fn cycle_day_wraps_at_seven() {
        assert_eq!(CheckInConfig::cycle_day(7), 7);
        assert_eq!(CheckInConfig::cycle_day(8), 1);
    }

    #[test]
    fn is_weekly_chest_only_on_cycle_day_seven() {
        assert!(CheckInConfig::is_weekly_chest(7));
        assert!(!CheckInConfig::is_weekly_chest(8));
    }

    #[test]
    fn parse_weekly_credits_round_trips_valid() {
        assert_eq!(
            CheckInConfig::parse_weekly_credits("2,3,4,4,4,4,10"),
            vec![2, 3, 4, 4, 4, 4, 10]
        );
    }

    #[test]
    fn parse_weekly_credits_short_falls_back_to_default() {
        assert_eq!(
            CheckInConfig::parse_weekly_credits("1,2,3"),
            DEFAULT_WEEKLY_CREDITS.to_vec()
        );
    }

    #[test]
    fn from_config_value_none_falls_back_to_default() {
        assert_eq!(
            CheckInConfig::from_config_value(None).weekly_credits,
            DEFAULT_WEEKLY_CREDITS.to_vec()
        );
    }
}
