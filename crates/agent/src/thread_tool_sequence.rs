use super::Thread;

impl Thread {
    /// Restore the number of assistant responses durably appended to this operation.
    ///
    /// Store implementations must count the current operation's complete journal,
    /// including responses replaced by compaction. Lane history and user-turn
    /// budgets are not this counter. Set it to zero for an unstarted operation.
    #[must_use]
    pub const fn with_tool_sequence(mut self, sequence: usize) -> Self {
        self.tool_sequence = Some(sequence);
        self
    }

    /// The step component for durable effect identity, stable across steering.
    ///
    /// Returns `None` for a new-profile thread restored without its authoritative
    /// operation counter. A durable driver must reject it rather than invent a key.
    /// Profiles without a key version retain the historical key calculation.
    #[must_use]
    pub fn tool_key_step(&self) -> Option<usize> {
        if self.profile.operation_tool_keys() {
            self.tool_sequence
        } else {
            Some(self.tool_step())
        }
    }
}
