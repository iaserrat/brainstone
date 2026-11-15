use serde::{Deserialize, Serialize};

use crate::{Error, OperationId, SubagentConfig};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// One request to spawn a child run.
pub struct SubagentRequest {
    pub(crate) parent: OperationId,
    pub(crate) key: String,
    pub(crate) task: String,
    pub(crate) config: SubagentConfig,
}

impl SubagentRequest {
    /// # Errors
    /// Returns `invalid_input` when the retry key or task is empty.
    pub fn new(
        parent: OperationId,
        key: &str,
        task: &str,
        config: SubagentConfig,
    ) -> Result<Self, Error> {
        if key.is_empty() {
            return Err(Error::InvalidInput {
                field: "subagent_key",
            });
        }
        if task.trim().is_empty() {
            return Err(Error::InvalidInput {
                field: "subagent_task",
            });
        }
        Ok(Self {
            parent,
            key: key.to_owned(),
            task: task.to_owned(),
            config,
        })
    }
}
