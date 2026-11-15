//! Who owns a schedule: the authenticated principal of the operation that
//! created it. Unauthenticated schedules have no owner and stay lane-scoped.
use agent::ExecutionIdentity;

/// Standing schedules one owner may hold across every lane.
pub const MAX_REPEAT_PER_OWNER: i64 = 50;
/// Schedules that post elsewhere one owner may create in a trailing hour.
pub const MAX_POSTS_PER_OWNER_PER_HOUR: i64 = 10;
pub(crate) const HOUR_MS: i64 = 3_600_000;

/// Whether `caller` may reach `owner`'s schedules from anywhere: only the same
/// principal, and only from a conversation private to that principal, so a
/// shared conversation never lists one person's schedules to everyone in it.
pub(crate) fn owner_in_private_conversation(
    caller: Option<&ExecutionIdentity>,
    owner: Option<&ExecutionIdentity>,
) -> bool {
    let (Some(caller), Some(owner)) = (caller, owner) else {
        return false;
    };
    caller.private && same_owner(caller, owner)
}

pub(crate) fn same_owner(a: &ExecutionIdentity, b: &ExecutionIdentity) -> bool {
    a.provider == b.provider && a.tenant == b.tenant && a.actor == b.actor
}
