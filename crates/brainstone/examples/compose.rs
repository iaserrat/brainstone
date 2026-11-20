//! The full engine, composed from settings: providers, MCP servers, skills,
//! subagents, schedules, and telemetry, driven by your own ingress.
//!
//! ```sh
//! DATABASE_URL=sqlite://compose-agent.db?mode=rwc \
//! MODEL_PROVIDER=anthropic ANTHROPIC_API_KEY=... \
//! cargo run -p brainstone --example compose
//! ```
//!
//! Configuration and the agent directory (prompts, skills, MCP servers) come
//! from `examples/minimal-agent`; `docs/operations.md` lists every variable.
//! The engine's model loop calls the configured provider, so this example
//! needs a reachable model to print text.
use std::path::Path;

use brainstone::Settings;
use brainstone::agent::ToolError;
use brainstone::host::Admission;
use brainstone::runtime::{Address, Destination, Prompt};
use brainstone::surface::{Destinations, PostRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Environment variables are the configuration contract; the path selects
    // the agent directory to compose.
    let settings = Settings::from_map(
        &std::env::vars().collect(),
        Path::new("examples/minimal-agent"),
    )?;
    let mut engine = brainstone::build(settings, Some(Surface)).await?;

    // Your ingress decides what a conversation is. Here: one lane in one
    // conversation, one accepted prompt, driven to its settled outcome.
    let address = Address::new("demo", "conversation-42", "main")?;
    let prompt = Prompt::new(
        address,
        "request-1",
        "You are a useful assistant.",
        "Summarize what makes a durable agent different in one sentence.",
    )?;
    let id = engine.admission.accept(&prompt).await?;
    let outcome = engine.host.execute(id).await?;
    println!("{outcome:?}");

    engine.shutdown().await;
    Ok(())
}

/// The seam your interface implements: whether a schedule may post into a
/// conversation you host, and where. This one refuses; a chat surface would
/// return the durable `Destination` its conversations map to. Pass `None`
/// instead to keep the schedule tools off entirely.
struct Surface;

#[expect(
    clippy::unused_async_trait_impl,
    reason = "The seam is async; a refusing surface simply never awaits."
)]
impl Destinations for Surface {
    async fn resolve(&self, _: PostRequest<'_>) -> Result<Destination, ToolError> {
        Err(ToolError::new(
            "post_denied_by_surface",
            "this surface does not accept scheduled posts",
        ))
    }
}
