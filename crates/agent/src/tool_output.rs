use crate::{File, Image, ToolCall, ToolResult};

/// What one successful call hands back: text for the model, plus any images
/// it fetched, replayed inside the same tool result, and any files, which only
/// the reply can attach.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolOutput {
    /// Text the model reads.
    pub text: String,
    /// Images replayed inside this tool result.
    pub images: Vec<Image>,
    /// Files the reply can attach.
    pub files: Vec<File>,
}

impl ToolOutput {
    /// The result that answers `call` with this output, and the files that
    /// stay out of it.
    pub(crate) fn answer(self, call: &ToolCall, is_error: bool) -> (ToolResult, Vec<File>) {
        let result = ToolResult {
            id: call.id.clone(),
            name: call.name.clone(),
            content: self.text,
            is_error,
            images: self.images,
        };
        (result, self.files)
    }
}

impl From<String> for ToolOutput {
    fn from(text: String) -> Self {
        Self {
            text,
            ..Self::default()
        }
    }
}
