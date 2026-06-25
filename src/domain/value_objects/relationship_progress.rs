//! Relationship-meter progress (spec 2026-06-24 §B.1 / §C.3).
//!
//! Pure calc over `(level, message_count, thresholds)` so the frontend meter never hardcodes
//! thresholds. `fraction` is progress *within the current segment* toward the next level (0..1).

use crate::domain::value_objects::{RelationshipLevel, RelationshipThresholds};

#[derive(Debug, Clone)]
pub struct RelationshipProgress {
    /// Progress within the current level segment toward the next level, clamped to `0.0..=1.0`.
    pub fraction: f32,
    /// Thai label of the next level, or `None` at the top level.
    pub next_level_label: Option<String>,
    /// Messages remaining to the next level, or `None` at the top level.
    pub messages_to_next: Option<i32>,
}

impl RelationshipProgress {
    pub fn compute(
        level: &RelationshipLevel,
        message_count: i32,
        thresholds: &RelationshipThresholds,
    ) -> Self {
        let bounds = match level {
            RelationshipLevel::Stranger => Some((0_i64, thresholds.acquaintance_at() as i64)),
            RelationshipLevel::Acquaintance => Some((
                thresholds.acquaintance_at() as i64,
                thresholds.friend_at() as i64,
            )),
            RelationshipLevel::Friend => Some((
                thresholds.friend_at() as i64,
                thresholds.close_friend_at() as i64,
            )),
            RelationshipLevel::CloseFriend => None,
        };

        match bounds {
            None => Self {
                fraction: 1.0,
                next_level_label: None,
                messages_to_next: None,
            },
            Some((prev, next)) => {
                let span = (next - prev).max(1);
                let into = (message_count as i64 - prev).clamp(0, span);
                Self {
                    fraction: into as f32 / span as f32,
                    next_level_label: level.next().map(|l| l.label_th().to_string()),
                    messages_to_next: Some((next - message_count as i64).max(0) as i32),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thresholds() -> RelationshipThresholds {
        RelationshipThresholds::new(8, 24, 70).unwrap()
    }

    #[test]
    fn stranger_midway_to_acquaintance() {
        let p = RelationshipProgress::compute(&RelationshipLevel::Stranger, 4, &thresholds());
        assert!((p.fraction - 0.5).abs() < 1e-6);
        assert_eq!(p.messages_to_next, Some(4));
        assert_eq!(p.next_level_label.as_deref(), Some("คนรู้จัก"));
    }

    #[test]
    fn acquaintance_segment_is_local() {
        // count 12 within [8, 24): (12-8)/(24-8) = 0.25, 12 to go
        let p = RelationshipProgress::compute(&RelationshipLevel::Acquaintance, 12, &thresholds());
        assert!((p.fraction - 0.25).abs() < 1e-6);
        assert_eq!(p.messages_to_next, Some(12));
        assert_eq!(p.next_level_label.as_deref(), Some("เพื่อน"));
    }

    #[test]
    fn close_friend_is_maxed() {
        let p = RelationshipProgress::compute(&RelationshipLevel::CloseFriend, 200, &thresholds());
        assert_eq!(p.fraction, 1.0);
        assert_eq!(p.messages_to_next, None);
        assert_eq!(p.next_level_label, None);
    }

    #[test]
    fn overshoot_clamps() {
        // legacy catch-up: Stranger with count beyond threshold (level not yet advanced)
        let p = RelationshipProgress::compute(&RelationshipLevel::Stranger, 50, &thresholds());
        assert_eq!(p.fraction, 1.0);
        assert_eq!(p.messages_to_next, Some(0));
    }
}
