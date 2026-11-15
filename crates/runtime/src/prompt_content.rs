use agent::Image;

/// Everything a user turn carries. A prompt is valid when any part is
/// non-empty, so the union is validated once at construction instead of in
/// one constructor per content kind.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptContent {
    /// The prompt's text.
    pub text: String,
    /// Images sent with it.
    pub images: Vec<Image>,
}

impl PromptContent {
    #[must_use]
    /// Whether neither part carries anything.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.images.is_empty()
    }
}
