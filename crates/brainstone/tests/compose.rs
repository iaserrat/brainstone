//! The facade composes a durable agent and drives one prompt with no
//! interface layer attached: no CLI binary, no Slack, no Docker.

#[path = "support/model_endpoint.rs"]
mod http;

use brainstone::agent::ToolError;
use brainstone::host::Admission;
use brainstone::runtime::{Address, Destination, OperationState, Prompt};
use brainstone::{Settings, surface};
use serde_json::json;
use std::collections::HashMap;

/// Not offered any schedule; `resolve` must never be called.
struct NoDestinations;

impl surface::Destinations for NoDestinations {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "Never called; the schedule tools are not offered."
    )]
    async fn resolve(&self, _request: surface::PostRequest<'_>) -> Result<Destination, ToolError> {
        unreachable!("schedule tools are not offered")
    }
}

fn text_model_settings(endpoint: &str, database: &std::path::Path) -> HashMap<String, String> {
    HashMap::from([
        (
            "DATABASE_URL".to_owned(),
            format!("sqlite://{}", database.join("runtime.db").display()),
        ),
        ("WORKER_ID".to_owned(), "compose".to_owned()),
        ("MODEL_PROVIDER".to_owned(), "openai".to_owned()),
        ("MODEL_NAME".to_owned(), "local-model".to_owned()),
        ("MODEL_ENDPOINT".to_owned(), endpoint.to_owned()),
        ("MODEL_PROTOCOL".to_owned(), "chat-completions".to_owned()),
        ("MODEL_TARGET".to_owned(), "compose".to_owned()),
        ("MODEL_AUTH".to_owned(), "none".to_owned()),
        ("MODEL_CAPABILITIES".to_owned(), "text".to_owned()),
    ])
}

#[tokio::test]
async fn the_facade_composes_and_drives_one_durable_prompt() {
    let answer = json!({"choices":[{"message":{"content":"done"},"finish_reason":"stop"}]});
    let (endpoint, _captured) = http::serve_once(answer).await;

    let database = tempfile::tempdir().unwrap();
    let values = text_model_settings(&endpoint, database.path());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/compose");
    let settings = Settings::from_map(&values, &root).unwrap();

    let mut engine = brainstone::build(settings, Option::<NoDestinations>::None)
        .await
        .unwrap();

    let address = Address::new("compose", "conversation", "main").unwrap();
    let prompt = Prompt::new(address, "request-1", "Be useful.", "hello").unwrap();
    let id = engine.admission.accept(&prompt).await.unwrap();
    let operation = engine.host.execute(id).await.unwrap();
    assert_eq!(operation.state, OperationState::Completed);

    engine.host.shutdown().await;
    engine.mcp.close().await;
    engine.store.close().await;
}
