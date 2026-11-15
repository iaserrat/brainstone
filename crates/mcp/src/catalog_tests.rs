#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! A server's catalog follows its live tool list; a failed listing keeps the
//! tools listed before.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use agent::{Registry, ToolSource};
use rmcp::{
    ServerHandler, ServiceExt,
    model::{
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool as McpTool,
    },
    service::{RequestContext, RoleServer},
};
use serde_json::{Map, Value, json};

use crate::Client;
use crate::catalog::{Catalog, ServerTools};

#[derive(Clone, Default)]
struct Changing {
    names: Arc<Mutex<Vec<&'static str>>>,
    failing: Arc<AtomicBool>,
}

impl ServerHandler for Changing {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        if self.failing.load(Ordering::Acquire) {
            return Err(rmcp::ErrorData::internal_error("listing failed", None));
        }
        let schema = Map::from_iter([("type".into(), json!("object"))]);
        let names = self.names.lock().unwrap().clone();
        let tools = names
            .into_iter()
            .map(|name| McpTool::new(name, "tool", schema.clone()));
        Ok(ListToolsResult::with_all_items(tools.collect()))
    }
}

async fn catalog(server: Changing, allowlist: Option<&[String]>) -> (Client, Registry) {
    let (server_io, client_io) = tokio::io::duplex(4_096);
    tokio::spawn(async move {
        let service = server.serve(server_io).await.expect("server starts");
        // software-design-allow: ignored-error a detached test server; no one awaits it
        service.waiting().await.ok();
    });
    let client = Client::connect_stream("local", client_io).await.unwrap();
    let tools = ServerTools::listed(&client, allowlist).await.unwrap();
    let source: Arc<dyn ToolSource> = Arc::new(Catalog::new(vec![tools]));
    (client, Registry::from_sources(vec![source]))
}

fn names(registry: &Registry) -> Vec<Value> {
    let definitions = registry.definitions();
    definitions
        .iter()
        .map(|tool| tool["name"].clone())
        .collect()
}

#[tokio::test]
async fn a_refresh_offers_tools_the_server_added_since_startup() {
    let server = Changing::default();
    server.names.lock().unwrap().push("search");
    let (_client, registry) = catalog(server.clone(), None).await;
    server.names.lock().unwrap().push("create");

    let registry = registry.listed().await;

    assert_eq!(
        names(&registry),
        [json!("mcp__local__search"), json!("mcp__local__create")]
    );
    assert!(registry.lookup("mcp__local__create").is_some());
}

#[tokio::test]
async fn a_failed_listing_keeps_the_tools_listed_before() {
    let server = Changing::default();
    server.names.lock().unwrap().push("search");
    let (_client, registry) = catalog(server.clone(), None).await;
    server.failing.store(true, Ordering::Release);

    let registry = registry.listed().await;

    assert_eq!(names(&registry), [json!("mcp__local__search")]);
}

#[tokio::test]
async fn a_refresh_applies_the_servers_enabled_tools() {
    let server = Changing::default();
    server.names.lock().unwrap().push("search");
    let allowlist = ["search".to_owned()];
    let (_client, registry) = catalog(server.clone(), Some(&allowlist)).await;
    server.names.lock().unwrap().push("delete");

    let registry = registry.listed().await;

    assert_eq!(names(&registry), [json!("mcp__local__search")]);
}
