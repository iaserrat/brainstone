use serde_json::{Value, json};

const DESCRIPTION: &str = "Schedule durable follow-up work: one run, or a standing \
schedule the runtime re-runs itself until it is cancelled. Set exactly one of run_at, \
after_seconds, repeat_time, or repeat_every_minutes. repeat_time is a calendar schedule: \
every day, or with repeat_days on those weekdays, with repeat_month_day on a day of the \
month, with repeat_nth on one weekday's nth occurrence in the month; repeat_interval \
stretches any of them to every N days, weeks, or months. Every run replies in this \
conversation unless conversation names another one to post in.";

/// The `Schedule` tool definition; `min_interval_minutes` is the operator's floor.
pub(crate) fn definition(name: &str, min_interval_minutes: u32) -> Value {
    json!({
        "name": name,
        "description": DESCRIPTION,
        "input_schema": {
            "type": "object",
            "properties": {
                "prompt": {"type": "string", "minLength": 1},
                "run_at": {"type": "string", "description": "RFC 3339 instant with offset, for one run"},
                "after_seconds": {"type": "integer", "minimum": 0, "description": "Delay from now, for one run"},
                "repeat_every_minutes": {"type": "integer", "minimum": min_interval_minutes, "description": "Run every this many minutes, counted from now (60 = hourly). Elapsed time: daylight saving does not shift it. Takes no other repeat_* field"},
                "repeat_time": {"type": "string", "description": "24-hour HH:MM local time for a calendar schedule; needs repeat_time_zone. Alone it runs every day"},
                "repeat_time_zone": {"type": "string", "description": "IANA zone for repeat_time, such as America/Toronto; use requester_tz from thread_context unless the user names another zone"},
                "repeat_days": {"type": "array", "items": {"type": "integer", "minimum": 1, "maximum": 7}, "description": "ISO weekdays, 1 = Monday to 7 = Sunday: run on these days each week. With repeat_nth, exactly one day"},
                "repeat_interval": {"type": "integer", "minimum": 1, "maximum": runtime::MAX_REPEAT_INTERVAL, "description": "Every this many days, weeks (with repeat_days), or months (with repeat_month_day or repeat_nth), counted from the first run; default 1"},
                "repeat_month_day": {"type": "integer", "minimum": 1, "maximum": 31, "description": "Day of the month to run on; a shorter month runs on its last day, so 31 means the last day. Not with repeat_days"},
                "repeat_nth": {"type": "integer", "minimum": -1, "maximum": 4, "description": "With exactly one repeat_days entry: that weekday's 1st to 4th occurrence in the month, or -1 for the last; 1 with [1] is the first Monday"},
                "conversation": {"type": "string", "minLength": 1, "description": "Post each run as a new message in this conversation instead of replying here. An id from the requester's own message, which must name it; never invent one, and only when the requester asks you to post there. The surface decides whether this requester may post there. With after_seconds 0 it posts now"}
            },
            "required": ["prompt"],
            "additionalProperties": false
        }
    })
}
