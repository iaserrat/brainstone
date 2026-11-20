# Operations reference

[Back to the README](../README.md)

Building Brainstone requires Rust 1.100.0 or newer. The workspace pins
Rust 1.100.0 in `rust-toolchain.toml`.

- [SQLite](#sqlite)
- [Compatible model targets](#compatible-model-targets)
- [Model provenance and replay](#model-provenance-and-replay)
- [Model failover](#model-failover)
- [Deployment-selected model and effort](#deployment-selected-model-and-effort)
- [Capabilities](#capabilities)
- [Observability](#observability)
- [Reasoning progress events](#reasoning-progress-events)
- [Permanent session deletion](#permanent-session-deletion)
- [Known limits](#known-limits)

The `chat` and `order_support` example programs have their own
[setup guide](building-agents.md#setup). They read `OPENAI_API_KEY`,
`OPENAI_MODEL`, optional `OPENAI_ENDPOINT`, `DATABASE_URL`, and `CONVERSATION_ID`.
Their default databases live under `data/`. These are example-program settings;
the configured preset has its own settings below.

## Composition and process lifecycle

The Rust composition API (`Host`, `Runtime`, `Registry`) does not read the
process environment or agent directory. Select providers, tools, and runtime
policies in Rust; see the [composition guide](../crates/brainstone/README.md).
The default `configured` Cargo feature retains the environment-based preset.
Use `default-features = false` and explicit adapter features to omit it.
Both SQLite and PostgreSQL remain compiled into the runtime.

`brainstone::configured(settings)` composes the preset without schedule tools.
Use `build(settings, Some(destinations))` when scheduled posting is required.

Call `engine.shutdown().await` from the process-owner task. It closes host
admission, waits for claimed operations, closes MCP clients, and closes the
shared database pool, including external clones. It does not cancel claimed
work. Enforce hard termination deadlines outside the engine; recovery resumes
from durable records. Join any separately spawned `host.run` task.
For manual Rust composition, shut down and join the host before closing your
own MCP clients, database, and other resources. Never call shutdown from a tool
currently executing inside that host.

The `recover`, `refund`, and coding-agent demos use 30-second leases for normal
work and five-second leases for deliberate crash runs.

## Known limits

- **Bundled SQLite version.** The SQLite backend links the SQLite copy bundled
  by `sqlx-sqlite` 0.9.0 (3.53.2), which is affected by CVE-2026-119375: a
  database opened next to a `-wal` file whose page size differs can return
  uninitialized page-cache bytes. The patched SQLite (3.54.0) reaches this
  workspace only once SQLx permits `libsqlite3-sys` 0.39, so `deny.toml`
  records `RUSTSEC-2026-0459` as an ignore. Exploiting it needs write access to
  the database directory; keep that directory writable by the runtime user
  only.
- **MCP SDK dependency.** The MCP SDK (`rmcp`) depends on `chrono`, which
  RustSec lists as unmaintained. `deny.toml` exempts this maintenance advisory
  while the SDK requires the crate. Other `chrono` advisories still apply.
  [RUSTSEC-2026-0543](https://rustsec.org/advisories/RUSTSEC-2026-0543.html)

## SQLite

Set `DATABASE_URL=sqlite://./data/runtime.db` to use SQLite.
Create the parent directory first. New Unix database files have mode `0600`.
The engine uses the same backend for every durable feature, including
steering, media, delivery, interactions, memories, schedules, and subagents.

SQLite uses WAL, `synchronous=FULL`, and foreign keys. Startup verifies these
settings and rejects in-memory databases with `durability_not_guaranteed`.
Concurrent first opens retry journal-mode contention for up to five seconds.
The schema baseline and every write use immediate transactions. One database can have
several readers and one writer; the four-connection pool waits up to five
seconds for a conflicting write before returning `store_unavailable`.
Workers use the existing retry policy. Model and tool calls run outside database
transactions. Leases and fences survive process exit.

Keep the file on persistent local storage. All processes sharing a WAL database
must run on the same host. Use PostgreSQL for workers on separate hosts.
SQLite does not add replication or automatic failover. Unicode memory filters
scan a scope in Rust and stop at the result limit.

Back up a running database with SQLite's online backup API. For a stopped copy,
stop every process first and keep the database and its WAL together. Do not
copy only the main file while writers are active. Switching `DATABASE_URL`
selects another database. Transfer sessions separately if needed.

## Compatible model targets

`MODEL_NAME` and `MODEL_MAX_OUTPUT_TOKENS` are aliases for `ANTHROPIC_MODEL`
and `ANTHROPIC_MAX_TOKENS`. When you set both names for a setting, their values
must agree.
`CoreSettings` accessors expose model name, target, output cap,
reasoning effort, and capabilities without provider-specific names.

For a custom endpoint, set `MODEL_PROVIDER=openai` and configure an exact URL:

```dotenv
DATABASE_URL=sqlite://./data/runtime.db
WORKER_ID=local-agent
MODEL_PROVIDER=openai
MODEL_NAME=local-model-id
MODEL_ENDPOINT=http://127.0.0.1:11434/v1/chat/completions
MODEL_PROTOCOL=chat-completions
MODEL_TARGET=local
MODEL_AUTH=none
MODEL_CONTEXT_WINDOW=8192
```

This configuration needs no provider credentials or Anthropic model variables.
Use a model ID installed on your server. Keep credentials in environment
variables or a secret store.

| Setting | Contract |
| --- | --- |
| `MODEL_ENDPOINT` | Exact absolute HTTP(S) endpoint, without embedded credentials |
| `MODEL_PROTOCOL` | `responses` (default) or `chat-completions` |
| `MODEL_TARGET` | Required stable target name; endpoint and protocol also contribute to its stored replay identity |
| `MODEL_AUTH` | `bearer` (default), using `OPENAI_API_KEY`; `none` omits built-in authentication and rejects a simultaneous key |
| `MODEL_CAPABILITIES` | Comma-separated `text`, `tools`, `images`, `tool-images`, `reasoning-effort`, `streaming`; defaults to `tools,streaming`, with text always supported |
| `MODEL_HEADERS` | JSON object of string HTTP headers; values stay out of debug output |
| `MODEL_PARAMETERS` | JSON object for provider options such as routing, `temperature`, or `max_completion_tokens`; reserved conversation fields are rejected, and duplicate adapter-owned fields fail with `invalid_request` |
| `MODEL_IDLE_TIMEOUT_SECONDS` | Positive timeout between response bytes (120 by default); increase it for local cold starts |
| `MODEL_MAX_OUTPUT_TOKENS` | Optional cap; Responses sends `max_output_tokens`, Chat Completions sends `max_tokens` |
| `MODEL_EFFORT` | Optional on custom targets; requires `reasoning-effort` in the capability list |
| `MODEL_REASONING_SUMMARY` | `true` adds `reasoning.summary: "auto"` to Responses requests that set an effort, so reasoning summaries stream as `reasoning_delta` [`false`]; rejected on `chat-completions`. OpenAI rejects every such request with `invalid_request` for an organization that is not verified |
| `MODEL_CONTEXT_WINDOW` | Positive declared capacity; compaction uses the smaller configured budget across primary and fallback targets |

For models requiring `max_completion_tokens`, omit `MODEL_MAX_OUTPUT_TOKENS`
and use `MODEL_PARAMETERS={"max_completion_tokens":4096}`. Provider-specific
reasoning options can use the same parameter object. Brainstone keeps full local
history and does not permit those options to enable remote conversation state.

Use `MODEL_CAPABILITIES=text` for a model without tool calling or streaming.
The application then offers no tools. Missing input or tool-result modalities
fail with stable `unsupported_*` codes before network I/O; content is never
silently dropped. Each model must still return a complete response through its
selected protocol. A compatible URL cannot add tool calling or vision to a
model that lacks them.

All target settings also accept the `FALLBACK_` prefix, including
`FALLBACK_MODEL_ENDPOINT`, `FALLBACK_MODEL_PROTOCOL`, and
`FALLBACK_MODEL_CAPABILITIES`. Its model ID remains `FALLBACK_MODEL`; its neutral
cap is `FALLBACK_MAX_OUTPUT_TOKENS`. Custom targets omit effort and output limits
unless supplied for that target. Failover checks the secondary's capabilities
before sending input. An unsupported primary can use a compatible secondary;
an incompatible secondary leaves the primary failure intact.

Reasoning belongs to its exact target and model. Responses items and Chat
Completions `reasoning`, `reasoning_content`, and `reasoning_details` fields
survive storage and replay. Provider or protocol changes cannot reuse another
target's opaque state. Routers must preserve continuation compatibility across
upstream routes, or operators must pin routing with provider parameters.

## Model provenance and replay

Every assistant response stores its producing `model`: `anthropic/{model}`,
`openai/{model}`, or `{provider}/{region}/{model}` for the three Bedrock
providers. A subagent's pinned model ID identifies that model. Before each
request, the adapter prepares history for the receiving model. It does not
store this replay or change the original records.

- **Reasoning:** only responses stored under the receiving model's name keep
  their reasoning blocks (signed thinking, redacted thinking, opaque items).
  Text and tool calls of every response stay. A response stored without
  `model` keeps no reasoning for any model. Reasoning is readable only by the
  model, and for some models only the account, that produced it. Brainstone requests
  adaptive thinking, which accepts an assistant turn without a thinking block,
  inside a tool loop too.
- **Failed responses:** a response that ended in a provider error is not
  replayed. A refusal can stop mid-stream with partial text, reasoning, or tool
  calls that no result answers.
- **Unanswered tool calls:** replay supplies an `unanswered_tool_call` error
  for calls without results. This can happen when a subagent stops mid-batch
  and later receives a new message. Providers reject requests with unanswered
  calls.
- **Tool ids:** Messages accepts ids matching `^[a-zA-Z0-9_-]+$`. The Anthropic
  adapter sends `-` as `--`, every other rejected byte as `-` and two hex
  digits, and an empty id as `-`. The mapping is one-to-one, so every call
  stays paired with its own result; an id made only of letters, digits, and
  `_` passes unchanged.
- **Model changes:** changing `MODEL_PROVIDER`, `ANTHROPIC_MODEL`, or a pinned
  model, or setting or unsetting `FALLBACK_MODEL`, is safe at any time. The new
  model is shown earlier turns without reasoning it cannot read. A model's own
  history replays byte-identical, so its prompt cache stays valid.

## Model failover

Set `FALLBACK_MODEL` to give every agent, subagent, and compaction summary a
secondary model. Without it every request goes to the primary.

- **Trigger:** the primary's adapter has spent its own retries (Anthropic HTTP:
  2 retries, 250 ms then 500 ms, or `retry-after`; Bedrock runtime: AWS SDK
  backoff; Responses: 3 retries, 250 ms, 500 ms, 1 s, or `retry-after`, on
  HTTP statuses only) and returns `provider_unavailable`, `rate_limited`,
  `request_timeout`, or `transport_error`. The same request then goes to the
  secondary once. Every other code goes up unchanged and the secondary is
  never called: `context_window_exceeded`, `invalid_request`,
  `authentication_failed`, `permission_denied`, `billing_error`, and the rest.
- **Both fail:** the turn reports the secondary's code and message, including
  configuration failures such as `authentication_failed`. The failover log
  retains the primary's code.
- **No circuit breaker:** every request tries the primary first, so during a
  primary outage each request waits out the primary's retries (about 1 s on
  Anthropic HTTP, about 2 s on Responses) before it fails over.
- **History:** each model is shown the other's responses without their
  reasoning, as [Model provenance and replay](#model-provenance-and-replay)
  describes. After a failover inside a tool-use turn, the secondary continues
  that turn without the primary's reasoning.
- **Across protocols:** any provider can fail over to any other. Each adapter
  replays the other's turns in its own wire format: text, tool calls under
  their original ids, and tool results with their images. The
  system prompt goes to Anthropic as `system` and to Responses as
  `instructions`.
- **GPT OSS:** not supported. AWS serves it with the Responses API only on
  `bedrock-mantle`, which rejects tool results that carry images, and the
  model reads text only; Bedrock does not offer it in `ca-central-1`.
- **Pinned models:** a subagent whose definition pins a model id names a
  primary model. The secondary ignores the pin and answers with
  `FALLBACK_MODEL`.
- **Log:** each failover emits one `WARN` line, once the secondary has
  answered, with message `model failover` and fields `primary` (name),
  `primary_code`, and `model` (the secondary's name). Count failovers by that
  message.
- **Per-model settings:** `FALLBACK_MAX_TOKENS` and `FALLBACK_MODEL_EFFORT`
  set the secondary's output cap and effort; unset, it uses
  `ANTHROPIC_MAX_TOKENS` and `MODEL_EFFORT`. Compaction uses one
  `MODEL_CONTEXT_WINDOW` for both: set it to the smaller window of the two.
- **Startup:** `invalid_configuration` when a `FALLBACK_*` variable is set
  without `FALLBACK_MODEL`, when the secondary's provider settings are missing
  or invalid (the primary's rules, under the `FALLBACK_` names), or when the
  secondary's name equals the primary's: two keys for the same provider,
  region, and model cannot be told apart in stored history.
- **Credentials:** a Bedrock secondary without `FALLBACK_ANTHROPIC_API_KEY`
  signs with the process's AWS credential chain (IRSA included), in
  `FALLBACK_AWS_REGION`, which may differ from the primary's region.
- **Live progress:** text the primary streamed before it failed stays in the
  provisional `ModelDelta` events, followed by the secondary's. Only the
  settled response is authoritative.

## Deployment-selected model and effort

By default every prompt uses `ANTHROPIC_MODEL` and `MODEL_EFFORT`. An optional
private-stdio selector lets the deployment pick both for each prompt instead.
Set both variables: `DYNAMIC_MODEL_AND_EFFORT_COMMAND` (absolute executable
path) and `DYNAMIC_MODEL_AND_EFFORT_ARGS` (JSON string array). Setting one
without the other fails startup with `invalid_configuration`. No selector runs
unless configured. The executable is trusted operator configuration, inherits
the service environment, and must not be model-controlled.

The selector runs at admission for every surface, after
the actor allowlist and quota and before durable acceptance. Brainstone writes one
JSON object to its stdin and closes it: `surface`, `conversation`, `lane`,
`actor` (or `null`), and `text`, the user's text before memory or skill
expansion. The first stdout line must be exactly
`{"model": "<id>", "effort": "low|medium|high|xhigh|max"}`, within ten seconds
and 16 KiB; input is limited to 64 KiB. Any other answer, a timeout, or a
failed spawn rejects the prompt with `model_selection_failed`; nothing is
stored.

The lane stores the chosen model and effort. Recovery, scheduled runs, and
redelivered requests reuse that choice without calling the selector. Input
that arrives while the lane is running becomes steering and keeps the active
operation's model. The turn limit stays `MAX_TURNS`.

The deployment must check that the selected model exists and supports the
effort level. An unknown model fails with the provider's own code. On a
compatible target without `reasoning-effort` in `MODEL_CAPABILITIES`, every
selected prompt fails with `unsupported_reasoning_effort`, because the selector
always supplies an effort. With failover configured, the secondary answers with
its own `FALLBACK_MODEL` and the selected effort.

## Capabilities

### MCP

Configure `[mcp_servers.<name>]` in `mcp.toml`. Use `url` with
`bearer_token_env`, or `command`, `args`, and `env = { VAR = "ENV_NAME" }`.
Both support `enabled_tools` and `required` (default `true`). A failed required
server stops startup; a failed optional server stays unavailable until restart.

The runtime lists tools at startup and before each claimed operation. Existing
threads see added or removed tools on their next message, without a restart or
backfill. A failed listing keeps the previous list and logs `MCP tool listing
failed`. A listing over 5 seconds logs `MCP tool listing timed out`. An
unreachable server can therefore add 5 seconds to every claim until it recovers.

Dead connections reconnect on the next call within the startup timeout.
Failures return `mcp_connect` or `mcp_startup_timeout`.

Supported transports are stdio and Streamable HTTP. MCP Tasks and elicitation
are supported. An `Interaction` implementation displays respond, decline, and
cancel controls; forms return one JSON object. The handshake starts with
`server/discover` for revision 2026-07-28, then falls back through older
`initialize` revisions. A server that ignores discover gets at most half of
`startup_timeout_seconds`, capped at 10 s, before fallback.

The model receives result text as JSON. Successful PNG, JPEG, and WebP blocks
also reach it as images. The same attachment rules apply: 5 MiB maximum and
scaling down past a 1,568 px long edge. The text includes a description and
attachment Markdown. No server setting is needed. Tool images count toward
the replay bound used to trigger compaction.

Audio, unsupported, corrupt, or oversized images, and all binaries in error
results become notes. Their format is `[<code>: <uri or kind> (<media type>, <n>
bytes) ...]`. Codes are `mcp_binary_omitted`, `mcp_image_unsupported`,
`mcp_image_invalid`, or `mcp_image_too_large`. The model sees the file's
existence without receiving its base64 contents. A `video/*` embedded blob
becomes `mcp_video_unsupported`; neither model nor surface receives the video.

Other embedded blobs, such as PDFs returned by the GitHub server, become reply
attachments of up to 50 MiB. They never go to the model. The filename comes
from the URI's last path segment; the declared `mimeType` supplies its type.
Result text carries the reference, name, type, byte size,
`"seen_by_model": false`, and attachment link. Invalid base64 or a URI with no
filename returns a `mcp_file_invalid` note. Oversized blobs return
`mcp_file_too_large`.

The stdio transport reads each JSON-RPC line whole, without a size limit. A
50 MiB blob makes a roughly 67 MiB line. The call holds several copies: line
buffer, parsed message, and decoded bytes. The buffer retains its peak
capacity until the connection closes.

### Skills

Startup exposes the names and descriptions from direct child files at
`skills/<name>/SKILL.md`. `Skill` loads one instruction file. `SkillResource`
reads one UTF-8 resource up to 512 KiB, with no path traversal or symlink
escape. A `$skill-name` mention injects its instructions before admission.

### Project agents

Definitions live in `agents/**/*.md`. YAML frontmatter accepts `name`,
`description`, `tools`, `disallowedTools`, `model`, `maxTurns`, `effort`,
`background`, and string `mcpServers`. Other fields stop startup.

Each child has a durable session, and retries reuse its identity. Maximum
depth is 3, with 20 active children per tree. Omitted `maxTurns` inherits the
parent's accepted limit, unlimited by default. A positive integer or
`unlimited` overrides it.

The controls are `Agent`, `SendMessage`, `TaskOutput`, and `TaskStop`.
`TaskOutput` waits at most `timeout_seconds`, then returns the durable status.
The child remains unfinished until it completes or `TaskStop` cancels it. A
wait that was executing the child can resume it after its writer lease expires
(30 s by default). This lets a process without a service worker finish child
work. Status reads use database timeouts.

### Memory

The `MEMORY_*` settings control memory. Saves use the tool-call ID to deduplicate
retries. Injection becomes part of the durable prompt, so recovery never
rewrites earlier context.

### Policy

Steering interrupts ordinary inference, including reasoning, when the next
100 ms database probe sees pending input. The runtime discards the partial
response and restarts inference with the durable correction. Completed
reasoning stays unchanged. Running tools finish; unstarted calls receive
`tool_call_superseded` before the correction applies. Database contention can
delay detection. Summaries and output validation finish at their safe
boundaries. Providers may continue work or billing after cancellation.

Compaction uses a provider-neutral token estimate. At the threshold, it asks
the same model for a summary and journals it. Beyond the image replay bound
(10 MiB or 96 files), summary requests replace images with a note per message
to stay within the provider request cap.

`Schedule` takes exactly one of `run_at` (RFC 3339), `after_seconds`,
`repeat_every_minutes`, or `repeat_time` with `repeat_time_zone`. Calendar
options are `repeat_days`, `repeat_interval`, `repeat_month_day`, and
`repeat_nth`. `ListSchedules` and `CancelSchedule` manage those schedules.
Worker claims release due work, so requested times are earliest starts; busy
workers can delay them.

Standing schedules never overlap themselves. Limits are 20 per lane and 50 per
owner. Failures trigger a nudge after 3 consecutive failures and cancellation
after 14. Listing and cancellation reach schedules from the current thread;
in a DM, they also reach all schedules created by that requester.

Intervals count elapsed minutes from creation and must meet
`SCHEDULE_MIN_INTERVAL_MINUTES`. Calendar rules keep local wall-clock time:
every N days, weeks on selected weekdays, or months on a date or the 1st to 4th
or last weekday. Dates beyond a month's end use its last day. Every-N rules
count from the first run after creation.

With a `surface::Destinations` implementation, each run can post a new message
elsewhere instead of replying. The limit is 10 posting schedules per requester
per hour. Failed post runs report in the requesting lane. `CancelSchedule`
stops future runs; active runs finish. Steering can ask an active run to stop,
subject to the model's decision. Existing sessions see new `Schedule` fields
on their next message because operations use the running release's tools.

### Extending

Implement `Model` for a provider, `Tool` for a capability in a `Registry`, and
`Sink` for journal output. The provider adapters are `anthropic` and `openai`.
A surface uses `host::Admission` (`Host::admit`, then
`Runtime::admit_with_tools`) and runs a delivery worker. Adding a surface
requires application code.

## Observability

The composition root installs platform observability once at startup.
Normal operation, model, tool, MCP, subagent, recovery, compaction and delivery
work needs no application instrumentation. Traces and metrics use standard
OpenTelemetry OTLP HTTP/protobuf; safe correlated JSON logs use stderr.
The [telemetry contract](../crates/telemetry/README.md) defines boundaries,
outcomes, privacy, bounded queues, failure diagnostics and shutdown semantics.

Export is off until an OTLP endpoint or an explicit `otlp` exporter is configured.
`DD_TRACE_ENABLED`, `DD_AGENT_HOST`, and Datadog admission alone do not enable
Rust instrumentation. A DogStatsD socket is not an OTLP receiver.

| Setting | Behavior |
|---|---|
| `OTEL_SERVICE_NAME` | Service identity, overriding `service.name` in resource attributes; set explicitly per deployment |
| `OTEL_RESOURCE_ATTRIBUTES` | Comma-separated operator-owned resource attributes, including `deployment.environment.name` and `service.version` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | Receiver base URL; SDK appends `/v1/traces` and `/v1/metrics` |
| `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` / `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT` | Full signal URLs; override the shared endpoint for that signal |
| `OTEL_EXPORTER_OTLP_PROTOCOL` and signal-specific protocol variants | Only `http/protobuf` is supported; other values fail configuration |
| `OTEL_TRACES_EXPORTER` / `OTEL_METRICS_EXPORTER` | `otlp` or `none`; default both enabled when an endpoint is set. Explicit `otlp` without an endpoint uses the SDK localhost default |
| `OTEL_SDK_DISABLED=true` | Disable trace/metric export; safe stderr logs remain |
| `OTEL_TRACES_SAMPLER` | `always_on`, `always_off`, `parentbased_always_on`, `parentbased_always_off`, `traceidratio`, `parentbased_traceidratio`; default parent-based always-on |
| `OTEL_TRACES_SAMPLER_ARG` | Required finite ratio in inclusive `[0,1]` for either ratio sampler |
| `OTEL_METRIC_EXPORT_INTERVAL` | SDK periodic metric interval in milliseconds, default 60,000 |
| `OTEL_EXPORTER_OTLP_METRICS_TEMPORALITY_PREFERENCE` | SDK aggregation temporality; use `delta` for Datadog |
| `OTEL_EXPORTER_OTLP_HEADERS` and signal-specific headers | Receiver authentication from secret-managed environment; never printed by Brainstone |
| `RUST_LOG` | Filters stderr logs only, default `info`; `warn`/`off` do not disable traces or metrics |

The platform fixes request timeout at three seconds, span queue size at 1,024,
and batch size at 128. Generic SDK timeout/queue variables cannot enlarge these
bounds. OTLP exporter retries are disabled; configure retry, batching, memory
limits and authentication at the local Agent/Collector. Plain HTTP is suitable
only on trusted local networks; HTTPS uses the system trust store. This build
does not implement OTLP/gRPC, SDK OTLP log export or custom client-certificate
configuration. Use a local Collector when those are required.

Keep sensitive data out of resource attributes and service names. Default
telemetry excludes prompt and completion text, tool inputs and outputs,
conversation content, credentials, and free-form error messages.

### Datadog and Helm wiring

Enable the Datadog Agent's HTTP OTLP receiver for traces and metrics. For a
node Agent using a host port, render the Helm chart and verify the Agent runs
on every application node and listens on port 4318.

```yaml
datadog:
  otlp:
    receiver:
      protocols:
        http:
          enabled: true
          endpoint: "0.0.0.0:4318"
          useHostPort: true
```

Set the exporter endpoint in your application's pod template. Define
`OTEL_AGENT_HOST` before the endpoint that references it. Replace the service,
environment, version, and container name to suit your deployment.

```yaml
metadata:
  labels:
    tags.datadoghq.com/service: "my-agent"
    tags.datadoghq.com/env: "staging"
    tags.datadoghq.com/version: "IMAGE_VERSION"
  annotations:
    ad.datadoghq.com/agent.logs: '[{"service":"my-agent","source":"rust"}]'
spec:
  terminationGracePeriodSeconds: 60
  containers:
    - name: agent
      env:
        - name: OTEL_AGENT_HOST
          valueFrom:
            fieldRef:
              fieldPath: status.hostIP
        - name: OTEL_EXPORTER_OTLP_ENDPOINT
          value: "http://$(OTEL_AGENT_HOST):4318"
        - name: OTEL_EXPORTER_OTLP_PROTOCOL
          value: "http/protobuf"
        - name: OTEL_SERVICE_NAME
          value: "my-agent"
        - name: OTEL_RESOURCE_ATTRIBUTES
          value: "deployment.environment.name=staging,service.version=IMAGE_VERSION"
        - name: OTEL_TRACES_SAMPLER
          value: "parentbased_traceidratio"
        - name: OTEL_TRACES_SAMPLER_ARG
          value: "0.1"
        - name: OTEL_EXPORTER_OTLP_METRICS_TEMPORALITY_PREFERENCE
          value: "delta"
```

Match the pod labels, log service, and OpenTelemetry resource values.
Enable Datadog container log collection if you want correlated logs.

Allow application traffic to the node host IP on TCP 4318. Restrict the
receiver to trusted clients. If you use a Collector behind a Kubernetes
Service, point the exporter at that service and configure its Datadog export.

Logs include W3C hexadecimal `trace_id`/`span_id` and decimal-string
`dd.trace_id`/`dd.span_id` (the trace's lower 64 bits) for Datadog correlation.
Preserve strings in log parsing; if a custom pipeline replaces reserved fields,
map these fields back before trace correlation. Validate a sampled operation
appears in APM and that its JSON log opens the same trace. Unsampled traces and
telemetry dropped during outages cannot be opened even though logs/metrics exist.

### Verify telemetry

Run `cargo test -p brainstone-telemetry` to test a local OTLP receiver and
crash recovery instrumentation. Run `cargo test -p brainstone-mcp --test logging`
to verify MCP trace propagation without exporting request data.

After deployment, check the receiver logs and look for
`brainstone.work.started`, `brainstone.work.finished`, and
`brainstone.work.duration`. Verify that a sampled log opens the corresponding
trace. Use `kind` and `outcome` for metric grouping. Keep operation IDs out of
metric tags.

Set `OTEL_SDK_DISABLED=true` to stop export. Allow up to ten seconds for
telemetry shutdown after the application drains.

### Reasoning progress events

Providers stream `reasoning_delta` progress beside `model_delta`. Both use
the same 512-character/100-ms batching policy and enter the journal in
arrival order.
They carry only provider-visible text or summaries, never encrypted reasoning or
signatures. Settled reasoning is unchanged for replay.

| Provider | Source of `reasoning_delta` |
| --- | --- |
| Anthropic and Bedrock | `thinking_delta`; requests send `thinking.display: "summarized"`, billed the same as `"omitted"` |
| OpenAI Responses and compatible Responses targets | `response.reasoning_summary_text.delta` and `response.reasoning_text.delta`; OpenAI sends summaries only when `MODEL_REASONING_SUMMARY=true` |
| Chat Completions targets | `reasoning`, `reasoning_content`, and `reasoning_details` text or summaries, when the server returns them |

## Permanent session deletion

Hosts may call `DurableStore::delete_session(&address)` after stopping admission,
execution, and delivery for that conversation. The operation is atomic on
PostgreSQL and SQLite and includes the session's subagent tree. Active leases
refuse deletion with `session_busy`; expired or abandoned work can be removed.
The same surface and conversation can then be accepted with fresh instructions.
Deletion removes runtime records, not external processes or host-owned files.
Shared memories remain. See the runtime crate's
[session deletion contract](../crates/runtime/README.md#deleting-a-durable-session).

## Structured output contracts

Rust callers can attach an `OutputSchema` to an accepted execution profile.
The schema survives recovery and validates the final answer before successful
settlement. See the [structured-output guide](structured-output.md) for native
provider fields, error codes, and schema restrictions.
Compaction summaries do not inherit the final-answer schema.

Local comparison runs and their budget controls are documented in the
[SQLite evaluation suite](../evals/sqlite/README.md). Live runs read the ignored
`.env` privately; they do not alter normal deployment credential loading.

## Deploying accepted tool contracts

Rust applications can attach a `ToolContract` to an execution profile to restrict
tools, require durable approvals, and require successful calls before completion.
See [typed tools and tool contracts](tools.md). Operator `ToolPolicy` remains a
separate restriction and can deny an accepted call.

Keep explicitly named tools available until their accepted work settles. A
missing capability ends the operation with `tool_contract_unavailable` before
inference. A denial or tool failure cannot satisfy a completion requirement.
Require calls only for workflows that need those actions. A general chat
agent should not require a write on every prompt.

Built-in stores restore successful-call evidence across compaction and
recovery. Custom stores must restore it through `Thread::with_tool_successes`
independently of the response sequence.
