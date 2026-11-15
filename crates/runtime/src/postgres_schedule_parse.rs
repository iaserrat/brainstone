use sqlx::types::Json;

use crate::{Error, ScheduleId, ScheduleState, ScheduleTime, Scheduled};

/// One `scheduled_work` row: id, prompt, run time, state, kind, value, the
/// three legacy `repeat_*` columns (minute of day, zone, weekdays), the
/// `recurrence` JSON, the owner (the creating operation's actor), the
/// `destination` JSON, then whether a stopped thread pauses it.
pub(crate) type Row = (
    i64,
    String,
    i64,
    String,
    String,
    i64,
    Option<i16>,
    Option<String>,
    Option<Vec<i16>>,
    Option<Json<serde_json::Value>>,
    Option<String>,
    Option<Json<serde_json::Value>>,
    bool,
);

/// What every query reading a [`Row`] selects from `scheduled_work s`, in
/// order. A macro because sqlx takes only `&'static str`: `concat!` joins it.
macro_rules! schedule_columns {
    () => {
        "s.id, s.prompt, s.run_at_unix_ms, s.state, s.schedule_kind, s.schedule_value, \
         s.repeat_minute, s.repeat_time_zone, s.repeat_days, s.recurrence, \
         (SELECT o.execution_identity->>'actor' FROM operations o \
         WHERE o.id = s.origin_operation_id), s.destination, \
         (s.destination IS NULL AND NOT COALESCE((SELECT l.follow_enabled FROM lanes l \
         WHERE l.id = s.lane_id), TRUE))"
    };
}
pub(crate) use schedule_columns;

pub(crate) const MILLIS_PER_SECOND: i64 = 1_000;

/// The timing a stored row was created with, for idempotent-retry comparison.
pub(crate) fn time(row: &Row) -> Result<ScheduleTime, Error> {
    match row.4.as_str() {
        "at" => Ok(ScheduleTime::AtUnixMillisecond(row.5)),
        "after" => u64::try_from(row.5 / MILLIS_PER_SECOND)
            .map(ScheduleTime::AfterSeconds)
            .map_err(Error::corrupt),
        "repeat" | "recur" => recurrence(row)?
            .map(ScheduleTime::Repeat)
            .ok_or_else(|| Error::corrupt("standing schedule without a recurrence")),
        other => Err(Error::corrupt(format!("unknown schedule kind: {other}"))),
    }
}

pub(crate) fn scheduled(row: &Row) -> Result<Scheduled, Error> {
    Ok(Scheduled {
        id: ScheduleId(row.0),
        prompt: row.1.clone(),
        run_at_unix_ms: row.2,
        state: state(&row.3)?,
        repeat: recurrence(row)?,
        owner: row.10.clone(),
        destination: destination(row)?.map(|destination| destination.identity.conversation),
        paused: row.12,
    })
}

/// Where the row's runs post, when they do not reply in place.
pub(crate) fn destination(row: &Row) -> Result<Option<crate::Destination>, Error> {
    row.11
        .as_ref()
        .map(|Json(value)| serde_json::from_value(value.clone()).map_err(Error::corrupt))
        .transpose()
}

fn recurrence(row: &Row) -> Result<Option<crate::Recurrence>, Error> {
    let legacy = (row.6, row.7.as_deref(), row.8.as_deref());
    crate::stored_recurrence::decode(row.9.as_ref(), legacy)
}

fn state(value: &str) -> Result<ScheduleState, Error> {
    match value {
        "pending" => Ok(ScheduleState::Pending),
        "cancelled" => Ok(ScheduleState::Cancelled),
        "released" => Ok(ScheduleState::Released),
        _ => Err(Error::CorruptRecord {
            message: format!("unknown schedule state: {value}"),
        }),
    }
}
