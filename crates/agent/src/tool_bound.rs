//! The size one tool result may take in the thread.

/// Every later request replays a result, so one result past this would let a
/// single call outgrow the context window on its own. About 25k to 35k tokens
/// of code or JSON.
pub const MAX_TOOL_RESULT_BYTES: usize = 100 * 1024;

/// Named in the marker that replaces the omitted middle of an oversized result.
pub const CODE_TOOL_RESULT_TRUNCATED: &str = "tool_result_truncated";

/// `text` itself when it fits; otherwise its head and tail, each half the cap,
/// cut on character boundaries around one marker line stating exactly how many
/// bytes were omitted, so the model knows the result is partial.
pub(crate) fn bounded(text: String) -> String {
    if text.len() <= MAX_TOOL_RESULT_BYTES {
        return text;
    }
    let head = text.floor_char_boundary(MAX_TOOL_RESULT_BYTES / 2);
    let tail = text.ceil_char_boundary(text.len() - MAX_TOOL_RESULT_BYTES / 2);
    format!(
        "{}\n[{CODE_TOOL_RESULT_TRUNCATED}: {} of {} bytes omitted from the middle of this result]\n{}",
        &text[..head],
        tail - head,
        text.len(),
        &text[tail..]
    )
}

impl crate::ToolResult {
    /// This result with its text held to the bound, for replaying a result
    /// recorded before results were bounded. An error document cut this way is
    /// no longer valid JSON; only a reader that needs no structure should ask.
    #[must_use]
    pub fn bounded(self) -> Self {
        Self {
            content: bounded(self.content),
            ..self
        }
    }
}
