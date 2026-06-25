//! Daily check-in value objects (spec 2026-06-24 §C.3).
//!
//! Pure: no IO, no clock. The usecase builds [`CheckInConfig`] from `app_config` and supplies it
//! to [`crate::domain::entities::User::check_in`].

/// Tunable check-in reward curve, built from `app_config` in the usecase layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckInConfig {
    pub base_credits: i32,
    pub per_day_bonus: i32,
    pub max_streak_for_bonus: i32,
    /// `(streak_day, bonus_credits)` pairs, e.g. `[(7, 20), (14, 30), (30, 60)]`.
    pub milestone_bonuses: Vec<(i32, i32)>,
    pub daily_cap: i32,
}

impl CheckInConfig {
    /// Credits granted for reaching `streak` consecutive days.
    ///
    /// `base + per_day_bonus * (min(streak, cap) - 1)` plus any milestone bonus for that exact
    /// day, clamped to `daily_cap`.
    pub fn credits_for_streak(&self, streak: i32) -> i32 {
        let ramped = self.base_credits
            + self.per_day_bonus * (streak.min(self.max_streak_for_bonus) - 1).max(0);
        let milestone = self
            .milestone_bonuses
            .iter()
            .find(|(day, _)| *day == streak)
            .map(|(_, bonus)| *bonus)
            .unwrap_or(0);
        (ramped + milestone).min(self.daily_cap)
    }

    /// Parse the `daily_checkin_milestone_bonuses` config string ("7:20,14:30,30:60") into pairs.
    /// Malformed entries are skipped rather than failing the whole config.
    pub fn parse_milestones(raw: &str) -> Vec<(i32, i32)> {
        raw.split(',')
            .filter_map(|pair| {
                let (day, bonus) = pair.trim().split_once(':')?;
                Some((day.trim().parse().ok()?, bonus.trim().parse().ok()?))
            })
            .collect()
    }
}

/// Result of a check-in attempt for one day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckInOutcome {
    pub already_checked_in: bool,
    pub credits_awarded: i32,
    pub new_streak: i32,
    pub crossed_milestone: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> CheckInConfig {
        CheckInConfig {
            base_credits: 4,
            per_day_bonus: 1,
            max_streak_for_bonus: 10,
            milestone_bonuses: vec![(7, 20), (14, 30), (30, 60)],
            daily_cap: 30,
        }
    }

    #[test]
    fn day_one_is_base() {
        assert_eq!(cfg().credits_for_streak(1), 4);
    }

    #[test]
    fn ramps_per_day() {
        // day 3: 4 + 1*(3-1) = 6
        assert_eq!(cfg().credits_for_streak(3), 6);
    }

    #[test]
    fn ramp_caps_at_max_streak() {
        // streak 20 ramps as if 10: 4 + 1*(10-1) = 13
        assert_eq!(cfg().credits_for_streak(20), 13);
    }

    #[test]
    fn milestone_adds_bonus() {
        // day 7: ramp 4 + 1*6 = 10, + milestone 20 = 30 (== cap)
        assert_eq!(cfg().credits_for_streak(7), 30);
    }

    #[test]
    fn daily_cap_clamps() {
        // day 30: ramp 13 + milestone 60 = 73 -> clamped to 30
        assert_eq!(cfg().credits_for_streak(30), 30);
    }

    #[test]
    fn parse_milestones_ok() {
        assert_eq!(
            CheckInConfig::parse_milestones("7:20,14:30,30:60"),
            vec![(7, 20), (14, 30), (30, 60)]
        );
    }

    #[test]
    fn parse_milestones_skips_garbage() {
        assert_eq!(
            CheckInConfig::parse_milestones("7:20, bad, 14:x, 30:60"),
            vec![(7, 20), (30, 60)]
        );
    }

    #[test]
    fn parse_milestones_empty() {
        assert_eq!(
            CheckInConfig::parse_milestones(""),
            Vec::<(i32, i32)>::new()
        );
    }
}
