use super::Message;
use crate::{Image, UserImages};

impl Message {
    /// The one authority for which variant a user turn takes: images alone
    /// make [`Message::UserImages`], and bare text stays [`Message::User`].
    /// Every producer of user turns — acceptance, steering, history replay —
    /// must build through here so the selection can never drift between them.
    ///
    /// # Panics
    ///
    /// Never in practice: the constructor it delegates to rejects only the
    /// emptiness this function has already branched away.
    #[must_use]
    pub fn user(text: impl Into<String>, images: Vec<Image>) -> Self {
        if images.is_empty() {
            return Self::User(text.into());
        }
        Self::UserImages(UserImages::new(text, images).expect("images are non-empty"))
    }

    /// A message that starts a user turn: it resets the step budget and marks
    /// where steering may no longer be applied.
    #[must_use]
    pub const fn is_user_turn(&self) -> bool {
        matches!(self, Self::User(_) | Self::UserImages(_))
    }
}
