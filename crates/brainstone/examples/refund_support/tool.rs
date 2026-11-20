use super::ledger;
use brainstone::{Tool, ToolError, typed_tool};
use schemars::JsonSchema;
use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Refund {
    #[schemars(length(min = 1))]
    order_id: String,
    #[schemars(range(min = 1, max = 1000))]
    amount_cents: u32,
}

pub(crate) fn tool(pool: SqlitePool, crash: bool) -> Result<impl Tool, ToolError> {
    typed_tool(
        "Refund",
        "Issue a simulated refund of 1 to 1000 cents.",
        move |args: Refund, invocation| {
            let pool = pool.clone();
            async move {
                let key = invocation.require_idempotency_key()?;
                let receipt = ledger::record(&pool, key, &args.order_id, args.amount_cents)
                    .await
                    .map_err(ToolError::from)?;
                if crash {
                    eprintln!(
                        "Simulated refund committed; exit before tool-result commit. Restart without --crash."
                    );
                    std::process::exit(73);
                }
                Ok::<_, ToolError>(receipt)
            }
        },
    )
}
