use std::sync::Arc;

use agent::Registry;
use runtime::ScheduleStore;
use surface::Destinations;

/// `min_interval_minutes` is the operator's floor for `repeat_every_minutes`;
/// `destinations` is the surface that decides where a schedule may post.
#[must_use]
pub fn registry<S: ScheduleStore + 'static, D: Destinations + 'static>(
    store: S,
    base: &Registry,
    min_interval_minutes: u32,
    destinations: D,
) -> Registry {
    base.extend(vec![
        Arc::new(crate::schedule_tool::ScheduleTool {
            store: store.clone(),
            min_interval_minutes,
            destinations,
        }),
        Arc::new(crate::list_tool::ListSchedules(store.clone())),
        Arc::new(crate::cancel_tool::CancelSchedule(store)),
    ])
}
