use std::fmt;

use crate::ResponsesModel;

impl fmt::Debug for ResponsesModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponsesModel")
            .field("endpoint", &self.http.redacted_endpoint().as_str())
            .field("model", &self.model)
            .field("credentials", &"[REDACTED]")
            .field("effort", &self.effort)
            .field("max_output_tokens", &self.max_output_tokens)
            .field("reasoning_summary", &self.reasoning_summary)
            .finish_non_exhaustive()
    }
}
