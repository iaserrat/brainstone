use super::{ToolContract, ToolContractError};

const MAX_NAME_BYTES: usize = 64;

pub(super) fn names(
    values: impl IntoIterator<Item = impl Into<String>>,
) -> Result<Vec<String>, ToolContractError> {
    let mut names: Vec<String> = values.into_iter().map(Into::into).collect();
    if names.iter().any(|name| !valid(name)) {
        return Err(ToolContractError::InvalidName);
    }
    names.sort_unstable();
    names.dedup();
    Ok(names)
}

fn valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

impl ToolContract {
    pub(super) fn validate(&self) -> Result<(), ToolContractError> {
        if self
            .require_approval
            .iter()
            .chain(&self.require_success)
            .any(|name| !self.allows(name))
        {
            return Err(ToolContractError::ToolNotAllowed);
        }
        Ok(())
    }
}
