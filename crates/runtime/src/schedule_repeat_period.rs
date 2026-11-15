use jiff::civil::{Date, Weekday};

use super::Period;
use crate::Error;
use crate::schedule_repeat::{RepeatSpec, invalid};

const LAST: i8 = -1;
const MAX_NTH: i8 = 4;
const MAX_MONTH_DAY: i8 = 31;

pub(super) fn period(spec: &RepeatSpec) -> Result<Period, Error> {
    let days = spec.days.as_deref().map(weekdays).transpose()?;
    match (days, spec.month_day, spec.nth) {
        (None, None, None) => Ok(Period::Days),
        (Some(days), None, None) => Ok(Period::Weeks(days)),
        (None, Some(day), None) => month_day(day),
        (Some(days), None, Some(nth)) => month_weekday(&days, nth),
        (None, None, Some(_)) => Err(invalid("repeat_nth")),
        _ => Err(invalid("repeat_month_day")),
    }
}

fn weekdays(days: &[i8]) -> Result<Vec<Weekday>, Error> {
    if days.is_empty() {
        return Err(invalid("repeat_days"));
    }
    days.iter()
        .map(|day| Weekday::from_monday_one_offset(*day).map_err(|_| invalid("repeat_days")))
        .collect()
}

fn month_day(day: i8) -> Result<Period, Error> {
    (1..=MAX_MONTH_DAY)
        .contains(&day)
        .then_some(Period::MonthDay(day))
        .ok_or_else(|| invalid("repeat_month_day"))
}

/// Fifth weekdays are refused: most months lack one, so a period could hold
/// no occurrence.
fn month_weekday(days: &[Weekday], nth: i8) -> Result<Period, Error> {
    let [weekday] = days else {
        return Err(invalid("repeat_days"));
    };
    (nth == LAST || (1..=MAX_NTH).contains(&nth))
        .then_some(Period::MonthWeekday {
            nth,
            weekday: *weekday,
        })
        .ok_or_else(|| invalid("repeat_nth"))
}

impl Period {
    pub(super) fn matches(&self, date: Date) -> bool {
        match self {
            Self::Days => true,
            Self::Weeks(days) => days.contains(&date.weekday()),
            Self::MonthDay(day) => date.day() == (*day).min(date.days_in_month()),
            Self::MonthWeekday { nth, weekday } => date
                .nth_weekday_of_month(*nth, *weekday)
                .is_ok_and(|found| found == date),
        }
    }

    /// The fields this period was written with; time, zone, and interval are the caller's.
    pub(super) fn spec(&self) -> RepeatSpec {
        let days = |days: &[Weekday]| days.iter().map(|day| day.to_monday_one_offset()).collect();
        match self {
            Self::Days => RepeatSpec::default(),
            Self::Weeks(list) => RepeatSpec {
                days: Some(days(list)),
                ..RepeatSpec::default()
            },
            Self::MonthDay(day) => RepeatSpec {
                month_day: Some(*day),
                ..RepeatSpec::default()
            },
            Self::MonthWeekday { nth, weekday } => RepeatSpec {
                days: Some(days(&[*weekday])),
                nth: Some(*nth),
                ..RepeatSpec::default()
            },
        }
    }
}
