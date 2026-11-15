use agent::{ModelResponse, StopReason};
use serde_json::Value;

use crate::content_block::Block;
use crate::failure::Failure;
use crate::sse::invalid;
use crate::stream_state::State;

impl State {
    pub(crate) fn start(&mut self, index: usize, value: &Value) -> Result<(), Failure> {
        if self.blocks.insert(index, Block::start(value)?).is_some() {
            return Err(invalid("content block index was reused"));
        }
        Ok(())
    }

    pub(crate) fn delta(&mut self, index: usize, value: &Value) -> Result<(), Failure> {
        self.blocks
            .get_mut(&index)
            .ok_or_else(|| invalid("delta has no content block"))?
            .delta(value)
    }

    pub(crate) fn block_stop(&self, index: usize) -> Result<(), Failure> {
        self.blocks
            .contains_key(&index)
            .then_some(())
            .ok_or_else(|| invalid("unknown content block stopped"))
    }

    pub(crate) fn set_stop(&mut self, reason: &str) -> Result<(), Failure> {
        self.stop = Some(stop(reason)?);
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> Result<ModelResponse, Failure> {
        let stop = self
            .stop
            .take()
            .ok_or_else(|| invalid("message_stop arrived without a stop reason"))?;
        let truncated = stop == StopReason::MaxTokens;
        let output = crate::content_output::output(std::mem::take(&mut self.blocks), truncated)?;
        Ok(ModelResponse {
            model: None,
            reasoning: output.reasoning,
            text: output.text,
            tool_calls: output.calls,
            stop,
        })
    }
}

fn stop(reason: &str) -> Result<StopReason, Failure> {
    match reason {
        "end_turn" | "stop_sequence" => Ok(StopReason::EndTurn),
        "tool_use" => Ok(StopReason::ToolUse),
        "max_tokens" | "model_context_window_exceeded" => Ok(StopReason::MaxTokens),
        "refusal" => Ok(StopReason::Error {
            code: crate::REFUSED.to_owned(),
            message: "Anthropic refused the request".to_owned(),
        }),
        _ => Err(invalid(format!("unknown stop reason: {reason}"))),
    }
}
