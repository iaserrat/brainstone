use super::ExecutionProfile;
use crate::OutputSchema;

impl ExecutionProfile {
    /// Require the final answer to match this schema. The schema is persisted
    /// with the accepted profile, so a restart keeps the same output contract.
    #[must_use]
    pub fn with_output_schema(mut self, schema: OutputSchema) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// The accepted final-answer contract, when structured output is required.
    #[must_use]
    pub const fn output_schema(&self) -> Option<&OutputSchema> {
        self.output_schema.as_ref()
    }

    /// Remove the final-answer contract for an internal request such as compaction.
    #[must_use]
    pub fn without_output_schema(mut self) -> Self {
        self.output_schema = None;
        self
    }
}
