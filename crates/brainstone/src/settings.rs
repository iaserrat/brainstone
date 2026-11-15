use std::collections::HashMap;
use std::path::Path;

use crate::{ConfigError, CoreSettings};

/// Operator settings parsed from the environment, plus the agent directory.
pub struct Settings {
    pub(crate) core: CoreSettings,
    pub(crate) file_tool: bool,
}

impl Settings {
    /// # Errors
    /// Reads common configuration and optional tool switches from `values`.
    pub fn from_map(values: &HashMap<String, String>, root: &Path) -> Result<Self, ConfigError> {
        Ok(Self {
            core: CoreSettings::from_map(values, root)?,
            file_tool: crate::settings_parse::flag(values, "FILE_TOOL")?,
        })
    }

    /// # Errors
    /// Returns `invalid_configuration` for invalid environment or files.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_map(&std::env::vars().collect(), Path::new("."))
    }
}
