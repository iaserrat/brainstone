#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
// Scenario assertions keep propagation and privacy together.
#![allow(clippy::too_many_lines)]
use opentelemetry::trace::TracerProvider as _;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
    },
    service::{RequestContext, RoleServer},
};
use serde_json::{Map, json};
use std::sync::{Arc, Mutex};
use tracing_subscriber::prelude::*;
#[derive(Clone)]
struct Server;
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        Ok(ListToolsResult::with_all_items(vec![Tool::new(
            "echo",
            "Echo",
            Map::new(),
        )]))
    }
    async fn call_tool(
        &self,
        _: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        let traceparent = context
            .meta
            .0
            .0
            .get("traceparent")
            .and_then(serde_json::Value::as_str);
        if traceparent.is_none_or(|value| value.len() != 55) {
            return Err(rmcp::ErrorData::invalid_params(
                "missing W3C traceparent",
                None,
            ));
        }
        Ok(CallToolResult::success(vec![ContentBlock::text("SENSITIVE-RESULT")]).into())
    }
}

#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for LogBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn operational_logs_identify_calls_without_logging_arguments_or_identity() {
    let buffer = LogBuffer::default();
    let sink = buffer.clone();
    let exporter = opentelemetry_sdk::trace::InMemorySpanExporter::default();
    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(provider.tracer("test"))
                .with_filter(tracing_subscriber::filter::filter_fn(|meta| {
                    meta.is_span() && meta.target() == "brainstone::telemetry"
                })),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(move || sink.clone())
                .with_filter(tracing_subscriber::filter::LevelFilter::INFO),
        );
    let _scope = tracing::subscriber::set_default(subscriber);
    let (server_io, client_io) = tokio::io::duplex(4096);
    tokio::spawn(async move {
        Server.serve(server_io).await.unwrap().waiting().await.ok();
    });
    let mut client = brainstone_mcp::Client::connect_stream("local", client_io)
        .await
        .unwrap();
    let tools = client.tools().await.unwrap();
    let tool = &tools[0];
    let mut request = agent::ToolRequest {
        args: json!({}),
        idempotency_key: None,
        context: Some(agent::ToolContext {
            operation_id: 42,
            identity: None,
            profile: agent::ExecutionProfile::default(),
        }),
    };
    request.args = json!({"sql":"SENSITIVE-SQL", "password":"SENSITIVE-PASSWORD"});
    request.idempotency_key = Some("SENSITIVE-KEY".into());
    tool.call(request).await.unwrap();
    client.close().await.unwrap();
    let spans = exporter.get_finished_spans().unwrap();
    for kind in ["mcp.call", "mcp.queue"] {
        assert!(
            spans.iter().any(|span| span
                .attributes
                .iter()
                .any(|attr| attr.key.as_str() == "kind" && attr.value.as_str() == kind)),
            "missing {kind}"
        );
    }
    let log = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
    for field in [
        "server",
        "tool",
        "operation_id",
        "request_id",
        "queue_ms",
        "execution_ms",
        "elapsed_ms",
        "MCP call finished",
    ] {
        assert!(log.contains(field), "missing {field}: {log}");
    }
    assert!(
        !log.contains("SENSITIVE-"),
        "argument or identity leaked into operational logs"
    );
}
