//! A standing schedule's failure streak: when it speaks and when it stops.

/// Consecutive failed runs before the failure delivery carries a nudge.
pub const REPEAT_FAILURE_NUDGE_AFTER: i16 = 3;
/// Consecutive failed runs before the runtime cancels the schedule.
pub const REPEAT_FAILURE_CANCEL_AFTER: i16 = 14;
/// What a failure delivery says about a standing schedule's streak, if anything.
#[must_use]
pub fn failure_notice(consecutive_failures: Option<i16>) -> Option<String> {
    let failures = consecutive_failures?;
    if failures >= REPEAT_FAILURE_CANCEL_AFTER {
        return Some(format!(
            "This schedule failed {failures} runs in a row and has been cancelled."
        ));
    }
    if failures >= REPEAT_FAILURE_NUDGE_AFTER {
        return Some(format!(
            "This schedule has failed {failures} runs in a row. \
             Cancel it with CancelSchedule or tell me what to change."
        ));
    }
    None
}
