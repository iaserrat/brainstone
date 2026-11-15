use jiff::civil::Time;
use serde::{Deserialize, Serialize};

use super::{Recurrence, Rule, calendar::Calendar, invalid};
use crate::Error;

/// A standing schedule's rule as fields, the shape a caller writes and reads
/// back. It is the RFC 5545 subset the runtime supports: `every_minutes` is
/// FREQ=MINUTELY; otherwise `time` in `time_zone` runs every day, on `days`
/// (BYDAY), on `month_day` (BYMONTHDAY), or on the `nth` of one weekday
/// (BYDAY with an ordinal), every `interval` (INTERVAL) days, weeks, or months.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepeatSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Run every this many minutes (`FREQ=MINUTELY`).
    pub every_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The local time of day each run happens.
    pub time: Option<Time>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The IANA zone `time` is read in.
    pub time_zone: Option<String>,
    /// ISO weekdays, 1 = Monday through 7 = Sunday.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days: Option<Vec<i8>>,
    /// Every this many days, weeks, or months; one when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<u16>,
    /// 1 to 31; a day past the month's end runs on its last day.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month_day: Option<i8>,
    /// With exactly one of `days`: its first to fourth, or last (-1), in the month.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nth: Option<i8>,
}

impl RepeatSpec {
    fn calendar_free(&self) -> bool {
        self.time_zone.is_none()
            && self.days.is_none()
            && self.interval.is_none()
            && self.month_day.is_none()
            && self.nth.is_none()
    }
}

impl TryFrom<RepeatSpec> for Recurrence {
    type Error = Error;

    fn try_from(spec: RepeatSpec) -> Result<Self, Error> {
        match (spec.every_minutes, spec.time) {
            (Some(minutes), None) if minutes > 0 && spec.calendar_free() => {
                Ok(Self(Rule::EveryMinutes(minutes)))
            }
            (None, Some(time)) => Calendar::new(time, &spec).map(|rule| Self(Rule::Calendar(rule))),
            (None, None) => Err(invalid("repeat_time")),
            _ => Err(invalid("repeat_every_minutes")),
        }
    }
}

impl From<Recurrence> for RepeatSpec {
    fn from(recurrence: Recurrence) -> Self {
        match recurrence.0 {
            Rule::EveryMinutes(minutes) => Self {
                every_minutes: Some(minutes),
                ..Self::default()
            },
            Rule::Calendar(calendar) => calendar.spec(),
        }
    }
}
