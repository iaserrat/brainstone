use jiff::civil::Time;
use serde::{Deserialize, Serialize};

use crate::Error;

#[path = "schedule_repeat_calendar.rs"]
mod calendar;
#[path = "schedule_repeat_spec.rs"]
mod spec;

pub use spec::RepeatSpec;

#[cfg(test)]
#[path = "schedule_repeat_tests.rs"]
mod tests;

/// Standing schedules one lane may hold; bounds what a runaway agent can create.
pub const MAX_REPEAT_PER_LANE: i64 = 20;
/// The largest `interval` a calendar rule accepts: every 52 days, weeks, or months.
pub const MAX_REPEAT_INTERVAL: u16 = 52;
const MILLIS_PER_MINUTE: i64 = 60_000;

/// A standing schedule's rule: fixed elapsed minutes, or a local wall-clock
/// time on a calendar pattern. A rule holds no start instant: intervals and
/// every-N calendar rules count from the anchor, the moment the schedule was
/// created, which the store records beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RepeatSpec", into = "RepeatSpec")]
pub struct Recurrence(Rule);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Rule {
    /// Elapsed time, so daylight saving never moves it.
    EveryMinutes(u32),
    Calendar(calendar::Calendar),
}

impl Recurrence {
    /// A local time every day, or only on ISO weekdays `days`.
    ///
    /// # Errors
    /// Returns `invalid_input` for a time with seconds, an unknown IANA zone,
    /// or an empty or out-of-range weekday list.
    pub fn new(time: Time, time_zone: &str, days: Option<Vec<i8>>) -> Result<Self, Error> {
        Self::try_from(RepeatSpec {
            time: Some(time),
            time_zone: Some(time_zone.to_owned()),
            days,
            ..RepeatSpec::default()
        })
    }

    /// The first occurrence strictly after `after_unix_ms`, so missed
    /// occurrences collapse into one catch-up run and drift never accumulates.
    ///
    /// # Errors
    /// Returns `corrupt_record` when the stored zone or instant is unusable.
    pub fn next_run_at(&self, after_unix_ms: i64, anchor_unix_ms: i64) -> Result<i64, Error> {
        match &self.0 {
            Rule::EveryMinutes(minutes) => every(*minutes, after_unix_ms, anchor_unix_ms),
            Rule::Calendar(calendar) => calendar.next_run_at(after_unix_ms, anchor_unix_ms),
        }
    }
}

/// `anchor + k * step` for the smallest `k >= 1` landing strictly after `after`.
fn every(minutes: u32, after: i64, anchor: i64) -> Result<i64, Error> {
    let step = i64::from(minutes) * MILLIS_PER_MINUTE;
    let steps = after.saturating_sub(anchor).div_euclid(step).max(0) + 1;
    steps
        .checked_mul(step)
        .and_then(|offset| anchor.checked_add(offset))
        .ok_or_else(|| Error::corrupt("interval run time overflows"))
}

/// Wall-clock milliseconds since the Unix epoch, the "now" a recurrence advances from.
pub(crate) fn now_unix_ms() -> i64 {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is after Unix epoch")
        .as_millis();
    i64::try_from(millis).expect("current timestamp fits i64")
}

fn invalid(field: &'static str) -> Error {
    Error::InvalidInput { field }
}
