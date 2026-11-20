//! Typed tool contracts reject invalid input before application code runs.
#![allow(clippy::too_many_lines)] // Cases describe the boundary as a whole.

use brainstone_agent::{
    ExecutionIdentity, ExecutionProfile, Tool, ToolContext, ToolError, ToolRequest, typed_tool,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Arguments {
    text: String,
    mode: Mode,
    #[serde(default)]
    #[schemars(range(min = 0, max = 3))]
    count: u8,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Read,
    Write,
}

fn request(args: serde_json::Value) -> ToolRequest {
    ToolRequest {
        args,
        idempotency_key: None,
        context: None,
    }
}

#[tokio::test]
async fn typed_arguments_accept_unicode_enums_and_serde_defaults() {
    let tool = typed_tool(
        "Echo",
        "Echo typed input.",
        |args: Arguments, _| async move {
            assert!(matches!(args.mode, Mode::Read));
            Ok::<_, ToolError>(json!({"text": args.text, "count": args.count}))
        },
    )
    .unwrap();
    let output = tool
        .call(request(json!({"text":"reçu ☕", "mode":"read"})))
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&output.text).unwrap(),
        json!({"outcome":"success", "value":{"text":"reçu ☕", "count":0}})
    );
    assert_eq!(
        tool.definition()["input_schema"]["properties"]["mode"]["$ref"],
        "#/$defs/Mode"
    );
}

#[tokio::test]
async fn invalid_arguments_never_reach_the_handler() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let tool = typed_tool("Echo", "Echo typed input.", move |_: Arguments, _| {
        observed.fetch_add(1, Ordering::SeqCst);
        async { Ok::<_, ToolError>(()) }
    })
    .unwrap();
    for args in [
        json!({}),
        json!({"text":"x","mode":"other"}),
        json!({"text":"x","mode":"read","count":256}),
        json!({"text":"x","mode":"read","count":4}),
        json!({"text":"x","mode":"read","extra":true}),
    ] {
        assert_eq!(
            tool.call(request(args)).await.unwrap_err().code,
            "invalid_tool_arguments"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[derive(Debug)]
struct DomainError;
impl From<DomainError> for ToolError {
    fn from(_: DomainError) -> Self {
        Self::new("not_refundable", "Order cannot be refunded")
    }
}

#[tokio::test]
async fn domain_errors_keep_their_stable_code() {
    let tool = typed_tool("Refund", "Refund an order.", |_: Arguments, _| async {
        Err::<(), _>(DomainError)
    })
    .unwrap();
    let error = tool
        .call(request(json!({"text":"x","mode":"write"})))
        .await
        .unwrap_err();
    assert_eq!(error.code, "not_refundable");
}

struct BrokenOutput;
impl Serialize for BrokenOutput {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("cannot encode"))
    }
}

#[tokio::test]
async fn output_encoding_failure_has_a_stable_code() {
    let tool = typed_tool(
        "Broken",
        "Produce invalid output.",
        |_: Arguments, _| async { Ok::<_, ToolError>(BrokenOutput) },
    )
    .unwrap();
    let error = tool
        .call(request(json!({"text":"x","mode":"read"})))
        .await
        .unwrap_err();
    assert_eq!(error.code, "tool_output_invalid");
}

#[tokio::test]
async fn invocation_preserves_durable_key_and_context() {
    let context = ToolContext {
        operation_id: 42,
        identity: Some(ExecutionIdentity {
            provider: "test".into(),
            tenant: "shop".into(),
            actor: "customer".into(),
            conversation: "order-42".into(),
            private: true,
        }),
        profile: ExecutionProfile::default(),
    };
    let expected = context.clone();
    let tool = typed_tool(
        "Inspect",
        "Inspect invocation.",
        move |_: Arguments, invocation| {
            assert_eq!(
                invocation.idempotency_key(),
                Some("operation/42/step/1/call/1")
            );
            assert_eq!(
                invocation.require_idempotency_key().unwrap(),
                "operation/42/step/1/call/1"
            );
            assert_eq!(invocation.context(), Some(&expected));
            async { Ok::<_, ToolError>(()) }
        },
    )
    .unwrap();
    let mut input = request(json!({"text":"x","mode":"read"}));
    input.idempotency_key = Some("operation/42/step/1/call/1".into());
    input.context = Some(context);
    tool.call(input).await.unwrap();
}

#[tokio::test]
async fn requiring_durable_identity_fails_without_a_key() {
    let tool = typed_tool(
        "Write",
        "Write once.",
        |_: Arguments, invocation| async move {
            invocation.require_idempotency_key()?;
            Ok::<_, ToolError>(())
        },
    )
    .unwrap();
    let error = tool
        .call(request(json!({"text":"x","mode":"write"})))
        .await
        .unwrap_err();
    assert_eq!(error.code, "tool_durability_required");
}

#[test]
fn construction_rejects_invalid_names() {
    let tool = typed_tool("invalid name", "Invalid.", |_: Arguments, _| async {
        Ok::<_, ToolError>(())
    });
    assert_eq!(tool.unwrap_err().code, "invalid_tool_definition");
}

#[tokio::test]
async fn oversized_json_fails_instead_of_becoming_truncated_json() {
    let tool = typed_tool(
        "Echo",
        "Echo typed input.",
        |args: Arguments, _| async move { Ok::<_, ToolError>(args.text) },
    )
    .unwrap();
    let text = "☕".repeat(brainstone_agent::MAX_TOOL_RESULT_BYTES / 3);
    let error = tool
        .call(request(json!({"text":text,"mode":"read"})))
        .await
        .unwrap_err();
    assert_eq!(error.code, "tool_output_invalid");
}

#[test]
fn construction_rejects_blank_descriptions() {
    for description in ["", " \t\n", "\u{2003}"] {
        let tool = typed_tool("Echo", description, |_: Arguments, _| async {
            Ok::<_, ToolError>(())
        });
        assert_eq!(tool.unwrap_err().code, "invalid_tool_definition");
    }
}

#[test]
fn construction_rejects_non_object_arguments() {
    let tool = typed_tool("Primitive", "Primitive input", |_: String, _| async {
        Ok::<_, ToolError>(())
    });
    assert_eq!(tool.unwrap_err().code, "invalid_tool_definition");
    let tool = typed_tool("RootEnum", "Enum input", |_: Mode, _| async {
        Ok::<_, ToolError>(())
    });
    assert_eq!(tool.unwrap_err().code, "invalid_tool_definition");
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RecursiveArgs {
    value: u8,
    #[serde(default)]
    child: Option<Box<RecursiveArgs>>,
}

async fn recursive_handler(
    args: RecursiveArgs,
    _: brainstone_agent::ToolInvocation,
) -> Result<u16, DomainError> {
    Ok(u16::from(args.value) + args.child.map_or(0, |child| u16::from(child.value)))
}

#[tokio::test]
async fn ordinary_async_functions_infer_types_and_validate_recursive_arguments() {
    let tool = typed_tool("Tree", "Read nested values", recursive_handler).unwrap();
    let output = tool
        .call(request(json!({"value":2,"child":{"value":3}})))
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&output.text).unwrap()["value"],
        5
    );
    let error = tool
        .call(request(json!({"value":2,"child":{"value":300}})))
        .await
        .unwrap_err();
    assert_eq!(error.code, "invalid_tool_arguments");
}
