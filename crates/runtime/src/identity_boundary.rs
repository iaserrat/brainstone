use agent::ExecutionIdentity;

use crate::Error;

pub(crate) fn check(
    stored: Option<&ExecutionIdentity>,
    incoming: Option<&ExecutionIdentity>,
) -> Result<(), Error> {
    let same = match (stored, incoming) {
        (None, None) => true,
        (Some(a), Some(b)) => compatible(a, b),
        _ => false,
    };
    if !same {
        return Err(Error::ExecutionIdentityConflict);
    }
    Ok(())
}

fn compatible(a: &ExecutionIdentity, b: &ExecutionIdentity) -> bool {
    let same_conversation = a.provider == b.provider
        && a.tenant == b.tenant
        && a.conversation == b.conversation
        && a.private == b.private;
    let same_owner = !a.private || a.actor == b.actor;
    same_conversation && same_owner && a.is_valid() && b.is_valid()
}
