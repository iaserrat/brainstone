use crate::Error;

impl Error {
    #[must_use]
    /// The stable code a caller branches on.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidArguments { .. } => "invalid_skill_arguments",
            Self::UnknownSkill { .. } => "unknown_skill",
            Self::ResourceIo { .. } => "skill_resource_io",
            Self::InvalidResourcePath { .. } => "invalid_skill_resource_path",
            Self::ResourceNotText { .. } => "skill_resource_not_text",
            Self::ResourceTooLarge { .. } => "skill_resource_too_large",
        }
    }
}
