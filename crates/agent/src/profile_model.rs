use super::ExecutionProfile;
use crate::ProfileError;

impl ExecutionProfile {
    /// Select a model while retaining every other accepted execution setting.
    ///
    /// # Errors
    /// Returns `empty_model` when `model` is empty, matching [`Self::new`].
    pub fn with_model(mut self, model: impl Into<String>) -> Result<Self, ProfileError> {
        let model = model.into();
        if model.is_empty() {
            return Err(ProfileError::EmptyModel);
        }
        self.model = Some(model);
        Ok(self)
    }
}
