# Operations reference

[Back to the README](../README.md)

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

## Known limits

- **Stored tool video.** Tool media stored while video was a supported tool
  output cannot be read back: those rows are rejected as `corrupt_record` when
  a reply names them, which is the forward-only rule for stored records. The
  rows age out after the retention window.
- **Bundled SQLite version.** The SQLite backend links the SQLite copy bundled
  by `sqlx-sqlite` 0.9.0 (3.53.2), which is affected by CVE-2026-119375: a
  database opened next to a `-wal` file whose page size differs can return
  uninitialized page-cache bytes. The patched SQLite (3.54.0) reaches this
  workspace only once SQLx permits `libsqlite3-sys` 0.39, so `deny.toml`
  records `RUSTSEC-2026-0459` as an ignore. Exploiting it needs write access to
  the database directory; keep that directory writable by the runtime user
  only.

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
copy only the main file while writers are active. Store rollback follows the
same forward-only migration rule as PostgreSQL. Switching `DATABASE_URL`
selects another database; it does not copy existing sessions between backends.

## Compatible model targets

Existing Anthropic, OpenAI, and Bedrock settings remain valid. Neutral names
`MODEL_NAME` and `MODEL_MAX_OUTPUT_TOKENS` are aliases for `ANTHROPIC_MODEL` and
`ANTHROPIC_MAX_TOKENS`. If both names are set, their values must agree.
The new `CoreSettings` accessors expose model name, target, output cap,
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

Every assistant response stores `model`, the name of the model that produced
it: `anthropic/{model}`, `openai/{model}`, or `{provider}/{region}/{model}` for
the three Bedrock providers. A subagent's pinned model id names the pinned
model. Before each request the history is replayed for the model that
receives it. The replay is never stored; records stay as written.

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
- **Unanswered tool calls:** a call with no result, which a subagent stopped
  mid-batch and then messaged again leaves, is answered in the replay with an
  `unanswered_tool_call` error result. Providers reject a request that holds an
  unanswered call.
- **Tool ids:** Messages accepts ids matching `^[a-zA-Z0-9_-]+$`. The Anthropic
  adapter sends `-` as `--`, every other rejected byte as `-` and two hex
  digits, and an empty id as `-`. The mapping is one-to-one, so every call
  stays paired with its own result; an id made only of letters, digits, and
  `_` passes unchanged.
- **Model changes:** changing `MODEL_PROVIDER`, `ANTHROPIC_MODEL`, or a pinned
  model, or setting or unsetting `FALLBACK_MODEL`, is safe at any time. The new
  model is shown earlier turns without reasoning it cannot read. A model's own
  history replays byte-identical, so its prompt cache stays valid.
- **Upgrade:** responses stored before this release carry no `model`, so their
  reasoning is no longer replayed; text and tool calls are. Before this
  release a response carried `model` only when `FALLBACK_MODEL` was set, so
  these were written by the single configured model. With `PROMPT_CACHE` off
  (the default) the Messages adapter sets no `cache_control`, so Anthropic
  caches nothing and the change costs no cache.
  The Responses API caches prefixes on its own: a lane that ran on a Responses
  model before this release misses that cache once.
- **Rollback:** this release is forward-only once a lane has failed over or
  `MODEL_PROVIDER` has changed. Older releases do not apply these rules and
  treat these records as they treat their own. A lane holding a response
  from the other wire protocol fails each turn with `invalid_request`
  (Messages) or `foreign_reasoning` (Responses) while an older binary runs.
  Records are not changed, so rolling forward again replays that lane
  correctly.

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
- **Both fail:** the turn reports the secondary's code and message. It is the
  last failure the turn hit, and a broken secondary (for example
  `authentication_failed`) must surface instead of hiding behind the primary's
  outage. The primary's code is in the failover log line.
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

The chosen model and effort are stored with the lane, so crash recovery and
scheduled runs replay them without asking the selector again, and a redelivered
request keeps the choice made at its first admission. A message that arrives
while the lane is running becomes steering and keeps the running operation's
model. The turn limit stays `MAX_TURNS`.

Brainstone does not check that the model exists or supports the effort level; the
deployment owns that. An unknown model fails with the provider's own code. On a
compatible target without `reasoning-effort` in `MODEL_CAPABILITIES`, every
selected prompt fails with `unsupported_reasoning_effort`, because the selector
always supplies an effort. With failover configured, the secondary answers with
its own `FALLBACK_MODEL` and the selected effort.

## Capabilities

**MCP** (`mcp.toml`): `[mcp_servers.<name>]` with `url` + `bearer_token_env`
or `command` + `args` + `env = { VAR = "ENV_NAME" }`, `enabled_tools`,
`required` [true]. Required servers that fail stop startup; optional ones are
skipped until restart. Tools are listed at startup and again before each
claimed operation, so the next message in any existing thread sees a tool a
server added or removed, with no backfill and no restart. A listing that fails
or takes over 5 seconds keeps the previous list and logs `MCP tool listing
failed` or `MCP tool listing timed out`; an unreachable server therefore adds
up to 5 seconds to each claim until it recovers. Rollback is safe: this release
still writes `lanes.tool_definitions` when a lane is created, and older
binaries offer that column to the model. Stop writing it only once no binary
that reads it can be deployed. Dead connections reconnect on the next call under
the startup timeout, else the call returns `mcp_connect` or
`mcp_startup_timeout`. Supports stdio, Streamable HTTP, MCP Tasks, and
elicitation (an `Interaction` implementation shows respond, decline, and
cancel controls, and its form takes one JSON object); the handshake starts with `server/discover` for
revision 2026-07-28 and falls back through older `initialize` revisions,
spending at most half of `startup_timeout_seconds` (and 10 s) on a server
that never answers discover. The
model reads a result as its JSON text. A PNG, JPEG, or WebP image block
reaches the model as an image beside that text, validated and bounded exactly
like any other attachment (5 MiB, downscaled past 1,568 px), and its place in the
text says what it was and the Markdown that attaches it to the reply (see
below). No server setting is needed. An audio block, an image that is
unsupported, corrupt, or too large, and any binary in a result the server
marked as an error are replaced by a note, `[<code>: <uri or kind> (<media type>, <n>
bytes) ...]` with code `mcp_binary_omitted`, `mcp_image_unsupported`,
`mcp_image_invalid`, or `mcp_image_too_large`, so the model knows the file
exists but never pays for its base64. Tool images count toward the replay
bound that triggers compaction like any other attachment. An embedded blob
resource whose `mimeType` is `video/*` becomes a note coded
`mcp_video_unsupported`: the reply does not carry video, so neither the model
nor the surface receives it. The stdio transport reads each JSON-RPC line whole
with no size limit, so a 50 MiB blob is a ~67 MiB line held in memory several
times over (line buffer, parsed message, decoded bytes) during the call, and
the line buffer keeps its peak capacity for the life of the connection. Any other embedded blob
resource (the GitHub server returns a fetched PDF this way) is kept for the
reply as a file named after the URI's last path segment, with its declared
`mimeType`, up to 50 MiB, and never sent to the model; the result text
carries its reference, name, media type, byte size, `"seen_by_model": false`,
and the link that attaches it. A blob that is not valid base64 or whose URI
names no file becomes a note coded `mcp_file_invalid`; one past 50 MiB,
`mcp_file_too_large`. Until this release such a blob was a
`mcp_binary_omitted` note, so the model now sees an attachment document where
it saw that note.

**Skills** (`skills/<name>/SKILL.md`, direct children only): startup exposes
names and descriptions; `Skill` loads one file, `SkillResource` reads one UTF-8
file up to 512 KiB with no traversal or symlink escape; `$skill-name` in a
prompt injects it before admission.

**Project agents** (`agents/**/*.md`, YAML frontmatter: `name`, `description`,
`tools`, `disallowedTools`, `model`, `maxTurns`, `effort`, `background`, string
`mcpServers`; other fields stop startup): each child is its own durable session,
retries reuse its identity, depth 3, 20 active per tree. Omitted `maxTurns`
inherits its parent's accepted limit (unlimited by default); an explicit positive value
or `unlimited` overrides it. Tools: `Agent`,
`SendMessage`, `TaskOutput`, `TaskStop`. `TaskOutput` waits at most
`timeout_seconds`, then returns the child's durable status; the child stays
unfinished until it completes or `TaskStop` cancels it. A wait that was
executing the child can resume unfinished work after the writer lease expires
(30 s by default), so a process with no service worker can still finish a child.
Status reads use the database's own timeouts.

**Memory**: see the `MEMORY_*` row; saves are keyed by tool-call id so a
retried call saves once; injection is part of the durable prompt, so replay
never rewrites earlier context.

**Policy**: steering interrupts an in-flight ordinary model request, including
reasoning, when the next 100 ms database probe observes it. The old partial
response is discarded and inference restarts with the durable correction.
Completed reasoning is preserved. An executing tool finishes; unstarted calls
are skipped with `tool_call_superseded` results before applying the correction.
Database contention can delay observation. Summary generation and output
validation finish at their existing safe boundaries. Provider work or billing
may continue after cancellation. Compaction estimates tokens provider-neutrally and asks the same model
for a summary at the threshold, journaled. When the history's images are past
the replay bound (10 MiB or 96 files), the summary request
carries a note per message in their place, so it stays inside the provider's
request cap. `Schedule` (exactly one of `run_at` RFC 3339,
`after_seconds`, `repeat_every_minutes`, or `repeat_time` + `repeat_time_zone`
with optional `repeat_days`, `repeat_interval`, `repeat_month_day`, and
`repeat_nth`), `ListSchedules`, `CancelSchedule`; promoted by ordinary worker
claims, so the requested time is the earliest start and a busy worker can
delay it. A standing row is re-released by the runtime after every run, never
overlaps itself, is capped at 20 per lane and 50 per owner, nudges the user
after 3 consecutive failures, and is cancelled after 14. `ListSchedules` and
`CancelSchedule` reach the schedules created in the current thread; in a DM
they also reach every schedule the requester created anywhere. Intervals are elapsed minutes from
creation, at least `SCHEDULE_MIN_INTERVAL_MINUTES`. Calendar rules keep the
local wall-clock time: every N days, weeks on listed weekdays, or months on a
day (a shorter month runs on its last day) or on the 1st to 4th or last
weekday. Every-N rules count from the first run after creation. With
`destinations` (the `surface::Destinations` implementation the composition
root supplies), each run posts a new message there instead of replying: at
most 10 such schedules per requester per hour. A failed post run is reported
in the requesting lane. `CancelSchedule` stops future runs; a run already in
progress finishes, and a steering message in its lane ends it, which the model
acts on like any other. Every operation offers the tools of the running
release, so existing sessions see new `Schedule` fields on their next message.

**Extending**: three `agent` traits. `Model` (a provider; `anthropic`,
`openai`), `Tool` (everything above is a `Tool` in one `Registry`), `Sink`
(the journal). A surface admits via `host::Admission` (`Host::admit`, then
`Runtime::admit_with_tools`) and runs a delivery worker; adding one is code.

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

Resource attributes, endpoint URLs and service names are operator configuration;
do not put patient information or secrets in identity attributes. No prompt,
completion, tool input/output, conversation content, credential, free-form
error or message body is exported by default. Logs retain safe IDs and static
event sites, not the former unrestricted tracing messages. A requested run
result and explicitly saved artifacts remain application output, not telemetry.

### Datadog and Helm wiring

The reference staging cluster uses Datadog chart `3.110.13`, Agent
`7.67.0-rc.2`, admission with `mutateUnlabelled=false`, and log collection with
`containerCollectAll=false`. Its checked-in configuration does not enable an
OTLP receiver. Existing pod labels and log annotations are useful wiring
references; their language agents and DogStatsD transport do not instrument Rust.

The cluster owner must enable the node Agent's HTTP OTLP receiver in the existing
Datadog Helm values, then render and verify the actual DaemonSet:

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

These keys are present in the [pinned chart values](https://github.com/DataDog/helm-charts/blob/datadog-3.110.13/charts/datadog/values.yaml#L521).
Ensure the Agent is running on every application node (`datadog=yes` and matching
tolerations in this cluster), receiver TCP 4318 reaches the Agent, and the
Agent's API-key secret and outbound intake access are healthy. Enable both
traces and metrics in the receiver; logs continue through container collection.
No Brainstone Datadog API/application key is needed for telemetry emission.

The consumer deployment must render the following pod template and container
environment, substituting its own service, environment, image version and actual
container name (`agent` below). Field order matters for Kubernetes expansion:

```yaml
metadata:
  labels:
    admission.datadoghq.com/enabled: "true"
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

Keep the three identity values consistent across pod labels, log service and
OTel resource. Admission is optional when configuring OTLP explicitly, but the
label is required for this cluster's admission-based features. Log collection
still requires the annotation when `containerCollectAll=false`; a chart without
pod-annotation support must add that support in its own repository.

Consumer network policy must permit TCP 4318 to the **node host IP** above;
namespace/pod selectors alone may not match host-port traffic after CNI address
translation. The network owner must verify the CNI's host-traffic policy and
restrict access to trusted application nodes. Do not make a broad internet
egress exception. Alternatively deploy an OTLP Collector behind a ClusterIP,
allow ingress/egress to its namespace/pod selector and TCP 4318 (plus DNS as
needed), and point the shared endpoint at that service. Configure Collector
export to Datadog separately. The receiver must never be publicly exposed.

Logs include W3C hexadecimal `trace_id`/`span_id` and decimal-string
`dd.trace_id`/`dd.span_id` (the trace's lower 64 bits) for Datadog correlation.
Preserve strings in log parsing; if a custom pipeline replaces reserved fields,
map these fields back before trace correlation. Validate a sampled operation
appears in APM and that its JSON log opens the same trace. Unsampled traces and
telemetry dropped during outages cannot be opened even though logs/metrics exist.

The cluster/consumer changes above are deployment prerequisites, not changes
made by the Brainstone repository. Publishing Brainstone main does not redeploy an existing
consumer: publish the base image, update the consumer's image pin, and deploy
its Helm configuration. Datadog query MCP credentials remain separate.

### Verify and roll back

Run `cargo test -p telemetry` for a real local OTLP receiver test and the
crash-resume instrumentation regressions. It verifies protobuf, resource
identity, safe log correlation, sampling independent of metrics, and a receiver
returning failures or stalling while 5,000 spans saturate the queue.
`cargo test -p mcp --test logging` verifies actual MCP
traceparent propagation without exporting request data.

After cluster and consumer rollout, check Agent status/receiver logs and APM for
`service:my-agent env:staging`, look for `brainstone.work.started`,
`brainstone.work.finished` and `brainstone.work.duration`, then check a sampled log/trace pair.
Group by `kind`/`outcome`; never promote numeric operation IDs to metric tags.
Investigate `BatchSpanProcessor.SpanDroppingStarted`, exporter-failure codes and
log-drop diagnostics before treating missing observations as missing work.
Datadog metric naming/aggregation depends on its receiver mapping; verify the
actual names before creating monitors. Brainstone does not install dashboards or alerts.

Set `OTEL_SDK_DISABLED=true` to stop export without changing execution or
journal compatibility. Removing the endpoint also disables default exporters;
remove explicit `otlp` settings too, otherwise the SDK targets localhost.
For a binary rollback, restore the previous image and its previous telemetry
settings; no database migration or trace-context record migration is needed.
The local receiver test does not verify deployed Datadog ingestion, CNI routing,
custom pipelines, indexing or consumer deployment. Those need the deployment
owner's smoke check. Keep application drain time plus ten seconds for telemetry
within the pod's termination grace period. The optional PostgreSQL gate also
passes the whole opt-in suite against an isolated PostgreSQL 16 container, covering recovery,
leases, delivery checkpoints, media, host store stalls and child resume.

Local release measurements on the development machine (10,000 fresh in-memory
operations after 100 warmups, mock model, no network latency): before this change
p50/p95 were 1.833/2.167 microseconds; with instrumentation disabled they were
1.792/2.167 microseconds. The enabled SDK pipeline with batched in-memory trace
export, metric aggregation and safe JSON logs to a nonblocking sink measured
14.708/25.792 microseconds. These isolate local observation cost, not production
model latency, Postgres latency, collector throughput or Datadog capacity.
Reproduce the first workload with `cargo test -p runtime --test observability_perf
--release -- --ignored --nocapture`; the enabled workload is `cargo test -p
telemetry --release operation_latency_observed -- --ignored --nocapture`.


### Reasoning progress events

Every provider streams `reasoning_delta` progress events beside `model_delta`,
batched by the same 512-character/100-ms policy and journaled in arrival order.
They carry only provider-visible text or summaries, never encrypted reasoning or
signatures. Settled reasoning is unchanged for replay.

| Provider | Source of `reasoning_delta` |
| --- | --- |
| Anthropic and Bedrock | `thinking_delta`; requests now send `thinking.display: "summarized"`, billed the same as `"omitted"` |
| OpenAI Responses and compatible Responses targets | `response.reasoning_summary_text.delta` and `response.reasoning_text.delta`; OpenAI sends summaries only when `MODEL_REASONING_SUMMARY=true` |
| Chat Completions targets | `reasoning`, `reasoning_content`, and `reasoning_details` text or summaries, when the server returns them |

No switch gates the event. The first Anthropic or Bedrock turn with thinking
writes one, and older binaries reject the new event variant: upgrade every
reader of a database before any writer, and do not roll a reader back after a
writer has recorded one.

## Permanent session deletion

Hosts may call `DurableStore::delete_session(&address)` after stopping admission,
execution, and delivery for that conversation. The operation is atomic on
PostgreSQL and SQLite and includes the session's subagent tree. Active leases
refuse deletion with `session_busy`; expired or abandoned work can be removed.
The same surface and conversation can then be accepted with fresh instructions.
Deletion removes runtime records, not external processes or host-owned files.
Shared memories remain. See the runtime crate's
[session deletion contract](../crates/runtime/README.md#deleting-a-durable-session).
