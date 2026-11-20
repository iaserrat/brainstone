//! Table-driven Messages wire cases preserve schema and effort independently.
#![allow(clippy::too_many_lines)]

use agent::{ExecutionProfile, OutputSchema, Thread};
use serde_json::{Value, json};

use crate::{AnthropicModel, Auth, Effort, Endpoint};

fn schema() -> Value {
    json!({"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":false})
}

fn model() -> AnthropicModel {
    let endpoint = Endpoint::messages("http://localhost/messages").unwrap();
    AnthropicModel::new(endpoint, "model", Auth::ApiKey("secret".into()), 1024).unwrap()
}

#[test]
fn messages_schema_coexists_with_effort_and_tools() {
    let model = model();
    let tools = vec![
        json!({"name":"lookup","description":"Read a value","input_schema":{"type":"object"}}),
    ];
    for effort in [None, Some(Effort::High)] {
        let profile = ExecutionProfile::new(None, None, 4, 0)
            .unwrap()
            .with_output_schema(OutputSchema::new("answer", schema()).unwrap());
        let thread = Thread::new_with_profile("system", "prompt", profile);
        let request = crate::request::Request::new(&model, "model", effort, &thread, &tools)
            .ok()
            .unwrap();
        let body = serde_json::to_value(request).unwrap();
        assert_eq!(
            body["output_config"]["format"],
            json!({"type":"json_schema","schema":schema()})
        );
        assert_eq!(
            body["output_config"].get("effort").and_then(Value::as_str),
            effort.map(|_| "high")
        );
        assert_eq!(body["tools"].as_array().unwrap().len(), 1);
        assert!(body.get("tool_choice").is_none());
        assert_eq!(body["stream"], true);
    }
}

#[test]
fn messages_without_schema_keep_effort_optional() {
    let model = model();
    let thread = Thread::new("system", "prompt");
    for effort in [None, Some(Effort::High)] {
        let body = serde_json::to_value(
            crate::request::Request::new(&model, "model", effort, &thread, &[])
                .ok()
                .unwrap(),
        )
        .unwrap();
        if effort.is_some() {
            assert_eq!(body["output_config"], json!({"effort":"high"}));
        } else {
            assert!(body.get("output_config").is_none());
        }
    }
}
