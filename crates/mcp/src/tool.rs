use std::{borrow::Cow, pin::Pin};

use agent::{Tool, ToolError, ToolOutput, ToolRequest};
use rmcp::model::Tool as McpTool;
use serde_json::{Value, json};

use crate::{error::Error, session::Session};

pub(crate) struct RemoteTool {
    pub(crate) name: String,
    pub(crate) server: String,
    pub(crate) remote_name: Cow<'static, str>,
    pub(crate) definition: Value,
    pub(crate) session: Session,
}

impl RemoteTool {
    pub(crate) fn new(server: &str, tool: McpTool, session: Session) -> Result<Self, Error> {
        let name = format!("mcp__{server}__{}", tool.name);
        validate_name(&name)?;
        let definition = definition(&name, &tool);
        Ok(Self {
            name,
            server: server.to_owned(),
            remote_name: tool.name,
            definition,
            session,
        })
    }
}

impl Tool for RemoteTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn definition(&self) -> Value {
        self.definition.clone()
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(crate::tool_call::call(self, request))
    }
}

fn definition(name: &str, tool: &McpTool) -> Value {
    json!({
        "name": name,
        "description": tool.description,
        "input_schema": tool.input_schema,
    })
}

fn validate_name(name: &str) -> Result<(), Error> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
    valid
        .then_some(())
        .ok_or_else(|| Error::new("mcp_invalid_tool_name", name))
}
