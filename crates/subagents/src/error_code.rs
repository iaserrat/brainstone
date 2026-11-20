use crate::Error;

impl Error {
    /// The stable code a caller branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "agent_definition_io",
            Self::InvalidFrontmatter { .. } => "invalid_agent_frontmatter",
            Self::InvalidDefinition { .. } => "invalid_agent_definition",
            Self::UnsupportedField { .. } => "unsupported_agent_field",
            Self::DuplicateName { .. } => "duplicate_agent_name",
            Self::InvalidArguments { .. } => "invalid_agent_arguments",
            Self::InvalidProfile { .. } => "invalid_agent_profile",
            Self::DurableContextRequired => "durable_context_required",
            Self::UnknownAgent { .. } => "unknown_agent",
            Self::AgentDenied { .. } => "agent_denied",
            Self::DepthExceeded => "agent_depth_exceeded",
            Self::AgentLost => "agent_lost",
            Self::Encode { .. } => "agent_result_encode",
            Self::Runtime(error) => error.code(),
            Self::UnknownMcpServer { .. } => "unknown_agent_mcp_server",
            Self::NoTools { .. } => "agent_has_no_tools",
            Self::UnknownTool { .. } => "unknown_agent_tool",
        }
    }
}
