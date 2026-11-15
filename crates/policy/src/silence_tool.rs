use std::pin::Pin;

use agent::{SILENCE_TOOL, Tool, ToolError, ToolOutput, ToolRequest};
use serde_json::{Value, json};

/// What the model reads back on its next turn. The loop ends the run on the
/// call itself, so this text exists only to make the decision legible in the
/// transcript that gets replayed.
const ACKNOWLEDGED: &str = "No reply was delivered for that turn.";

/// Ends a turn without replying.
///
/// Stateless on purpose: the decision is the call, and `agent::next` acts on
/// it. This only has to advertise the tool and answer it, because a provider
/// rejects a batch holding a call no result answers.
#[derive(Debug)]
pub struct Silence;

impl Tool for Silence {
    fn name(&self) -> &'static str {
        SILENCE_TOOL
    }

    fn definition(&self) -> Value {
        json!({
            "name": self.name(),
            "description": "End this turn without replying, when nothing you could say \
                belongs in the conversation: the message was not addressed to you, someone \
                else has already answered, the decision is not yours, or your whole reply \
                would be an acknowledgement. Calling this ends the turn and delivers \
                nothing, including anything you have already written. Never write a \
                stand-in such as \"No response\" instead — that is a reply.",
            "input_schema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        })
    }

    fn call<'a>(
        &'a self,
        _request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async { Ok(ACKNOWLEDGED.to_owned().into()) })
    }
}
