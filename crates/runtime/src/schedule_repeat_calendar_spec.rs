//! A calendar rule to and from its `RepeatSpec` fields.

use jiff::civil::Time;
use jiff::tz::TimeZone;

use super::period::period;
use super::{Calendar, MAX_REPEAT_INTERVAL, RepeatSpec, invalid};
use crate::Error;

impl Calendar {
    pub(crate) fn new(time: Time, spec: &RepeatSpec) -> Result<Self, Error> {
        if time.second() != 0 || time.subsec_nanosecond() != 0 {
            return Err(invalid("repeat_time"));
        }
        let every = spec.interval.unwrap_or(1);
        if !(1..=MAX_REPEAT_INTERVAL).contains(&every) {
            return Err(invalid("repeat_interval"));
        }
        Ok(Self {
            time,
            time_zone: zone(spec.time_zone.as_deref())?,
            every,
            period: period(spec)?,
        })
    }

    pub(crate) fn spec(self) -> RepeatSpec {
        let mut spec = self.period.spec();
        spec.time = Some(self.time);
        spec.time_zone = Some(self.time_zone);
        spec.interval = (self.every > 1).then_some(self.every);
        spec
    }
}

fn zone(time_zone: Option<&str>) -> Result<String, Error> {
    let time_zone = time_zone.ok_or_else(|| invalid("repeat_time_zone"))?;
    TimeZone::get(time_zone).map_err(|_| invalid("repeat_time_zone"))?;
    Ok(time_zone.to_owned())
}
