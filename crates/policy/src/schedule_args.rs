use runtime::{Recurrence, RepeatSpec, ScheduleTime};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScheduleArgs {
    pub(crate) prompt: String,
    pub(crate) run_at: Option<String>,
    pub(crate) after_seconds: Option<u64>,
    pub(crate) repeat_time: Option<String>,
    pub(crate) repeat_time_zone: Option<String>,
    pub(crate) repeat_days: Option<Vec<i8>>,
    pub(crate) repeat_every_minutes: Option<u32>,
    pub(crate) repeat_interval: Option<u16>,
    pub(crate) repeat_month_day: Option<i8>,
    pub(crate) repeat_nth: Option<i8>,
    pub(crate) conversation: Option<String>,
}

impl ScheduleArgs {
    /// `min_interval_minutes` is the operator's floor for fixed intervals.
    pub(crate) fn time(&self, min_interval_minutes: u32) -> Result<ScheduleTime, agent::ToolError> {
        let timing = (
            &self.run_at,
            self.after_seconds,
            &self.repeat_time,
            self.repeat_every_minutes,
        );
        match timing {
            (Some(value), None, None, None) => absolute(value),
            (None, Some(value), None, None) => Ok(ScheduleTime::AfterSeconds(value)),
            (None, None, Some(value), None) => self.repeat(Some(parse_time(value)?), None),
            (None, None, None, Some(minutes)) => self.every(minutes, min_interval_minutes),
            _ => Err(invalid(
                "set exactly one of run_at, after_seconds, repeat_time, or repeat_every_minutes",
            )),
        }
    }

    fn every(
        &self,
        minutes: u32,
        min_interval_minutes: u32,
    ) -> Result<ScheduleTime, agent::ToolError> {
        if minutes < min_interval_minutes {
            return Err(too_short(min_interval_minutes));
        }
        self.repeat(None, Some(minutes))
    }

    /// Every `repeat_*` field goes to the runtime, which refuses combinations
    /// a rule cannot hold rather than dropping them.
    fn repeat(
        &self,
        time: Option<jiff::civil::Time>,
        every_minutes: Option<u32>,
    ) -> Result<ScheduleTime, agent::ToolError> {
        if time.is_some() && self.repeat_time_zone.is_none() {
            return Err(invalid(
                "repeat_time needs repeat_time_zone, an IANA zone name",
            ));
        }
        let spec = RepeatSpec {
            every_minutes,
            time,
            time_zone: self.repeat_time_zone.clone(),
            days: self.repeat_days.clone(),
            interval: self.repeat_interval,
            month_day: self.repeat_month_day,
            nth: self.repeat_nth,
        };
        Recurrence::try_from(spec)
            .map(ScheduleTime::Repeat)
            .map_err(|error| crate::tool_support::runtime(&error))
    }
}

fn parse_time(value: &str) -> Result<jiff::civil::Time, agent::ToolError> {
    value
        .parse::<jiff::civil::Time>()
        .map_err(|_| invalid("repeat_time must be a 24-hour HH:MM local time"))
}

fn absolute(value: &str) -> Result<ScheduleTime, agent::ToolError> {
    value
        .parse::<jiff::Timestamp>()
        .map(|value| ScheduleTime::AtUnixMillisecond(value.as_millisecond()))
        .map_err(|_| invalid("run_at must be an RFC 3339 instant with an offset"))
}

fn too_short(min_interval_minutes: u32) -> agent::ToolError {
    agent::ToolError::new(
        "schedule_interval_too_short",
        format!("repeat_every_minutes must be at least {min_interval_minutes}"),
    )
}

pub(crate) fn invalid(message: impl Into<String>) -> agent::ToolError {
    agent::ToolError::new("invalid_schedule_arguments", message)
}
