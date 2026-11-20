# mcp

Model Context Protocol client for Brainstone, built on the official Rust SDK
(`rmcp`, re-exported). It loads `mcp.toml`, connects every enabled server at
startup, and exposes each remote tool through the existing `agent::Tool`
interface.

## Wiring

```rust
use std::path::Path;
let config = mcp::Config::load(Path::new("."), &environment)?;
let (mut clients, registry) = mcp::ClientSet::connect(&config).await?;
// ... hand `registry` to the agent loop ...
clients.close().await;
# Ok::<(), mcp::Error>(())
```

`connect` declines every elicitation. `connect_with_interactions` takes an
`Interaction` implementation; the composition root passes the durable
form and URL elicitation. `ClientSet::client(name)` hands back the `Client`; its async `peer()` exposes
the raw SDK peer (resources, prompts, tasks, custom methods) for callers that
need more than tools.

## Configuration

`mcp.toml` in the working directory; a missing file means no servers. Unknown
keys are rejected. Each server uses exactly one transport. `required`,
`enabled`, both timeouts, and `enabled_tools` apply to either; the other keys
are transport-specific, and mixing them fails startup:

```toml
[mcp_servers.remote]                 # Streamable HTTP
url = "https://mcp.example.com/mcp"
bearer_token_env = "MCP_REMOTE_TOKEN" # env var name, never the secret
headers = { "X-Static" = "value" }
env_headers = { "X-Secret" = "MCP_SECRET" }
enabled_tools = ["search", "read"]   # omit to expose every tool

[mcp_servers.local]                  # stdio child
command = "example-mcp"
args = ["--stdio"]
env = { API_TOKEN = "MCP_LOCAL_TOKEN" }
cwd = "servers/local"                # relative to the working directory
required = false                     # default true
startup_timeout_seconds = 5          # default 10
tool_timeout_seconds = 30            # default 60
enabled = true                       # default true; false skips the server
```

Every `*_env` value and `env` entry names an environment variable that must be
present and non-blank. A stdio child starts with a cleared environment: only
`HOME`, `LANG`, `PATH`, `TMPDIR`, `USER`, and the declared `env` entries reach
it. OAuth is not supported.

## Contract

- A required server that fails to connect or list tools at startup stops
  startup with the first failure. An optional server is logged and contributes no tools until
  the process restarts.
- Tools are named `mcp__<server>__<tool>`. They are listed at startup and
  again, for every server at once, before each claimed operation, so a tool a
  server adds or removes reaches the next message in every thread. A listing
  reconnects a dead transport unless a call is in flight. A listing that fails
  or takes over 5 seconds logs a warning and keeps the previous list.
  `tools/list_changed` is not needed and not observed.
- The handshake offers the newest protocol revision the SDK supports and
  walks down one revision at a time, each on a fresh connection. A revision
  without `initialize` (2026-07-28 on) starts with `server/discover`; MRTR
  elicitation needs it, since a server answering `initialize` picks an older
  revision. A server that rejects discover, or does not answer it within half
  the startup timeout (at most 10 s), gets the `initialize` revisions next,
  and a server that accepts `initialize` but rejects the session it issued
  gets the next older one. The startup timeout bounds the whole walk.
  `Client::connect_stream` has one stream, so it probes discover and falls
  back to `initialize` on that same stream.
- Calls to one server run one at a time. Queue wait is independently bounded
  by `tool_timeout_seconds`; `mcp_queue_timeout` means the call was never sent.
  A call that finds its transport dead
  reconnects first, under the startup timeout; a failed reconnect is returned
  as the tool result. The next call retries the connection. An explicitly
  closed client stays closed (`mcp_closed`).
- Every call carries the runtime's tool-call identity as
  `_meta["io.brainstone/idempotencyKey"]`. A server with external effects must honour
  it: the runtime retries calls after a crash.
- A PNG, JPEG, or WebP image block in a successful result reaches the model
  as an image, validated, bounded, and fitted by the `media` crate exactly as
  a surface's image attachment is. In the result text the block becomes its
  `media::attachment` document, which carries the reference and the Markdown
  that shows the image in the reply. Each image block gets its own reference.
- An embedded blob resource whose `mimeType` is `video/*` becomes a note
  opening with `mcp_video_unsupported`, ending `and cannot be shown in the
  reply`. The reply does not carry video, so nothing is kept for it.
- Any other embedded blob resource (a fetched PDF, a spreadsheet, an archive)
  is kept whole for the reply as an `agent::File` in `ToolOutput::files` and
  never reaches the model. Its name is the URI's last path segment, without
  query or fragment, percent-decoded when that yields UTF-8; its media type is
  the declared `mimeType` (`application/octet-stream` when absent). The block
  becomes its `media::file_attachment` document plus `"seen_by_model": false`.
  A blob that is not valid base64, or whose URI or type names no file (a URI
  ending in `/`), becomes a note opening with `mcp_file_invalid`; one past
  `media::MAX_FILE_BYTES` (50 MiB) opens with `mcp_file_too_large`. Both end
  `and cannot be attached to the reply`.
- Audio, unsupported, corrupt, or oversized images, and every binary in an
  error result become a note opening with
  `mcp_binary_omitted`, `mcp_image_unsupported`, `mcp_image_invalid`, or
  `mcp_image_too_large`. No server configuration turns this on or off: an
  image the model cannot take is always a note.
- Tool results that the server marks as an error, that fail to encode, or that
  arrive after `tool_timeout_seconds` become typed failures for the model. The
  execution deadline pauses while an elicitation is outstanding, so a call
  parked on an elicitation form can outlive it. Queue wait does not pause.
- A timeout attempts protocol request/task cancellation within 250 ms and
  invalidates the connection. Dropping an in-flight call also invalidates it.
  Before the next call, the old service gets up to five seconds to close,
  followed by the configured startup budget for reconnection. A failed cleanup
  or reconnection is a typed failure. Other server sessions are unaffected.
  This adapter never replays the timed-out call. Cancellation is best effort:
  a remote write may already have happened and must not be blindly retried.
- Operational logs include server, tool, operation ID, MCP request ID, queue
  wait, execution/total elapsed time, and outcome code. The adapter logs no
  arguments, results, credential values, or free-text error bodies.
- MCP Tasks are supported and awaited to a terminal state. Elicitation is
  answered through `Interaction`; sampling and roots are rejected as input
  requests, and no roots capability is offered.

## Stable failure codes

All codes are prefixed `mcp_` and listed in
[`docs/failure-codes.md`](../../docs/failure-codes.md). Startup failures are
`mcp_configuration`, `mcp_list_tools`, and `mcp_invalid_tool_name`.
`mcp_connect` and `mcp_startup_timeout` are startup-fatal and are also returned
as a tool result when a reconnect fails. `mcp_shutdown` also reports cleanup
failure before reconnection. Everything else is a per-call tool result.

## Testing

`cargo test -p brainstone-mcp` uses an in-process fake server and a local HTTP
server without contacting external services. `tests/fixtures/mcp.toml`
defines the configuration contract.

## Trusted execution context

Use `access = { authenticated_actor = true, private_conversation = true }` on a server
to enforce identity requirements at execution. Both transports carry runtime-owned
`io.brainstone/executionContext` metadata alongside `io.brainstone/idempotencyKey`. See the
[versioned consumer contract](../../docs/mcp-execution-context.md) for the JSON
shape, authentication boundary, stable errors, and downstream ownership checks.
