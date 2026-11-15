#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
#![allow(clippy::too_many_lines)]
// Table-style protocol assertions are intentionally kept in one test flow.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent::{Tool, ToolRequest};
use rmcp::{
    ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, CreateTaskResult,
        ElicitRequest, ElicitRequestParams, ElicitResult, ElicitationAction, GetTaskParams,
        GetTaskResult, InputRequest, InputRequests, InputRequiredResult, ListToolsResult,
        PaginatedRequestParams, ProtocolVersion, ResourceContents, ServerCapabilities,
        ServerConfig, Tool as McpTool,
    },
    service::{RequestContext, RoleServer},
    task_manager::{TaskManager, TaskOptions},
};
use serde_json::{Map, Value, json};

use crate::Client;
use crate::{Interaction, InteractionContext, InteractionError};

#[derive(Clone, Default)]
struct Server {
    seen_key: Arc<Mutex<Option<String>>>,
    seen_context: Arc<Mutex<Option<Value>>>,
    calls: Arc<Mutex<Vec<(Value, Value)>>>,
}

#[derive(Clone, Default)]
struct SlowServer;

#[derive(Clone)]
struct TaskServer(TaskManager);

impl Default for TaskServer {
    fn default() -> Self {
        Self(TaskManager::new())
    }
}

impl ServerHandler for TaskServer {
    fn get_info(&self) -> ServerConfig {
        let mut info = ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tasks()
                .build(),
        );
        info.protocol_version = ProtocolVersion::V_2026_07_28;
        info
    }

    async fn call_tool(
        &self,
        _request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        let task = self
            .0
            .spawn(TaskOptions::new().with_poll_interval_ms(1), |_context| {
                Box::pin(async {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    Ok(CallToolResult::success(vec![ContentBlock::text(
                        "from task",
                    )]))
                })
            });
        Ok(CreateTaskResult::new(task).into())
    }

    async fn get_task(
        &self,
        request: GetTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, rmcp::ErrorData> {
        Ok(GetTaskResult::new(self.0.get_task(&request.task_id)?))
    }
}

impl ServerHandler for SlowServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn call_tool(
        &self,
        _request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(CallToolResult::success(vec![]).into())
    }
}

#[derive(Clone, Default)]
struct MrtrServer;

impl ServerHandler for MrtrServer {
    fn get_info(&self) -> ServerConfig {
        let mut info = ServerConfig::new(ServerCapabilities::builder().enable_tools().build());
        info.protocol_version = ProtocolVersion::V_2026_07_28;
        info
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        if request.input_responses.is_some() {
            return Ok(CallToolResult::success(vec![ContentBlock::text("approved")]).into());
        }
        let params = ElicitRequestParams::FormElicitationParams {
            meta: None,
            message: "Proceed?".into(),
            requested_schema: serde_json::from_value(json!({
                "type": "object", "properties": {}
            }))
            .expect("elicitation schema"),
        };
        let mut requests = InputRequests::new();
        requests.insert(
            "approval".into(),
            InputRequest::Elicitation(ElicitRequest::new(params)),
        );
        Ok(InputRequiredResult::new(Some(requests), Some("state-1".into())).into())
    }
}

#[derive(Default)]
struct Approve(Mutex<Vec<InteractionContext>>);

impl Interaction for Approve {
    fn elicit<'a>(
        &'a self,
        context: InteractionContext,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<ElicitResult, InteractionError>> + Send + 'a>>
    {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            self.0.lock().unwrap().push(context);
            Ok(ElicitResult::new(ElicitationAction::Accept).with_content(json!({})))
        })
    }
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let schema = Map::from_iter([("type".into(), json!("object"))]);
        Ok(ListToolsResult::with_all_items(vec![McpTool::new(
            "echo",
            "Echo text",
            schema,
        )]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        let key = context
            .meta
            .0
            .0
            .get("io.brainstone/idempotencyKey")
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.calls
            .lock()
            .unwrap()
            .push((json!(request.arguments), json!(context.meta)));
        *self.seen_key.lock().expect("key lock") = key;
        *self.seen_context.lock().unwrap() = context
            .meta
            .0
            .0
            .get("io.brainstone/executionContext")
            .cloned();
        Ok(CallToolResult::success(vec![ContentBlock::text("hello")]).into())
    }
}

fn echo_tool(session: crate::session::Session) -> crate::tool::RemoteTool {
    let schema = Map::from_iter([("type".into(), json!("object"))]);
    crate::tool::RemoteTool::new("local", McpTool::new("echo", "Echo text", schema), session)
        .expect("tool adapts")
}

fn echo_request() -> ToolRequest {
    ToolRequest {
        args: json!({}),
        idempotency_key: None,
        context: None,
    }
}

/// A connector that serves a fresh in-process echo server per invocation.
fn echo_connector() -> crate::session::Connector {
    Arc::new(|| {
        Box::pin(async {
            let (server_io, client_io) = tokio::io::duplex(4_096);
            tokio::spawn(async move {
                let service = Server::default()
                    .serve(server_io)
                    .await
                    .expect("server starts");
                service.waiting().await.ok();
            });
            crate::handler::Handler::new("local", Arc::new(crate::interaction::Decline))
                .serve(client_io)
                .await
                .map_err(|error| crate::Error::new("mcp_connect", error))
        })
    })
}

/// A connected session whose server has already died.
async fn dying_session(connector: Option<crate::session::Connector>) -> crate::session::Session {
    let (server_io, client_io) = tokio::io::duplex(4_096);
    let server = tokio::spawn(async move {
        let service = Server::default()
            .serve(server_io)
            .await
            .expect("server starts");
        service.waiting().await.ok();
    });
    let service = crate::handler::Handler::new("local", Arc::new(crate::interaction::Decline))
        .serve(client_io)
        .await
        .expect("client connects");
    let session = crate::session::SessionState::new(
        service,
        Duration::from_secs(1),
        connector,
        agent::AccessPolicy::default(),
    );
    server.abort();
    wait_for_close(&session).await;
    session
}

async fn wait_for_close(session: &crate::session::SessionState) {
    for _ in 0..500 {
        let guard = session.service.read().await;
        if guard.as_ref().is_some_and(|s| s.is_transport_closed()) {
            return;
        }
        drop(guard);
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    panic!("transport never observed server death");
}

#[tokio::test]
async fn a_dead_connection_is_reestablished_on_the_next_call() {
    let session = dying_session(Some(echo_connector())).await;
    let tool = echo_tool(session);
    let output = tool
        .call(echo_request())
        .await
        .expect("call succeeds after reconnect");
    assert!(output.text.contains("hello"));
}

#[tokio::test]
async fn a_failed_reconnect_surfaces_the_stable_connect_code() {
    let connector: crate::session::Connector = Arc::new(|| {
        Box::pin(async { Err(crate::Error::new("mcp_connect", "server unreachable")) })
    });
    let session = dying_session(Some(connector)).await;
    let tool = echo_tool(session);
    let error = tool
        .call(echo_request())
        .await
        .expect_err("reconnect must fail");
    assert_eq!(error.code, "mcp_connect");
}

#[tokio::test]
async fn a_closed_client_is_not_resurrected_by_its_connector() {
    let session = dying_session(Some(echo_connector())).await;
    let mut client = Client {
        name: "local".into(),
        session: session.clone(),
    };
    client.close().await.expect("client closes");
    let tool = echo_tool(session);
    let error = tool
        .call(echo_request())
        .await
        .expect_err("closed stays closed");
    assert_eq!(error.code, "mcp_closed");
}

#[tokio::test]
async fn discovers_namespaces_and_calls_a_tool() {
    let server = Server::default();
    let seen_key = server.seen_key.clone();
    let (server_io, client_io) = tokio::io::duplex(4_096);
    let server_task = tokio::spawn(async move {
        let service = server.serve(server_io).await.expect("server starts");
        service.waiting().await.expect("server stops");
    });

    let mut client = Client::connect_stream("local", client_io)
        .await
        .expect("client connects");
    let tools = client.tools().await.expect("tools discovered");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name(), "mcp__local__echo");
    assert_eq!(tools[0].definition()["description"], "Echo text");

    let output = tools[0]
        .call(ToolRequest {
            args: json!({"text": "hello"}),
            idempotency_key: Some("operation/1/call/1".into()),
            context: None,
        })
        .await
        .expect("tool succeeds");
    assert_eq!(
        serde_json::from_str::<Value>(&output.text).unwrap()["content"][0]["text"],
        "hello"
    );
    assert_eq!(
        seen_key.lock().unwrap().as_deref(),
        Some("operation/1/call/1")
    );

    client.close().await.expect("client closes");
    server_task.await.expect("server task");
}

#[tokio::test]
async fn tool_calls_obey_the_configured_timeout() {
    let (server_io, client_io) = tokio::io::duplex(4_096);
    tokio::spawn(async move {
        let service = SlowServer.serve(server_io).await.expect("server starts");
        service.waiting().await.expect("server stops");
    });
    let mut client = Client::connect_stream_with_timeout(
        "slow",
        client_io,
        Duration::from_millis(1),
        Arc::new(crate::interaction::Decline),
    )
    .await
    .expect("client connects");
    let tool = crate::tool::RemoteTool::new(
        "slow",
        McpTool::new("wait", "Wait", Map::new()),
        client.session(),
    )
    .expect("tool adapts");

    let error = tool
        .call(ToolRequest {
            args: json!({}),
            idempotency_key: None,
            context: None,
        })
        .await
        .expect_err("call must time out");
    assert_eq!(error.code, "mcp_tool_timeout");
    client.close().await.expect("client closes");
}

#[tokio::test]
async fn mrtr_elicitation_pauses_the_timeout_and_retries_with_the_answer() {
    let (server_io, client_io) = tokio::io::duplex(4_096);
    tokio::spawn(async move {
        let service = MrtrServer.serve(server_io).await.expect("server starts");
        service.waiting().await.expect("server stops");
    });
    let approval = Arc::new(Approve::default());
    let mut client = Client::connect_stream_with_timeout(
        "mrtr",
        client_io,
        Duration::from_millis(1),
        approval.clone(),
    )
    .await
    .expect("client connects");
    let tool = crate::tool::RemoteTool::new(
        "mrtr",
        McpTool::new("approve", "Approve", Map::new()),
        client.session(),
    )
    .expect("tool adapts");
    let output = tool
        .call(ToolRequest {
            args: json!({}),
            idempotency_key: Some("operation/9/step/1/call/a".into()),
            context: None,
        })
        .await
        .expect("MRTR completes");

    assert!(output.text.contains("approved"));
    {
        let seen = approval.0.lock().unwrap();
        assert_eq!(seen[0].call_key, "operation/9/step/1/call/a");
        assert_eq!(seen[0].server, "mrtr");
    }
    client.close().await.expect("client closes");
}

#[tokio::test]
async fn a_task_returned_by_a_tool_is_polled_to_completion() {
    let (server_io, client_io) = tokio::io::duplex(4_096);
    tokio::spawn(async move {
        let service = TaskServer::default()
            .serve(server_io)
            .await
            .expect("server starts");
        service.waiting().await.expect("server stops");
    });
    let mut client = Client::connect_stream("tasks", client_io)
        .await
        .expect("client connects");
    let tool = crate::tool::RemoteTool::new(
        "tasks",
        McpTool::new("later", "Later", Map::new()),
        client.session(),
    )
    .expect("tool adapts");
    let output = tool
        .call(ToolRequest {
            args: json!({}),
            idempotency_key: Some("op/task".into()),
            context: None,
        })
        .await
        .expect("task completes");
    assert!(output.text.contains("from task"));
    client.close().await.expect("client closes");
}

#[tokio::test]
async fn model_arguments_cannot_supply_execution_metadata() {
    let server = Server::default();
    let seen = server.seen_context.clone();
    let (server_io, client_io) = tokio::io::duplex(4096);
    tokio::spawn(async move {
        server.serve(server_io).await.unwrap().waiting().await.ok();
    });
    let mut client = Client::connect_stream("identity", client_io).await.unwrap();
    let tools = client.tools().await.unwrap();
    tools[0]
        .call(ToolRequest {
            args: json!({"_meta": {"io.brainstone/executionContext": {"actor": "victim"}}}),
            idempotency_key: Some("retry-key".into()),
            context: None,
        })
        .await
        .unwrap();
    assert_eq!(
        *seen.lock().unwrap(),
        Some(json!({"version": 1, "identity": null}))
    );
    client.close().await.unwrap();
}

fn identity_request(tenant: &str) -> ToolRequest {
    ToolRequest {
        args: json!({"tenant": tenant, "actor": "victim", "_meta": {"io.brainstone/executionContext": {"actor": "victim"}}}),
        idempotency_key: Some(tenant.into()),
        context: Some(agent::ToolContext {
            operation_id: 1,
            profile: agent::ExecutionProfile::default(),
            identity: Some(agent::ExecutionIdentity {
                provider: "test".into(),
                tenant: tenant.into(),
                actor: "U1".into(),
                conversation: "D1".into(),
                private: true,
            }),
        }),
    }
}

#[tokio::test]
async fn concurrent_callers_cannot_override_or_cross_contaminate_metadata() {
    let server = Server::default();
    let seen = server.calls.clone();
    let (server_io, client_io) = tokio::io::duplex(4096);
    tokio::spawn(async move {
        server.serve(server_io).await.unwrap().waiting().await.ok();
    });
    let mut client = Client::connect_stream("identity", client_io).await.unwrap();
    let tools = client.tools().await.unwrap();
    let (a, b) = tokio::join!(
        tools[0].call(identity_request("T1")),
        tools[0].call(identity_request("T2"))
    );
    a.unwrap();
    b.unwrap();
    {
        let calls = seen.lock().unwrap();
        assert_eq!(calls.len(), 2);
        for (args, meta) in calls.iter() {
            assert_eq!(
                meta["io.brainstone/executionContext"]["identity"]["tenant"],
                args["tenant"]
            );
            assert_eq!(
                meta["io.brainstone/executionContext"]["identity"]["actor"],
                "U1"
            );
            assert_eq!(meta["io.brainstone/idempotencyKey"], args["tenant"]);
        }
    }
    client.close().await.unwrap();
}

#[tokio::test]
async fn restricted_tools_reject_direct_calls_before_connecting() {
    let mut session = dying_session(None).await;
    Arc::get_mut(&mut session).unwrap().access = agent::AccessPolicy {
        authenticated_actor: true,
        private_conversation: true,
    };
    let tool = echo_tool(session);
    assert_eq!(
        tool.call(echo_request()).await.unwrap_err().code,
        "authenticated_actor_required"
    );
    let mut request = identity_request("T1");
    request
        .context
        .as_mut()
        .unwrap()
        .identity
        .as_mut()
        .unwrap()
        .private = false;
    assert_eq!(
        tool.call(request).await.unwrap_err().code,
        "private_conversation_required"
    );
}

#[tokio::test]
async fn waiting_for_a_busy_server_has_a_deadline() {
    let session = dying_session(Some(echo_connector())).await;
    let held = session.call.lock().await;
    let tool = echo_tool(session.clone());
    let result = tokio::time::timeout(Duration::from_secs(2), tool.call(echo_request())).await;
    drop(held);
    assert_eq!(
        result
            .expect("queue wait must be bounded")
            .unwrap_err()
            .code,
        "mcp_queue_timeout"
    );
}

#[tokio::test]
async fn a_timed_out_call_is_not_replayed_and_the_next_call_reconnects() {
    let (server_io, client_io) = tokio::io::duplex(4_096);
    tokio::spawn(async move {
        SlowServer
            .serve(server_io)
            .await
            .unwrap()
            .waiting()
            .await
            .ok();
    });
    let service = crate::handler::Handler::new("local", Arc::new(crate::interaction::Decline))
        .serve(client_io)
        .await
        .unwrap();
    let session = crate::session::SessionState::new(
        service,
        Duration::from_millis(10),
        Some(echo_connector()),
        agent::AccessPolicy::default(),
    );
    let tool = echo_tool(session);
    assert_eq!(
        tool.call(echo_request()).await.unwrap_err().code,
        "mcp_tool_timeout"
    );
    assert!(
        tool.call(echo_request())
            .await
            .unwrap()
            .text
            .contains("hello")
    );
}

#[derive(Clone)]
struct CancellationServer(Arc<tokio::sync::Notify>);
impl ServerHandler for CancellationServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
    async fn call_tool(
        &self,
        _: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        std::future::pending().await
    }
    fn on_cancelled(
        &self,
        _: rmcp::model::CancelledNotificationParam,
        _: rmcp::service::NotificationContext<RoleServer>,
    ) -> impl Future<Output = ()> + Send + '_ {
        self.0.notify_one();
        std::future::ready(())
    }
}

#[tokio::test]
async fn timeout_sends_protocol_cancellation_before_invalidating_the_connection() {
    let cancelled = Arc::new(tokio::sync::Notify::new());
    let server = CancellationServer(cancelled.clone());
    let (server_io, client_io) = tokio::io::duplex(4096);
    tokio::spawn(async move {
        server.serve(server_io).await.unwrap().waiting().await.ok();
    });
    let mut client = Client::connect_stream_with_timeout(
        "cancel",
        client_io,
        Duration::from_millis(20),
        Arc::new(crate::interaction::Decline),
    )
    .await
    .unwrap();
    let tool = echo_tool(client.session());
    assert_eq!(
        tool.call(echo_request()).await.unwrap_err().code,
        "mcp_tool_timeout"
    );
    tokio::time::timeout(Duration::from_secs(1), cancelled.notified())
        .await
        .expect("server received explicit cancellation");
    client.close().await.unwrap();
}

#[tokio::test]
async fn dropping_an_inflight_call_invalidates_it_for_the_next_caller() {
    let (server_io, client_io) = tokio::io::duplex(4096);
    tokio::spawn(async move {
        SlowServer
            .serve(server_io)
            .await
            .unwrap()
            .waiting()
            .await
            .ok();
    });
    let service = crate::handler::Handler::new("local", Arc::new(crate::interaction::Decline))
        .serve(client_io)
        .await
        .unwrap();
    let session = crate::session::SessionState::new(
        service,
        Duration::from_secs(5),
        Some(echo_connector()),
        agent::AccessPolicy::default(),
    );
    let tool = echo_tool(session);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), tool.call(echo_request()))
            .await
            .is_err()
    );
    assert!(
        tool.call(echo_request())
            .await
            .unwrap()
            .text
            .contains("hello")
    );
}

/// Base64 of `%PDF-1.7\n`, nine bytes.
const PDF_BASE64: &str = "JVBERi0xLjcK";

/// A complete, decodable 1x1 PNG.
pub(crate) const PNG_1X1_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/// Answers the way the GitHub MCP server answers a request for a PDF: a text
/// block, then the file as a base64 blob resource. An image and a text file
/// ride along to show what is and is not binary.
#[derive(Clone, Default)]
struct FileServer;

impl ServerHandler for FileServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let schema = Map::from_iter([("type".into(), json!("object"))]);
        Ok(ListToolsResult::with_all_items(vec![McpTool::new(
            "get_file_contents",
            "Read a file",
            schema,
        )]))
    }

    async fn call_tool(
        &self,
        _request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        let pdf = ResourceContents::blob(PDF_BASE64.repeat(1000), "repo://o/r/contents/req.pdf")
            .with_mime_type("application/pdf");
        Ok(CallToolResult::success(vec![
            ContentBlock::text("successfully downloaded binary file (SHA: abc)"),
            ContentBlock::resource(pdf),
            ContentBlock::image("iVBORw0KGgo=", "image/png"),
            ContentBlock::image(PNG_1X1_BASE64, "image/png"),
            ContentBlock::embedded_text("repo://o/r/contents/a.php", "<?php echo 1;"),
        ])
        .into())
    }
}

/// The model reads text, so base64 costs context for nothing: one inlined
/// PDF template filled a lane past its window. A binary block the model
/// cannot see reaches it as a note naming what it was; a decodable image
/// reaches it as an image, named in the text so the reply can show it; text
/// is untouched.
#[tokio::test]
async fn images_reach_the_model_and_a_fetched_pdf_is_kept_for_the_reply() {
    let (server_io, client_io) = tokio::io::duplex(4_096);
    let server_task = tokio::spawn(async move {
        let service = FileServer.serve(server_io).await.expect("server starts");
        service.waiting().await.expect("server stops");
    });
    let mut client = Client::connect_stream("github", client_io)
        .await
        .expect("client connects");
    let tools = client.tools().await.expect("tools discovered");

    let output = tools[0].call(echo_request()).await.expect("tool succeeds");

    let text = output.text.clone();
    assert!(
        !text.contains(PDF_BASE64) && !text.contains("iVBORw0KGgo"),
        "{text}"
    );
    assert!(text.contains("successfully downloaded binary file (SHA: abc)"));
    assert!(text.contains("<?php echo 1;"));
    assert_eq!(output.files.len(), 1);
    assert_eq!(
        (output.files[0].name(), output.files[0].media_type()),
        ("req.pdf", "application/pdf")
    );
    assert_eq!(output.files[0].bytes().len(), 9000);
    let link = format!("[req.pdf]({})", media::file_reference(&output.files[0]));
    assert!(text.contains(&link), "{text}");
    assert!(
        text.contains("[mcp_image_invalid: image (image/png, 8 bytes)"),
        "{text}"
    );
    assert_eq!(output.images.len(), 1);
    assert_eq!(
        (output.images[0].width(), output.images[0].height()),
        (1, 1)
    );
    let reference = media::reference(&output.images[0]);
    assert!(
        text.contains(&format!("![short description]({reference})")),
        "{text}"
    );
    client.close().await.expect("client closes");
    server_task.await.expect("server task");
}
