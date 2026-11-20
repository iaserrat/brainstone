# brainstone

Compose a durable agent with `Runtime`, `Host`, and `Registry`. These APIs
work without environment parsing, an agent directory, a surface, or background
tasks.

```rust,no_run
use brainstone::{Address, Host, Model, Prompt, Registry, Runtime};
use brainstone::runtime::SqliteStore;

async fn run(model: impl Model) -> Result<(), Box<dyn std::error::Error>> {
    let store = SqliteStore::connect("sqlite://agent.db").await?;
    let host = Host::new(Runtime::new(store.clone()), model, Registry::default());
    let address = Address::new("app", "conversation", "main")?;
    let prompt = Prompt::new(address, "request-1", "Be useful.", "Hello")?;
    let accepted = host.accept(&prompt).await?;
    let operation = host.execute(accepted.operation.id).await?;
    host.shutdown().await;
    store.close().await;
    Ok(())
}
```

`Host::accept` queues work and returns `Accepted`, including the durable
operation and whether this is a duplicate. `Host::admit` instead steers an
active operation, or accepts one when idle. The `Admission` trait uses that
steering behavior and returns its operation ID. `execute(id)` drives only that
operation, waits for another writer when necessary, and returns an already
committed terminal outcome without running inference again.

Implement `Model` to supply inference and `Tool` for native tools. A heterogeneous
`Registry` accepts `Arc<dyn Tool>`; a model or custom store stays generic.
Tool definitions and handlers travel together. External effects must atomically
honor the retry key supplied in `ToolRequest`; the framework cannot make an
external service idempotent.

Use `Runtime::with_config` for leases, compaction, and authorization;
`Host::with_config` controls worker count and polling. Admission decorators for
memory, skills, model selection, and gates remain independently composable.

## Ownership and shutdown

Construction starts no workers. Call `host.execute` for a single operation,
or spawn `host.run` for a resident pool and clone the host into ingress handlers.
From a separate process-owner task, call `host.shutdown`, join a spawned worker
task, then close resources you created. Shutdown rejects new host admission and
waits for currently claimed operations; accepted unclaimed work stays durable.
Do not call shutdown from a tool executing on that host.

A custom model or tool may own additional resources; their lifetime belongs to
the embedding application. Cloned built-in stores share a pool, so closing one
closes the pool for every clone.

## Cargo features

```toml
brainstone = { version = "0.1", default-features = false }
```

This exposes `agent`, `runtime`, `host`, and `media`, plus the common composition
types at the crate root. Both durable backends remain available. It excludes
the facade's configured provider and capability dependencies.

| Feature | Public adapter or capability |
| --- | --- |
| `anthropic` | Anthropic Messages and Bedrock adapter |
| `openai` | Responses and Chat Completions adapters |
| `mcp` | Local and remote MCP tools |
| `memory` | Memory tools and admission |
| `skills` | Agent Skills discovery and admission |
| `subagents` | Durable child-agent tools |
| `policy` | Schedule, silence, and compaction policy |
| `surface` | Surface input and destination contracts |
| `telemetry` | Observability initialization and JSON logging |
| `configured` (default) | Settings and the complete deployment preset; enables all above |

Features are additive. Existing default dependencies and module-qualified imports
continue to work. Choose adapters explicitly when using `default-features = false`.
The local gate checks core-only tests/docs and each optional adapter independently.

## Configured preset

```rust,no_run
use brainstone::{Admission, Settings};

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let settings = Settings::from_env()?;
let mut engine = brainstone::configured(settings).await?;
# let prompt = brainstone::Prompt::new(brainstone::Address::new("app", "conversation", "main")?, "request-1", "Be useful.", "Hello")?;
let id = engine.admission.accept(&prompt).await?;
let operation = engine.host.execute(id).await?;
engine.shutdown().await;
# Ok(())
# }
```

`configured(settings)` omits schedule tools without requiring a destination
implementation. Existing `build(settings, Some(destinations))` enables them.
The preset owns its resources. `Engine::shutdown` closes the host, MCP
clients, and database in order; repeated calls after completion are safe. Join
any separately spawned `host.run` task. Custom applications follow the
ownership rules above.

`build` connects the selected durable database, applies the schema baseline,
builds the configured model and capabilities, connects MCP servers, and
returns a durable host. Its admission chain applies gates, memory injection,
skills, and model selection. The optional `destinations` argument decides where a
schedule's runs post; without one the schedule tools are not offered.

It composes the autonomy policy into the existing seams: active-lane messages
become durable steering, context is summarized automatically before the model
window is exhausted, and the `Schedule`, `ListSchedules`, and `CancelSchedule`
tools create durable one-shot and standing follow-ups, with a
`SCHEDULE_MIN_INTERVAL_MINUTES` floor on fixed intervals. With `FILE_TOOL=true`
it registers `media::CreateFile`. No separate scheduler process runs; ordinary
worker claims release due work.

At startup, it reads `prompts/system_prompt.md` relative to the working
directory you give `Settings::from_map`. The file must exist, contain valid
UTF-8, and include non-whitespace text.

It also recursively loads `agents/**/*.md`, validates those definitions
against the composed MCP and control-tool catalog, and registers durable
subagent controls. Invalid definitions stop startup. Restart to load edits;
already-created children keep their stored definition snapshot.

Project Agent Skills are discovered from direct child directories under
`skills/`. The engine registers metadata-only discovery plus lazy instruction
and resource tools, and expands explicit `$skill-name` mentions before durable
admission. Invalid individual skills are logged and skipped.

Operations have no response-count limit by default. Set `MAX_TURNS` to a
positive integer to impose one, or `unlimited` to remove it explicitly. Idle
lanes adopt a changed value with their next operation; active operations keep
their accepted limit. Project agents inherit it unless their frontmatter sets
`maxTurns` to a positive integer or `unlimited`.

The process does not bind a port or run an HTTP server. Call
`engine.shutdown().await` from the process owner to stop admission, wait for
claimed work, close MCP clients, and close the shared database pool.

The [agent-building guide](../../docs/building-agents.md) walks through the
`chat` and `order_support` examples with provider setup, retry keys, conversation
history, and a native lookup tool. Both require the `openai` feature.

The other compiled examples cover three entry points: `recover` demonstrates custom
Rust composition and a process crash, `echo` drives the runtime directly, and
`compose` drives the configured preset.

## Structured final answers

Create an `OutputSchema`, attach it with `ExecutionProfile::with_output_schema`,
and use that profile on the prompt. The schema is durable, reaches native
provider request fields, and rejects an invalid final document before success.
See [structured outputs](../../docs/structured-output.md) for examples and
[SQLite evaluations](../../docs/evaluations.md) for observed tool/schema interactions.

## Typed tools and effect requirements

Use `typed_tool` with a typed async handler and register it through
`Registry::default().with_tool(tool)`. Attach a `ToolContract` to the prompt's
`ExecutionProfile` to restrict tools, require approval, or require successful
calls before completion. You can also implement `Tool` directly.
See the [tool guide](../../docs/tools.md) and the local
[refund example](examples/refund.rs), which requires no API credentials.
