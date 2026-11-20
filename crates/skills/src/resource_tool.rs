use std::{pin::Pin, sync::Arc};

use agent::{Tool, ToolError, ToolOutput, ToolRequest};
use serde_json::{Value, json};

use crate::Catalog;

pub(crate) struct ResourceTool(pub(crate) Arc<Catalog>);

impl Tool for ResourceTool {
    fn name(&self) -> &'static str {
        "SkillResource"
    }

    fn definition(&self) -> Value {
        let names: Vec<&str> = self.0.iter().map(crate::Skill::name).collect();
        json!({
            "name":"SkillResource",
            "description":"Read one text resource from an activated skill only when its instructions require that file.",
            "input_schema":{"type":"object","additionalProperties":false,"required":["name","path"],"properties":{"name":{"type":"string","enum":names},"path":{"type":"string"}}}
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            self.read(request.args)
                .map_err(|error| ToolError::new(error.code(), error.to_string()))
                .map(ToolOutput::from)
        })
    }
}
