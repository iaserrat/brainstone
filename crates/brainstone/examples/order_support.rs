//! A support agent with a native, read-only order tool and durable history.
#[path = "support/order_lookup.rs"]
mod orders;
#[path = "support/session.rs"]
mod session;

use brainstone::Registry;

const SYSTEM_PROMPT: &str = "You help with demo orders. Use LookupOrder before stating an order's status. If an order is missing, ask the user to check its ID. Do not invent order facts or promise refunds.";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tools = Registry::default().with_tool(orders::tool()?);
    session::run("order-support", SYSTEM_PROMPT, tools).await
}
