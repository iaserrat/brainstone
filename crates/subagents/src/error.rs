use std::path::PathBuf;

/// Expected sub-agent failures, reported to the model as structured tool
/// errors. Each variant carries a stable code.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A definition file could not be read.
    Io {
        /// The file that failed.
        path: PathBuf,
        /// The I/O error.
        message: String,
    },
    /// The frontmatter is not valid YAML.
    InvalidFrontmatter {
        /// The definition file.
        path: PathBuf,
        /// The parser's message.
        message: String,
    },
    /// A frontmatter field holds a value this loader rejects.
    InvalidDefinition {
        /// The definition file.
        path: PathBuf,
        /// The field's name.
        field: &'static str,
        /// Why the value was rejected.
        message: String,
    },
    /// The frontmatter names a field this loader does not support.
    UnsupportedField {
        /// The definition file.
        path: PathBuf,
        /// The unknown field.
        field: &'static str,
    },
    /// Two definitions declare the same name.
    DuplicateName {
        /// The duplicated name.
        name: String,
    },
    /// The tool arguments did not match the schema the model was shown.
    InvalidArguments {
        /// The parser's message, for the model to correct its call.
        message: String,
    },
    /// The requested profile cannot be applied to a child.
    InvalidProfile {
        /// The reason it cannot.
        message: String,
    },
    /// A sub-agent tool was called outside a durable operation.
    DurableContextRequired,
    /// No definition loaded under that name.
    UnknownAgent {
        /// The requested agent name.
        name: String,
    },
    /// Policy does not permit delegating to this agent.
    AgentDenied {
        /// The refused agent.
        name: String,
    },
    /// The delegation chain reached its depth cap.
    DepthExceeded,
    /// The child session exists but its definition no longer loads.
    AgentLost,
    /// The child's request or result could not be encoded.
    Encode {
        /// The encoder's message.
        message: String,
    },
    /// The store failed; carries the runtime's own code.
    Runtime(runtime::Error),
    /// The definition names an MCP server that is not configured.
    UnknownMcpServer {
        /// The server name.
        name: String,
    },
    /// The definition's tool selection leaves nothing callable.
    NoTools {
        /// The agent whose tool set is empty.
        name: String,
    },
    /// The definition names a tool that is not offered.
    UnknownTool {
        /// The agent.
        agent: String,
        /// The tool name.
        tool: String,
    },
}
