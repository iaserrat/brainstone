use std::{pin::Pin, sync::Arc};

use agent::{Tool, ToolError, ToolOutput, ToolRequest};
use serde_json::{Value, json};

use crate::Catalog;

pub(crate) struct SkillTool(pub(crate) Arc<Catalog>);

impl Tool for SkillTool {
    fn name(&self) -> &'static str {
        "Skill"
    }

    fn definition(&self) -> Value {
        let skills: Vec<Value> = self
            .0
            .iter()
            .map(|skill| json!({"name":skill.name(),"description":skill.description()}))
            .collect();
        let names: Vec<&str> = self.0.iter().map(crate::Skill::name).collect();
        json!({
            "name":"Skill",
            "description": format!("Load complete instructions for one relevant skill before acting. Available skills: {}", serde_json::to_string(&skills).expect("skill metadata is JSON")),
            "input_schema":{"type":"object","additionalProperties":false,"required":["name"],"properties":{"name":{"type":"string","enum":names}}}
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            self.activate(request.args)
                .map_err(|error| ToolError::new(error.code(), error.to_string()))
                .map(ToolOutput::from)
        })
    }
}
