use super::Thread;

impl Thread {
    /// Restore successful tool names from the current operation's complete journal.
    ///
    /// Count only durably appended tool results with `is_error == false`, including
    /// results replaced by compaction. Earlier operations and progress events do
    /// not qualify. Custom stores must supply even an empty list for contracts
    /// requiring success; absent evidence fails closed at terminal validation.
    #[must_use]
    pub fn with_tool_successes(mut self, mut names: Vec<String>) -> Self {
        names.sort_unstable();
        names.dedup();
        self.tool_successes = Some(names);
        self
    }

    pub(crate) fn tool_successes(&self) -> Option<&[String]> {
        self.tool_successes.as_deref()
    }

    pub(super) fn remember_successes(&mut self, message: &super::Message) {
        if let super::Message::ToolResults(results) = message
            && let Some(successes) = &mut self.tool_successes
        {
            successes.extend(
                results
                    .iter()
                    .filter(|result| !result.is_error)
                    .map(|result| result.name.clone()),
            );
            successes.sort_unstable();
            successes.dedup();
        }
    }
}
