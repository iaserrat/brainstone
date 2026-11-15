//! The executable that picks a prompt's model and reasoning effort.
use std::time::Duration;

use agent::{Effort, ExecutionProfile};
use runtime::Prompt;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::Error;

/// The selector may consult a classifier; past this the prompt is rejected.
const SELECTION_TIMEOUT: Duration = Duration::from_secs(10);

/// The trusted executable that picks a model and effort for each prompt. The
/// deployment owns whether the model exists and supports the effort level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelSelector {
    program: String,
    args: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    model: String,
    effort: Effort,
}

impl ModelSelector {
    #[must_use]
    /// Runs `program` with `args` to choose a model and effort per prompt.
    pub const fn new(program: String, args: Vec<String>) -> Self {
        Self { program, args }
    }

    /// The prompt's profile with the selected model and effort; every other
    /// field, such as the turn limit, is kept.
    pub(crate) async fn select(&self, prompt: &Prompt) -> Result<ExecutionProfile, Error> {
        let (output, _child) = crate::stdio_json::exchange(
            &self.program,
            &self.args,
            &input(prompt),
            SELECTION_TIMEOUT,
        )
        .await
        .map_err(|_| Error::ModelSelectionFailed)?;
        let selection: Selection =
            serde_json::from_value(output).map_err(|_| Error::ModelSelectionFailed)?;
        let current = prompt.profile();
        ExecutionProfile::new(
            Some(&selection.model),
            Some(selection.effort),
            current.max_turns(),
            current.depth(),
        )
        .map(|profile| {
            profile.with_allowed_agents(current.allowed_agents().map(<[String]>::to_vec))
        })
        .map_err(|_| Error::ModelSelectionFailed)
    }
}

/// The prompt facts the selector reads.
fn input(prompt: &Prompt) -> Value {
    let address = prompt.address();
    json!({
        "surface": address.surface(),
        "conversation": address.conversation(),
        "lane": address.lane(),
        "actor": prompt.actor(),
        "text": prompt.text(),
    })
}
