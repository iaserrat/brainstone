//! Durable CLI conversation. See docs/building-agents.md for setup and retries.
#[path = "support/session.rs"]
mod session;

use brainstone::Registry;

const SYSTEM_PROMPT: &str =
    "You are a concise assistant. Remember facts the user shares in this conversation.";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    session::run("chat", SYSTEM_PROMPT, Registry::default()).await
}
