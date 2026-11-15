use agent::ToolContext;

use crate::Error;

pub(crate) fn check(context: &ToolContext, name: &str, max_depth: u8) -> Result<(), Error> {
    if context.profile.depth() >= max_depth {
        return Err(Error::DepthExceeded);
    }
    if context
        .profile
        .allowed_agents()
        .is_some_and(|items| !items.iter().any(|item| item == name))
    {
        return Err(Error::AgentDenied {
            name: name.to_owned(),
        });
    }
    Ok(())
}
