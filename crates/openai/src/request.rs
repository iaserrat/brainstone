use agent::Thread;
use serde::Serialize;
use serde_json::Value;

use crate::failure::Failure;
use crate::history::input;
use crate::tool_def::tools;
use crate::{Effort, ResponsesModel};

const ENCRYPTED_REASONING: &str = "reasoning.encrypted_content";
const SUMMARY_AUTO: &str = "auto";

#[derive(Serialize)]
pub(crate) struct Request<'a> {
    model: &'a str,
    instructions: &'a str,
    input: Vec<Value>,
    tools: Vec<Value>,
    store: bool,
    /// With `store: false`, reasoning replays only as encrypted content.
    #[serde(skip_serializing_if = "Option::is_none")]
    include: Option<[&'static str; 1]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<Reasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "is_false")]
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<Value>,
}

#[derive(Serialize)]
struct Reasoning {
    effort: Effort,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<&'static str>,
}

impl<'a> Request<'a> {
    /// The thread's execution profile overrides the model's own model and effort.
    pub(crate) fn new(
        model: &'a ResponsesModel,
        thread: &'a Thread,
        definitions: &[Value],
    ) -> Result<Self, Failure> {
        let effort = thread.profile().effort().map(Into::into).or(model.effort);
        Ok(Self {
            model: thread.profile().model().unwrap_or(&model.model),
            instructions: &thread.system,
            input: input(thread)?,
            tools: tools(definitions)?,
            store: false,
            include: effort.map(|_| [ENCRYPTED_REASONING]),
            reasoning: effort.map(|effort| Reasoning::new(effort, model.reasoning_summary)),
            max_output_tokens: model.max_output_tokens,
            stream: false,
            text: thread
                .profile()
                .output_schema()
                .map(crate::output_format::responses),
        })
    }

    pub(crate) fn streaming(mut self) -> Self {
        self.stream = true;
        self
    }
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde callback receives a reference"
)]
const fn is_false(value: &bool) -> bool {
    !*value
}

impl Reasoning {
    fn new(effort: Effort, summary: bool) -> Self {
        Self {
            effort,
            summary: summary.then_some(SUMMARY_AUTO),
        }
    }
}
