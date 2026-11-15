use crate::{Error, ScheduleTime};

pub(crate) fn run_at(when: &ScheduleTime, now: i64) -> Result<i64, Error> {
    match when {
        ScheduleTime::AtUnixMillisecond(value) => Ok(*value),
        ScheduleTime::AfterSeconds(seconds) => {
            let millis = i64::try_from(*seconds)
                .ok()
                .and_then(|value| value.checked_mul(1_000));
            millis
                .and_then(|value| now.checked_add(value))
                .ok_or(Error::InvalidInput {
                    field: "after_seconds",
                })
        }
        ScheduleTime::Repeat(recurrence) => recurrence.next_run_at(now, now),
    }
}
