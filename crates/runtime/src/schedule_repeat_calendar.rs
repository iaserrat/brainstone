use jiff::Timestamp;
use jiff::civil::{Date, Time, Weekday};
use jiff::tz::TimeZone;

use super::{MAX_REPEAT_INTERVAL, RepeatSpec, invalid};
use crate::Error;

/// Every period of a valid rule holds an occurrence, so two whole periods of
/// the longest rule (52 months) always contain the next one.
const SEARCH_DAYS: u16 = 31 * (MAX_REPEAT_INTERVAL + 2);
const SECONDS_PER_DAY: i64 = 86_400;
const DAYS_PER_WEEK: i64 = 7;
const MONTHS_PER_YEAR: i64 = 12;
const EPOCH: Date = Date::constant(1970, 1, 1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Calendar {
    time: Time,
    time_zone: String,
    every: u16,
    period: Period,
}

/// Which dates of a period run. Each variant matches a date in every period.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Period {
    Days,
    Weeks(Vec<Weekday>),
    MonthDay(i8),
    /// First to fourth, or last (-1), `weekday` of the month.
    MonthWeekday {
        nth: i8,
        weekday: Weekday,
    },
}

impl Calendar {
    /// The first run after `anchor` fixes which periods run; every `every`th
    /// period from it does.
    pub(super) fn next_run_at(&self, after: i64, anchor: i64) -> Result<i64, Error> {
        let zone = TimeZone::get(&self.time_zone).map_err(Error::corrupt)?;
        let first = self.search(&zone, anchor, None)?;
        let phase = self.index(first.date());
        self.search(&zone, after, Some(phase))
            .map(|found| found.timestamp().as_millisecond())
    }

    fn search(
        &self,
        zone: &TimeZone,
        after: i64,
        phase: Option<i64>,
    ) -> Result<jiff::Zoned, Error> {
        let after = Timestamp::from_millisecond(after).map_err(Error::corrupt)?;
        let mut date = after.to_zoned(zone.clone()).date();
        for _ in 0..SEARCH_DAYS {
            let occurrence = zone
                .to_zoned(date.to_datetime(self.time))
                .map_err(Error::corrupt)?;
            if self.runs_on(date, phase) && occurrence.timestamp() > after {
                return Ok(occurrence);
            }
            date = date.tomorrow().map_err(Error::corrupt)?;
        }
        Err(Error::corrupt("no occurrence within the search window"))
    }

    fn runs_on(&self, date: Date, phase: Option<i64>) -> bool {
        let in_phase = phase
            .is_none_or(|phase| (self.index(date) - phase).rem_euclid(i64::from(self.every)) == 0);
        in_phase && self.period.matches(date)
    }

    /// The period `date` falls in, counted from the Unix epoch.
    fn index(&self, date: Date) -> i64 {
        let days = date
            .duration_since(EPOCH)
            .as_secs()
            .div_euclid(SECONDS_PER_DAY);
        match self.period {
            Period::Days => days,
            Period::Weeks(_) => {
                (days - i64::from(date.weekday().to_monday_zero_offset())).div_euclid(DAYS_PER_WEEK)
            }
            Period::MonthDay(_) | Period::MonthWeekday { .. } => {
                i64::from(date.year()) * MONTHS_PER_YEAR + i64::from(date.month())
            }
        }
    }
}

#[path = "schedule_repeat_calendar_spec.rs"]
mod fields;
#[path = "schedule_repeat_period.rs"]
mod period;
