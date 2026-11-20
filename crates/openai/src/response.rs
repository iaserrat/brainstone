use agent::{ModelResponse, StopReason};
use serde::Deserialize;
use serde_json::Value;

use crate::failure::{
    CODE_CONTENT_FILTERED, CODE_INVALID_RESPONSE, CODE_REQUEST_CANCELLED, Failure,
};
use crate::output::parse;

#[derive(Deserialize)]
pub(crate) struct Response {
    status: Status,
    output: Vec<Value>,
    incomplete_details: Option<IncompleteDetails>,
    error: Option<crate::response_error::ApiError>,
}

#[derive(Deserialize)]
struct IncompleteDetails {
    reason: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Completed,
    Incomplete,
    Failed,
    Cancelled,
    Queued,
    InProgress,
}

impl Response {
    pub(crate) fn into_model(self) -> Result<ModelResponse, Failure> {
        let truncated = self.is_truncated();
        let output = parse(&self.output, truncated)?;
        let stop = self.stop(output.tool_calls.is_empty())?;
        Ok(ModelResponse {
            model: None,
            reasoning: output.reasoning,
            text: output.text,
            tool_calls: output.tool_calls,
            stop,
        })
    }

    fn is_truncated(&self) -> bool {
        matches!(self.status, Status::Incomplete)
            && self
                .incomplete_details
                .as_ref()
                .is_some_and(|details| details.reason == "max_output_tokens")
    }

    fn stop(&self, no_calls: bool) -> Result<StopReason, Failure> {
        match self.status {
            Status::Completed if no_calls => Ok(StopReason::EndTurn),
            Status::Completed => Ok(StopReason::ToolUse),
            Status::Incomplete if self.is_truncated() => Ok(StopReason::MaxTokens),
            Status::Incomplete if self.is_filtered() => Err(Failure::new(
                CODE_CONTENT_FILTERED,
                "the provider filtered an incomplete response",
            )),
            Status::Incomplete => Err(Failure::new(
                CODE_INVALID_RESPONSE,
                "the provider returned an unknown incomplete reason",
            )),
            Status::Failed => Err(crate::response_error::failure(self.error.as_ref())),
            Status::Cancelled => Err(Failure::new(
                CODE_REQUEST_CANCELLED,
                "the provider cancelled the response",
            )),
            Status::Queued | Status::InProgress => Err(Failure::new(
                CODE_INVALID_RESPONSE,
                "the synchronous provider returned a non-terminal response",
            )),
        }
    }

    fn is_filtered(&self) -> bool {
        self.incomplete_details
            .as_ref()
            .is_some_and(|details| details.reason == "content_filter")
    }
}
