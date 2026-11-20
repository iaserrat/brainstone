<p align="center">
  <img src="docs/assets/logo.png" alt="Brainstone, Rust Agent Engine" width="320" height="320">
</p>

<h1 align="center">Brainstone</h1>

<p align="center">
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/Rust-1.100.0%2B-orange?logo=rust" alt="Rust 1.100.0+" height="20"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/edition-2024-blue" alt="Edition 2024" height="20"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="License: MIT" height="20"></a>
</p>

Build durable AI agents in Rust. Your application supplies the model, tools,
and interface. Brainstone accepts prompts, runs the model/tool loop, and stores
what it needs to recover after a crash or deploy. Start with SQLite on one host
or use PostgreSQL across hosts.

## The primitives

| Primitive | What you supply or use |
| --- | --- |
| `Model` | Inference through OpenAI, Anthropic, Bedrock, or your own implementation |
| `Tool` / `typed_tool` | Capabilities the model can call, with typed arguments, results, and errors |
| `Registry` | The tools available to an agent, including native and MCP tools |
| `ExecutionProfile` | Model settings, response limit, output schema, and tool contract |
| `ToolContract` | Allowed tools, required approvals, and successful calls required before completion |
| `Runtime` | Durable acceptance, execution, leases, and recovery over a store |
| `Host` | Acceptance and execution of a named operation, or a resident worker pool |
| `Prompt` | A request ID, address, system instructions, input, and execution profile |
| `Outcome` | The committed answer or a failure with a stable code |

An `Address` contains a surface, conversation, and lane. A conversation maps to
one session; lanes hold its sub-conversations. Each session has one writer.
Each accepted prompt becomes an operation with a durable receipt.

## Try the coding agent

The [coding agent](crates/brainstone/examples/coding_agent.rs) reads a Rust
project, edits a file, and runs its tests. Start with the local demo:

```sh
cargo run -p brainstone --no-default-features --features openai --example coding_agent -- \
  demo data/coding-demo
```

This creates a small broken project in a new or empty directory. A scripted
model reads `src/lib.rs`, requests the fix, and calls the test tool. No API key
or network request is needed. The demo approves only its fixed example calls.
Run the same command again to read the stored answer without repeating work.

The same agent can use a real model on your project:

```sh
export OPENAI_API_KEY='your-api-key'
export OPENAI_MODEL='your-model-id'
cargo run -p brainstone --no-default-features --features openai --example coding_agent -- \
  run ./my-project fix-1 'Fix the failing test in src/lib.rs'
```

It asks you to approve each exact edit and test command. Its tools run with your
process permissions in a trusted workspace. The example requires an edit and
passing library tests; it is not a general read-only assistant. See the
[coding-agent guide](docs/coding-agent.md) for limits, resume commands, and
provider configuration.

## Compose in Rust

Compose the same pieces in your own binary. No agent directory or framework
environment settings are required. This minimal example reads `OPENAI_API_KEY` and `OPENAI_MODEL` from your environment. Choose
a model ID available to your account:

```toml
[dependencies]
brainstone = { version = "0.1", default-features = false, features = ["openai"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use brainstone::{Address, Host, Prompt, Registry, Runtime};
use brainstone::openai::ResponsesModel;
use brainstone::runtime::SqliteStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = SqliteStore::connect("sqlite://agent.db").await?;
    let model = ResponsesModel::openai(std::env::var("OPENAI_MODEL")?)?;
    let host = Host::new(Runtime::new(store.clone()), model, Registry::default());

    let address = Address::new("app", "conversation-42", "main")?;
    let prompt = Prompt::new(address, "request-1", "Be useful.", "Hello")?;
    let accepted = host.accept(&prompt).await?;
    let operation = host.execute(accepted.operation.id).await?;
    println!("{:?}", operation.outcome);

    host.shutdown().await;
    store.close().await;
    Ok(())
}
```

`accept` commits the prompt and returns a receipt before inference starts.
`execute` drives that operation or returns its stored result. Retrying the same
request ID and input returns the original operation. Changed input under the
same ID fails with `idempotency_conflict`.

### Add tools and execution requirements

Write an async function with a typed input and output, then register it with
`typed_tool` and `Registry::with_tool`. Brainstone derives its input schema,
validates arguments before calling it, and preserves typed error codes.
The [tool guide](docs/tools.md) has a complete program.

A coding workflow can pin its requirements in the prompt's profile:

```rust
use brainstone::{ExecutionProfile, ToolContract};

let contract = ToolContract::only(["ReadFile", "EditFile", "RunChecks"])?
    .require_approval(["EditFile", "RunChecks"])?
    .require_success(["EditFile", "RunChecks"])?;
let profile = ExecutionProfile::new(None, None, 12, 0)?
    .with_tool_contract(contract);
let prompt = prompt.with_profile(profile);
```

This fragment assumes a prompt and registry containing those tools. The model
cannot bypass the allowlist by inventing a call name. An approved call still
needs a successful committed result to satisfy `require_success`. Text claiming
that a tool ran does not count. Attach an [`OutputSchema`](docs/structured-output.md)
to require a validated JSON answer as well.

The coding example combines these contracts. It also rejects edits after a
successful check, so an earlier test result cannot cover a later edit.

### Recover after a crash

Use a fresh demo directory to stop after the file changes but before its tool
result commits:

```sh
cargo run -p brainstone --no-default-features --features openai --example coding_agent -- \
  demo data/coding-recovery --crash
cargo run -p brainstone --no-default-features --features openai --example coding_agent -- \
  demo data/coding-recovery
```

The first command exits with code 73. Recovery reopens the accepted operation.
The edit tool sees that the file already matches the approved replacement and
returns the same receipt; the agent then runs tests and commits its answer.
State lives in the project's `.brainstone/` directory.

Durable tool calls have stable retry keys. Tools must use them or otherwise
reconcile retries with their external effects. Brainstone cannot make an
arbitrary external service idempotent. See the
[recovery contract](crates/runtime/README.md#leases-recovery-and-visibility).

### Run a worker pool

For a server or queue consumer, spawn `host.run()` and clone the host into your
handlers. Acknowledge an incoming event after acceptance succeeds. Use
`Host::admit` to steer an active lane; `Host::accept` queues another operation.
At shutdown, close admission, join workers, and close the resources you opened.
The [composition guide](crates/brainstone/README.md) covers ownership and
admission decorators.

## Build your own agent

The [agent-building guide](docs/building-agents.md) also covers smaller
examples for conversation history, native tools, and retry handling.

| Example | What it teaches |
| --- | --- |
| [Coding agent](crates/brainstone/examples/coding_agent.rs) | Read, edit, test, approve, and recover in one application |
| [Chat assistant](crates/brainstone/examples/chat.rs) | A real provider, a persistent conversation, and safe message retries |
| [Order support](crates/brainstone/examples/order_support.rs) | A native Rust lookup tool with input checks and stable errors |
| [Simulated refund](crates/brainstone/examples/refund.rs) | Typed tools, durable approval, required effects, and crash recovery |
| [Crash recovery](crates/brainstone/examples/recover.rs) | Resume committed tool work after a process exits, without network access |

The chat and support examples include copyable commands in the guide. Both
use SQLite and your chosen OpenAI Responses model.

## Configured agents

The default `configured` feature provides an environment-configured preset:
providers, MCP tools, memory, skills, subagents, schedules, and approvals.
`brainstone::configured(settings).await?` builds it without schedule tools;
`brainstone::build(settings, Some(destinations)).await?` enables scheduled
posting through your surface. Call `engine.shutdown().await` to drain work
and close its resources.

See the [composition guide](crates/brainstone/README.md) and the compiled
[`compose` example](crates/brainstone/examples/compose.rs). An
[example agent directory](examples/minimal-agent) contains prompts, skills,
and MCP configuration.

## Documentation

- [Coding agent](docs/coding-agent.md)
- [SQLite evaluation results](docs/evaluations.md)
- [Typed tools and tool contracts](docs/tools.md)
- [Structured outputs](docs/structured-output.md)
- [Composition and Cargo features](crates/brainstone/README.md)
- [Operations and configuration](docs/operations.md)
- [Architecture and crate boundaries](docs/architecture.md)
- [Stable failure codes](docs/failure-codes.md)
- [MCP execution context](docs/mcp-execution-context.md)
- [Contributing](CONTRIBUTING.md) and [security](SECURITY.md)

## Development

```sh
cargo xtask gate
cargo xtask audit
```

MIT. See [LICENSE](LICENSE).
