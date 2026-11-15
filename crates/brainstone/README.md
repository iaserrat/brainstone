# brainstone

The composition root for the durable agent. Parse [`Settings`], compose an
[`Engine`] with [`build`], and drive it from your own binary. There is no CLI
and no bundled interface; the `surface` crate's traits are the seam for one.

```rust
let settings = Settings::from_env()?;
let engine = brainstone::build(settings, destinations).await?;
// Accept a prompt durably; the id is the retry/idempotency key.
let id = engine.admission.accept(&prompt).await?;
// Either drive it in this process:
let operation = engine.host.execute(id).await?;
// ...or leave it for the durable worker loop:
engine.host.run().await?;
engine.host.shutdown().await;
engine.mcp.close().await;
engine.store.close().await;
```

`build` connects the selected durable database, applies the schema baseline,
builds the configured model and common capabilities, connects the MCP servers,
and returns
the durable host with the admission chain (gates, memory injection, skills,
model selection). The optional `destinations` argument decides where a
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

The process does not bind a port or run an HTTP server. Stopping admission and
draining the current durable boundary are the host's `shutdown`, followed by
`mcp.close` and `store.close`.

The public API is this crate's `lib.rs`: `Settings`, `Engine`, `build`, plus
re-exports of every underlying crate (`agent`, `runtime`, `host`, `mcp`,
`surface`, and the rest), so a consumer depends on this one crate. The
`examples/` directory holds two compiled starting points: `echo` runs a
durable turn against an in-process model with no network, and `compose`
drives the full engine from settings and an agent directory.