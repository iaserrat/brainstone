# telemetry

Process-wide telemetry for Brainstone, independent of any vendor. The
`brainstone` crate initializes it automatically. Custom applications call
`telemetry::init()` once before starting workers and keep the `Telemetry`
guard alive. Drain application work before awaiting `guard.shutdown()`.

Do not install another tracing subscriber. `brainstone::init_tracing()`
initializes logging; `brainstone::init_observability()` initializes
the telemetry described here.

The OpenTelemetry SDK exports traces and metrics over OTLP HTTP/protobuf.
JSON logs go to stderr for the platform's container log collector. No Datadog
client, API key, backend name, prompt, or message enters execution instrumentation.
See [operations](../../docs/operations.md#observability) for configuration and
Datadog/Helm wiring.

## Instrumentation contract

`agent::observation` owns fixed execution boundary names and observations of real
results. Its reserved tracing target is `brainstone::telemetry`, span name `work`.
The telemetry crate manages export, privacy filtering, aggregation, and
worker lifetime. This diagnostic data does not become a durable record or
guide recovery.

| Boundary (`kind`) | Meaning |
|---|---|
| `operation` | One claimed execution attempt, through authoritative terminal-state reconciliation |
| `model` | One logical invocation, including streaming, provider retries and fallback; compaction calls also use this boundary |
| `provider` | One HTTP send through response headers, or one Bedrock SDK invocation; response streaming belongs to `model` |
| `tool` | One actual registry execution, including unknown-tool refusal; its result precedes durable journaling |
| `mcp.call` | One MCP invocation, including queue admission and connection/task handling |
| `mcp.queue` | Waiting for the server's exclusive call lock; a queue timeout sends no call |
| `subagent` | Durable spawn/reopen; parent and child operation IDs map the tree across separate worker traces |
| `delivery` | A claimed durable delivery batch through checkpoint; empty polls do not count |
| `compaction` | Summary generation and durable replacement of a lane prefix |
| `recovery` | Reconstruction of a thread from stored records on claim, including first claims |

`outcome` is `ok`, `failed`, `retry`, `cancelled`, `lease_lost`, or `abandoned`.
A successful Rust result that contains a model or tool failure records
`failed`. An operation records `ok` only after durable completion, and
terminal `agent_cancelled` records `cancelled`. Dropping execution without a
result records `abandoned`; it does not imply cancellation. A process crash
emits no final close event, log, or metric. Recovery starts a new attempt. Its operation ID and session fencing token
correlate it with earlier attempts. Operation spans call the token `attempt`;
it is not an operation retry count. No trace context is
added to durable records. A completed tool effect and a failed later journal
write remain separate observations.

Provider `retry` means the existing HTTP retry loop chose another send; the
next send is another provider span. Bedrock SDK-internal retries remain inside
one SDK invocation. Logical model durations include backoff and the full stream.
Delivery `ok` means the checkpoint succeeded. This includes checkpoints that
close a permanent surface rejection, so it does not prove a message appeared. Delivery
errors leave existing lease-expiry retry behavior unchanged. Subsequent attempts
share the operation ID. Spawning a child does not claim that the child completed.

| Metric | Unit | Labels |
|---|---|---|
| `brainstone.work.started` | count | `kind` |
| `brainstone.work.finished` | count | `kind`, `outcome` |
| `brainstone.work.duration` | seconds, histogram | `kind`, `outcome` |

Duration buckets span 1 ms through 600 s, with an overflow bucket.
Metrics aggregate locally before trace sampling, independent of `RUST_LOG`.
Metric dimensions use fixed values. Operation, session, and lane IDs never
become labels. Neither do fencing tokens, names, URLs, model IDs, or tool names. These are observations,
not exactly-once durable totals or a queue-depth estimate. Started minus finished
is not a durable queue depth; process crashes can leave unclosed observations.

Traces and logs accept only reviewed numeric identifiers and fixed
kind/outcome values. The trace exporter strips arbitrary attributes, events, links and error
descriptions. The default log formatter discards message bodies, free-form errors,
arguments, results, plugin names and arbitrary fields. Logs identify their static
event site (`event`, `target`, including named `brainstone.*` lifecycle events), boundary, outcome, safe IDs, service/env/version,
and trace/span IDs. No content-logging opt-in is provided by this crate. Operator
resource attributes and endpoints are trusted configuration and must be safe.
Messages/Responses HTTP requests and MCP `_meta` carry W3C `traceparent`,
without baggage or user identity. Remote services must continue the context.
Bedrock SDK calls have local observations only; the adapter does not inject
customer trace context into AWS requests.
Surface and durable queue boundaries correlate through numeric operation IDs rather
than pretending external messages or restarted workers carry an active span.

## Failure contract

| Code | Behavior |
|---|---|
| `telemetry_configuration` | Invalid exporter/protocol/sampler or exporter construction; startup fails before installing the subscriber |
| `telemetry_already_installed` | Another global subscriber exists; it is preserved and the new initialization is rejected |

Both codes are typed `Error` variants and never include configuration values.
Receiver failures do not propagate into tools, stores or application results.
Each failed trace/metric exporter emits one content-free warning log
(`telemetry_trace_export_failed`, `telemetry_metric_export_failed`). SDK queue
warnings are safe JSON events, including `BatchSpanProcessor.SpanDroppingStarted`
and its shutdown drop count, subject to the log filter.

The span queue and stderr queue each hold at most 1,024 pending records;
log records larger than 16 KiB are dropped whole; trace
batches hold at most 128 spans. Saturated queues drop telemetry without waiting.
Metric streams have at most ten known kinds plus `unknown`, and six outcomes;
identifiers are excluded before aggregation. Export requests have a fixed
three-second timeout and no SDK retry; a local Agent/Collector owns delivery
retry. Cumulative metrics can recover later snapshots; delta intervals lost to
an outage cannot be recovered. No telemetry is journaled or replayed.

After producers drain, shutdown runs on a detached OS worker. The trace SDK
waits up to three seconds; the metric SDK waits up to five. The log worker
allows one second to drain. Full queues or stalled stderr can lose logs, but
never send warnings to stdout. Public shutdown waits at most ten seconds and
reports content-free warnings without changing the application exit status.
Diagnostics are subject to `RUST_LOG` and the same bounded stderr queue.
An in-progress export or blocked stderr may lose data; SIGKILL/crash cannot flush.
Allow application drain time **plus ten seconds** in termination grace periods.

## Verification

`cargo test -p brainstone-telemetry` launches the production initializer in isolated child
processes and decodes real HTTP OTLP protobuf at a local TCP receiver. It checks
resource identity, logs/traces, sampling, privacy, failed receivers, and a stalled
receiver that saturates the queue without blocking 5,000 observations. Runtime
regressions cover semantic failures and rebuilding from records after dropping
an attempt without repeating already journaled tool work. MCP's logging test
checks actual request propagation and queue/call spans.

Local receiver evidence does not verify deployed Agent ingestion, Datadog
retention/indexing, billing, log pipelines, host networking or a consumer rollout.

The stderr worker uses `std::sync::mpsc`. `tracing-appender` can print to
stdout when its queue is full during shutdown, corrupting driver results or
blocking on that output. A synchronous stderr writer can also block execution.
The worker keeps whole records, bounds their size, and detaches if draining
stalls. Tests cover blocked writers and oversized records.
