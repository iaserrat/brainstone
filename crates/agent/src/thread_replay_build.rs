//! The owned replay, built only when a thread holds something to change.

use super::{failed, own, unanswered};
use crate::{Message, ModelResponse, ToolCall};

#[derive(Default)]
pub(super) struct Replay<'a> {
    pub(super) messages: Vec<Message>,
    pending: &'a [ToolCall],
}

impl<'a> Replay<'a> {
    pub(super) fn push(&mut self, message: &'a Message, producer: &str) {
        match message {
            Message::Assistant(response) if failed(response) => {}
            Message::Assistant(response) => {
                self.close();
                self.pending = &response.tool_calls;
                self.messages
                    .push(Message::Assistant(shown(response, producer)));
            }
            Message::ToolResults(results) => {
                let mut results = results.clone();
                results.extend(unanswered(self.pending, &results));
                self.pending = &[];
                self.messages.push(Message::ToolResults(results));
            }
            other => {
                self.close();
                self.messages.push(other.clone());
            }
        }
    }

    /// Answer the calls the last response left open, before anything follows.
    pub(super) fn close(&mut self) {
        if !self.pending.is_empty() {
            let results = unanswered(self.pending, &[]);
            self.messages.push(Message::ToolResults(results));
            self.pending = &[];
        }
    }
}

fn shown(response: &ModelResponse, producer: &str) -> ModelResponse {
    let mut response = response.clone();
    if !own(&response, producer) {
        response.reasoning.clear();
    }
    response
}
