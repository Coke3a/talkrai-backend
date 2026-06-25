//! Wall-clock helpers.
//!
//! Thailand is permanently UTC+7 (no DST), so the Asia/Bangkok civil date is a fixed-offset
//! calculation — no `chrono-tz` dependency needed (spec §C: all day-boundary math in ICT).

use chrono::{Duration, NaiveDate, Utc};

/// The current civil date in Asia/Bangkok (UTC+7).
pub fn bangkok_today() -> NaiveDate {
    (Utc::now() + Duration::hours(7)).date_naive()
}
