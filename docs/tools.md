# Typed tools and tool contracts

Use `typed_tool` for JSON arguments and results. It adapts an async Rust function
to the same `Tool` seam used by native handlers and MCP tools. Use `Tool` directly
when returning images or files.

## Define a typed tool

Add these dependencies to your application:

```toml
[dependencies]
brainstone = { version = "0.1", default-features = false }
serde = { version = "1", features = ["derive"] }
schemars = "1"
```

This complete program registers a typed lookup tool without contacting a model:

```rust
use brainstone::{Registry, ToolError, ToolInvocation, typed_tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Lookup {
    order_id: String,
}

#[derive(Serialize)]
struct Order {
    status: &'static str,
}

enum LookupError {
    NotFound,
}

impl From<LookupError> for ToolError {
    fn from(_: LookupError) -> Self {
        Self::new("order_not_found", "No demo order has that ID.")
    }
}

async fn lookup(args: Lookup, _: ToolInvocation) -> Result<Order, LookupError> {
    match args.order_id.as_str() {
        "BS-1001" => Ok(Order { status: "shipped" }),
        _ => Err(LookupError::NotFound),
    }
}

fn main() -> Result<(), ToolError> {
    let tool = typed_tool("LookupOrder", "Look up a demo order.", lookup)?;
    let tools = Registry::default().with_tool(tool);
    println!("{:?}", tools.definitions());
    Ok(())
}
```

The [order-support example](../crates/brainstone/examples/support/order_lookup.rs)
uses the same pattern with a provider and stored conversation history.

`typed_tool(name, description, handler)` derives and compiles the input schema
at construction. Names and descriptions must be non-blank, and the input schema
must describe an object. Use a struct for top-level arguments; nested structs
and local schema references work. It uses Serde's deserialization shape,
including field names and defaults. Arguments must pass both schema validation and deserialization
before the handler runs. Use `#[serde(deny_unknown_fields)]` to reject extra fields.
Add `#[schemars(...)]` constraints for bounds that the Rust type alone cannot express.

The handler receives `ToolInvocation` as its second argument. It provides:

- `idempotency_key()`: the driver's stable retry key, if present.
- `require_idempotency_key()`: the key, or `tool_durability_required`.
- `context()`: operation ID, authenticated identity, and accepted profile, if present.

The driver supplies this metadata; it does not come from model arguments.
Require the key before changing external state, and use it to deduplicate
retries.

Successful calls return one JSON document:

```json
{"outcome":"success","value":{"order_id":"BS-1001","status":"shipped"}}
```

Represent expected domain failures as an enum. Convert it to `ToolError`
with `From<YourError>`. Assign each variant a stable code; keep details in the message.
The executor produces a JSON failure document with `error` and `message` fields.
For example: `{"error":"order_not_found","message":"No demo order has that ID."}`.
Encoding failures and outputs larger than `MAX_TOOL_RESULT_BYTES` return `tool_output_invalid`.
Oversized JSON is rejected rather than truncated.

| Code | Meaning |
| --- | --- |
| `invalid_tool_definition` | The name, description, or derived object schema is invalid |
| `invalid_tool_arguments` | Arguments fail schema validation or Serde decoding |
| `tool_output_invalid` | Successful output cannot serialize or exceeds the byte limit |
| `tool_durability_required` | The handler requires a retry key but the driver supplied none |

## Constrain an operation

`ToolContract` belongs to the accepted `ExecutionProfile`. It persists with the
prompt, so recovery uses the same requirements. The
[refund example](../crates/brainstone/examples/refund.rs) constructs its contract as:

```rust
let contract = ToolContract::only(["Refund"])?
    .require_approval(["Refund"])?
    .require_success(["Refund"])?;
let profile = ExecutionProfile::new(None, None, 3, 0)?.with_tool_contract(contract);
```

`only` restricts both model-visible tools and dispatch to exact names. An empty
list permits none. Omit the contract to preserve unrestricted behavior over the
registered tools. A default contract permits all registered tools without adding
approval or success requirements. Names are sorted and deduplicated; wildcards
are not accepted.

`require_approval` uses the runtime's durable approval records. Inspect the exact
request through `DurableStore::approval`, then record a decision with
`decide_approval`. A changed call, definition, identity, or policy binding requires
a separate decision. If executor behavior changes without a definition change,
change the operator policy binding to invalidate old approvals.
The framework stores the binding; applications do not need
to invent one for accepted approval requirements. Operator `ToolPolicy` can add
restrictions, and its denial takes precedence.

`require_success` requires a committed successful result from each named tool in
this operation. Model claims, failed calls, denied approvals, and successes in
older operations do not satisfy it. Evidence survives steering, compaction, and
recovery. A missing result fails terminal completion with
`tool_requirement_unmet`; it does not trigger hidden repair calls to the model.

A successful result records the handler's report. It cannot prove an amount,
recipient, or business outcome. Check those conditions in the handler and
limit arguments to the operation's authorized scope.
The contract applies to this operation. Permitted tools still own any delegated
actions; child agents have their own profiles and tool grants.

## Run the simulated refund

No credentials or network access are required:

```sh
cargo run -p brainstone --no-default-features --example refund -- --approve --crash
cargo run -p brainstone --no-default-features --example refund -- --approve
```

The first command exits with code 73 after committing the simulated effect. The
second resumes it with the same retry key. The application's SQLite ledger holds
one refund row and returns the same receipt. Run the second command again to read
the stored terminal outcome. Add a fresh SQLite URL as the last argument to start
over. Without `--approve`, the program exits before opening a database.

The flag approves only this fixed demonstration. Real applications must obtain
an authorized operator's decision. For payments or other remote effects, use the
remote service's idempotency API or reconcile uncertain outcomes; a retry key by
itself cannot guarantee exactly-once execution.

The demo reports `refund_conflict` if a retry key carries different refund
arguments, and `refund_storage_failed` if its effect ledger cannot complete a
database operation. These codes belong to the example application, not the
framework.
