<p align="center">
  <img src="docs/assets/logo.png" alt="Brainstone, Rust Agent Engine" width="320" height="320">
</p>

<h1 align="center">Brainstone</h1>

<p align="center">
  <a href="Cargo.toml"><img src="docs/assets/rust.svg" alt="Rust 1.95.0+" height="20"></a>
  <a href="Cargo.toml"><img src="docs/assets/edition.svg" alt="Edition 2024" height="20"></a>
</p>

A Rust crate for durable AI agents. It runs a model and its tools and stores
progress in SQLite or PostgreSQL. It resumes accepted work after a crash or
deploy. You compose it in Rust and hang your own interface (HTTP bot, CLI,
worker queue) off the `surface` traits. An agent is a directory of prompts,
tools, skills, and child agents; you don't need to write Rust to configure one.

## Why Brainstone

- **Durable work.** Accepted prompts survive process restarts. Retries return
  the same committed outcome; streamed text stays provisional until completion.
- **Model choice.** Use Anthropic, Amazon Bedrock, or OpenAI-compatible endpoints,
  including routers and local servers. The model's capabilities set the limits.
- **Steering and compaction.** Correct an active run, including during reasoning.
  Completed reasoning survives replay, and compaction bounds growing context.
- **Durable approvals.** Approvals survive restarts and bind the exact tool call,
  arguments, identity, and current policy and executor configuration.
- **Isolated MCP tools.** Run local MCP servers as child processes or reach
  remote ones over Streamable HTTP. Native tools remain part of the trusted runtime.
- **One runtime, two databases.** SQLite and PostgreSQL share behavioral contracts
  and recovery tests. Start on one host or use PostgreSQL across hosts.

External tools must honor their retry keys to avoid duplicate effects. See the
[recovery contract](crates/runtime/README.md#leases-recovery-and-visibility).

## Build an agent

An agent is a directory:

```text
my-agent/
  prompts/
    system_prompt.md   # required: valid UTF-8, non-blank
  agents/*.md          # optional child agents (name, tools, model, background)
  skills/<name>/SKILL.md   # optional Agent Skills
  mcp.toml             # optional MCP tool servers
```

Create it and write the prompt:

```sh
mkdir -p my-agent/prompts
printf 'You are a useful assistant.\n' > my-agent/prompts/system_prompt.md
```

Depend on the `brainstone` crate. It re-exports the whole engine, which the
workspace publishes as `brainstone` plus `brainstone-*` crates behind it.

```rust
use brainstone::Settings;
use std::path::Path;

// Where a schedule's runs post. Omit it and the schedule tools are not offered.
struct Destinations;

impl brainstone::surface::Destinations for Destinations {
    async fn resolve(
        &self,
        _: brainstone::surface::PostRequest<'_>,
    ) -> Result<brainstone::runtime::Destination, brainstone::agent::ToolError> {
        todo!("resolve a model-named conversation to a durable delivery target")
    }
}

#[tokio::main]
async fn main() -> Result<(), brainstone::Error> {
    let settings = Settings::from_map(&std::env::vars().collect(), Path::new("my-agent"))?;
    let mut engine = brainstone::build(settings, Option::<Destinations>::None).await?;

    // A durable prompt: surface, conversation, lane, retry key, system, text.
    let address = brainstone::runtime::Address::new("demo", "conversation-42", "main")?;
    let prompt =
        brainstone::runtime::Prompt::new(address, "request-1", "You are a useful assistant.", "Hello")?;

    // accept() stores the prompt durably; the id is the retry key.
    let id = engine.admission.accept(&prompt).await?;
    let operation = engine.host.execute(id).await?;

    engine.host.shutdown().await;
    engine.mcp.close().await;
    engine.store.close().await;
    Ok(())
}
```

`Settings::from_map` reads the agent directory from the path you give it.
`Settings::from_env` is `from_map` over the process environment with the
current working directory as the agent path. The system prompt is loaded once
at startup and must exist and be non-blank, or startup fails with
`invalid_configuration`; already-created sessions keep their stored prompt.

[`examples/minimal-agent`](examples/minimal-agent) is a complete agent
directory: prompt, MCP servers, and skills layout, with nothing to delete.
Two compiled examples live in the facade crate: `cargo run -p brainstone
--example echo` runs a durable turn against an in-process model with no
network, and `cargo run -p brainstone --example compose` composes the full
engine from settings and that directory.

For a resident agent, call `engine.host.run().await?` instead of `execute`;
it drains accepted prompts until shutdown. Feed it from your own ingress by
calling `engine.admission.accept` from your handlers.

Configuration comes from environment variables: `DATABASE_URL`, the model
(`MODEL_PROVIDER`, `ANTHROPIC_MODEL`, `ANTHROPIC_API_KEY`, …), and optional
switches. Copy [.env.example](.env.example) as the checklist. The
[operations reference](docs/operations.md) documents every variable, storage
behavior, compatible model targets, and observability.

## Layout

| Crate | Responsibility |
| --- | --- |
| [brainstone](crates/brainstone) | The composition root and public facade |
| [agent](crates/agent) | Model/tool loop and extension interfaces |
| [runtime](crates/runtime) | Durable operations, shared record codecs, PostgreSQL/SQLite storage |
| [host](crates/host) | Prompt admission, workers, shutdown |
| [anthropic](crates/anthropic), [openai](crates/openai) | Model adapters |
| [mcp](crates/mcp), [skills](crates/skills), [subagents](crates/subagents), [memory](crates/memory), [policy](crates/policy) | Agent capabilities |
| [media](crates/media) | Model-input images (validation, bounds, fitting), the stable reference a reply uses to attach a tool-returned image or file, and `CreateFile`, which lets any agent write a file for the reply |
| [sigv4](crates/sigv4) | AWS SigV4 identity and request signing shared by the Bedrock paths of both adapters |
| [surface](crates/surface) | The seam your interface implements |
| [telemetry](crates/telemetry) | OTLP traces/metrics and correlated JSON logs |

## Documentation

- [Operations reference](docs/operations.md): every variable, storage
  behavior, compatible model targets, and observability.
- [Architecture](docs/architecture.md): crate layout and runtime diagrams.
- [Failure codes](docs/failure-codes.md): the codes a surface or recovery
  branches on.
- [MCP execution context](docs/mcp-execution-context.md): what a configured
  server receives, and what `access` decides.

## Development

```sh
cargo xtask gate    # fmt --check, clippy -D warnings, tests, doctests, docs
cargo xtask audit   # machete, deny check
```

Requests and bug reports are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md)
before a change; report vulnerabilities through [SECURITY.md](SECURITY.md), not
in a public issue.

## License

MIT. See [LICENSE](LICENSE).
