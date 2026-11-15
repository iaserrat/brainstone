//! A stored standing rule: the `recurrence` JSON of a 'recur' row, or the
//! `repeat_*` columns (minute of day, IANA zone, ISO weekdays) of a 'repeat' row.

use jiff::civil::Time;
use sqlx::types::Json;

use crate::{Error, Recurrence};

const MINUTES_PER_HOUR: i16 = 60;

/// The `repeat_*` columns of a row written before rules were stored as JSON.
pub(crate) type Legacy<'a> = (Option<i16>, Option<&'a str>, Option<&'a [i16]>);

/// `None` for a one-shot row; `corrupt_record` when the stored rule is unusable.
pub(crate) fn decode(
    recurrence: Option<&Json<serde_json::Value>>,
    legacy: Legacy<'_>,
) -> Result<Option<Recurrence>, Error> {
    match recurrence {
        Some(Json(value)) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|error| Error::corrupt(format!("stored recurrence is invalid: {error}"))),
        None => decode_legacy(legacy),
    }
}

pub(crate) fn encode(recurrence: &Recurrence) -> Result<Json<serde_json::Value>, Error> {
    serde_json::to_value(recurrence)
        .map(Json)
        .map_err(Error::corrupt)
}

fn decode_legacy((minute, time_zone, days): Legacy<'_>) -> Result<Option<Recurrence>, Error> {
    let (Some(minute), Some(time_zone)) = (minute, time_zone) else {
        return Ok(None);
    };
    let hour = i8::try_from(minute / MINUTES_PER_HOUR).map_err(Error::corrupt)?;
    let minute = i8::try_from(minute % MINUTES_PER_HOUR).map_err(Error::corrupt)?;
    let time = Time::new(hour, minute, 0, 0).map_err(Error::corrupt)?;
    let days = days
        .map(|days| days.iter().map(|day| i8::try_from(*day)).collect())
        .transpose()
        .map_err(Error::corrupt)?;
    Recurrence::new(time, time_zone, days)
        .map(Some)
        .map_err(|error| Error::corrupt(format!("stored recurrence is {error}")))
}
