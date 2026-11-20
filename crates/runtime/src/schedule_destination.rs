use agent::ExecutionIdentity;
use serde::{Deserialize, Serialize};

use crate::schedule_owner::same_owner;
use crate::{DeliveryTarget, Error};

/// Where each run of a schedule speaks when it does not reply in its origin
/// lane: a new conversation root in `conversation`, a session of `surface`.
/// Every run opens its own lane there, which the surface binds to the root it
/// posts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Destination {
    /// The surface the run posts in.
    pub surface: String,
    /// The surface's id for that conversation.
    pub conversation: String,
    /// The surface's routing record for a new root in that conversation.
    pub target: DeliveryTarget,
    /// The principal every run executes as: the owner, in that conversation.
    pub identity: ExecutionIdentity,
}

/// Whether `caller` is in the conversation `destination` posts into: anyone
/// who can read a post there may list and cancel the schedule producing it.
pub(crate) fn in_destination(
    caller: Option<&ExecutionIdentity>,
    destination: Option<&Destination>,
) -> bool {
    let (Some(caller), Some(destination)) = (caller, destination) else {
        return false;
    };
    let target = &destination.identity;
    caller.provider == target.provider
        && caller.tenant == target.tenant
        && caller.conversation == target.conversation
}

/// What the runtime knows about the operation creating a schedule.
pub(crate) struct Creator<'a> {
    pub(crate) identity: Option<&'a ExecutionIdentity>,
    /// Surface-admitted work has a delivery target; subagent children do not.
    pub(crate) has_target: bool,
    pub(crate) scheduled_run: bool,
}

/// A schedule may post elsewhere only when a person asked for it: an
/// authenticated operation admitted from a surface, posting as that same
/// person into a conversation that is not private to them. A scheduled run or
/// a subagent cannot, since its instructions were written by the model.
pub(crate) fn check(creator: &Creator<'_>, destination: &Destination) -> Result<(), Error> {
    let person = creator
        .identity
        .filter(|_| creator.has_target && !creator.scheduled_run);
    let allowed = person.is_some_and(|owner| {
        same_owner(owner, &destination.identity)
            && !destination.identity.private
            && destination.conversation == destination.identity.conversation_key()
            && destination.identity.is_valid()
    });
    if allowed {
        return Ok(());
    }
    Err(Error::ScheduleDestinationForbidden)
}
