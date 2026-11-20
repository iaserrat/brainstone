# agent

The model/tool loop, conversation records, and extension traits.

This crate does not persist records, choose retry policy, or manage sessions.
It exposes provider progress separately from conversation state.

## The loop

Read a `Thread`, decide one `Step`, perform it, repeat:

```
next(&Thread) -> Step        pure: no I/O, no clock, no store
run(&mut Thread, ...)        performs what next decided, and records it
```

`Step` is the whole vocabulary:

| Step | Meaning |
|---|---|
| `CallModel` | Ask the provider for the next response |
| `RunTools` | Run the batch the model requested |
| `FailTruncated` | Answer a batch that was cut off mid-stream, without running it |
| `Done(Outcome)` | The run is over |

A successful result from the reserved `SILENCE_TOOL` (`Silence`) ends the run
as `Completed` with empty text. The transcript records this decision. An errored
result lets the model continue, including when the registry has no handler for
that name. Register `policy::Silence` or supply a handler under the reserved name.

To resume, rebuild the thread from stored records and call `next` again.
A restored thread and a live thread in the same state produce the same step.

## The seams

Four extension traits:

| Trait | Responsibility |
|---|---|
| `Model` | One settled response for one thread, with optional provisional progress. Owns retries and timeouts. Never returns a partial response |
| `Tool` | One callable capability. Its definition lives with its handler, so an advertised tool is always a dispatchable one |
| `ToolSource` | Tools that change while the process runs, such as one MCP catalog. `Registry::listed` lists its sources again and fixes the result for a run; a failed listing keeps the previous tools |
| `Sink` | Where events go. Fallible, because a run must not continue past a write that did not land |

```rust
use std::sync::Arc;
use agent::{Event, Model, Registry, Sink, Thread, run};

struct Stdout;

impl Sink for Stdout {
    type Error = serde_json::Error;

    async fn emit(&mut self, event: Event) -> Result<(), Self::Error> {
        println!("{}", serde_json::to_string(&event)?);
        Ok(())
    }
}

let mut thread = Thread::new("You are helpful.", "What time is it?");
let tools = Registry::new(vec![Arc::new(my_clock_tool)]);
let outcome = run(&mut thread, &my_provider, &tools, &mut Stdout).await?;
```

## Durability is the event contract

The loop emits each appended message in full as `Event::Appended`. Replaying
those events in order rebuilds the exact conversation. The runtime crate persists that event contract together with accepted
operations, leases, and outcomes; a `Sink` alone does not make prompt acceptance
durable.

The driver records each message before appending it to the in-memory thread.
If the process crashes between those steps, recovery can replay the recorded
message.

`ToolStarted` and `ToolFinished` report progress and rebuild nothing. Do not
depend on them for state.

A `ToolOutput` can carry text, images, and files. Images replay in the tool
result; only the appended `ToolResults` message stores them. `ToolFinished`
carries text alone. Files appear only in the reply, never in the result.
`execute_tool_with_media` returns them separately for a durable driver to
store; other execution paths drop them. A `File` holds a name, media type,
and bytes. `File::new` keeps a name only if a person could save it
as given (one path segment of 1 to `MAX_FILE_NAME_BYTES` bytes, no control
character, no surrounding spaces, not `.` or `..`), keeps only a media type's
lowercase `type/subtype`, and refuses empty bytes; `FileError::code` is
`invalid_file_name`, `invalid_media_type`, or `empty_file`.

`ModelDelta` and `ReasoningDelta` report provisional progress and rebuild nothing.
`ReasoningDelta` contains only provider-visible reasoning text, never encrypted
payloads or signatures. A streaming model emits
ordered deltas through `generate_stream`, then returns one settled
`ModelResponse`. Adapters hand each wire delta to `Progress::text` or
`Progress::reasoning` and call `Progress::flush` when the stream idles and
before settling; `Progress` batches up to 512 characters and emits in arrival
order. The default implementation delegates to `generate` and emits
no deltas. A surface may render deltas, but must treat the later `Appended`
assistant message and `RunFinished` outcome as authoritative.

The standalone driver sends tools `ToolRequest::idempotency_key = None`. A
durable driver supplies a stable key, retries the same key after a crash, and
requires the tool to atomically associate that key with its side effect and
stored result. A tool that ignores the key can repeat an external side effect.
Execution profiles carry `tool_key_version = "operation-v2"`. Durable tool
identity counts assistant responses across the whole operation, independently
of the response budget. Steering and compaction do not reset it. A restored
thread must receive the current operation's durable assistant count through
`Thread::with_tool_sequence`; counting visible lane messages is insufficient
after compaction. `tool_key_step()` returns `None` when that count is missing,
and a durable driver must fail closed. Unknown key versions are rejected.

The durable driver also supplies `ToolRequest::context` with the caller's
operation identity and inherited `ExecutionProfile`; ordinary standalone calls
leave it `None`.

## Retries live in the provider

A `Model` handles backoff, timeouts, and retries. It reports unrecoverable
failures as `StopReason::Error { code, message }`. The loop receives only
settled responses and needs no backoff logic.

Execution profiles are unlimited by default. `ExecutionProfile::new` accepts
`None` for unlimited responses or a positive integer for an explicit limit.
`max_turns()` returns `Option<usize>`. An exhausted finite limit returns
`step_budget_exhausted`; unlimited execution has no response-count cutoff.
Output correction shares that same limit and still permits only one correction.

`TurnLimit` serializes as a positive integer or `"unlimited"`. The runtime
rejects invalid stored limits. Compaction and recovery preserve the accepted
limit. Callers can opt in to the `MAX_STEPS` limit.

## Reasoning is data, not behaviour

`Reasoning::Thinking { text, signature }` and `Reasoning::Redacted { data }` are
stored verbatim and replayed ahead of text and tool calls.
`Reasoning::Opaque { provider, data }` carries a provider's complete reasoning
item when reducing it to those two common shapes would lose replay data. Only
the named provider may replay it; provider adapters must reject foreign items
rather than silently dropping them.

After tool use, signed thinking must replay unchanged and in order in the
assistant turn. The provider rejects requests with rewritten, reordered, or
missing blocks.

`ModelResponse::model` names the model that produced a response. `None`,
omitted from stored JSON, means the producer is unknown.

`Thread::replay` prepares history for one named model. It omits responses that
ended in an error. Responses from another or unknown model keep their text
and tool calls but lose all reasoning. Calls without results receive an
`unanswered_tool_call` result. When no changes are needed, replay borrows the
thread and preserves the model's own history byte for byte. The application
provider calls it before each request.

## Failure codes

Failures are typed, and every one carries a stable slug consumers branch on.
Free-text messages are for humans and are never the branching signal.

A run ends as `Outcome::Failed { code, message }` with:

| Code | Cause |
|---|---|
| `step_budget_exhausted` | The run exhausted an explicitly configured response limit. Unlimited executions do not emit this code |
| *provider-defined* | The provider reported `StopReason::Error`. Its code passes through unchanged, so provider crates own their own catalogue |
| `context_window_exceeded` | `CODE_CONTEXT_WINDOW_EXCEEDED`: the request did not fit the model's context window. Every provider crate reports an overlong request under this one code, because it is the one rejection a smaller history answers |

A tool result the model reads is a structured document, never prose it has to
parse: `{"error": "<code>", "message": "..."}` with `is_error` set.

| Code | Cause |
|---|---|
| `unknown_tool` | The model called a name no registry entry answers to. The run continues so the model can correct itself |
| `truncated_tool_call` | The response carrying the call hit the output token limit. Streamed arguments are salvage-parsed, so a truncated call can parse and validate while being silently incomplete. Nothing in the batch is executed |
| `unanswered_tool_call` | `Thread::replay` found a call no result answers: the run that asked for it ended first, as when a subagent is stopped mid-batch and messaged again. Only the replay carries it; nothing is stored |

`execute_tool_with_context` bounds each tool result at creation. Text past `MAX_TOOL_RESULT_BYTES` (100 KiB)
keeps its first and last 50 KiB around one marker line,
`[tool_result_truncated: <omitted> of <total> bytes omitted from the middle of
this result]`. `CODE_TOOL_RESULT_TRUNCATED` names it. It is not a failure: the
result keeps its `is_error` value. The executor truncates failure messages
before building their JSON document, so the document remains parseable. This
bound prevents one replayed result from exceeding the context window.

## Known limitations

- **Tool batches run sequentially.** Providers emit batches meant to run
  concurrently, so a multi-call turn costs more wall time than it needs to.
  Both the standalone and durable drivers preserve provider order.
- **The core does not choose compaction boundaries.** It can replay a stored
  `Message::Summary`; the durable runtime commits explicit summaries.
- **Standalone runs have no steering.** The durable runtime provides steering
  through its admission API.

## Dependencies

The crate uses `serde` and `serde_json` for records, `schemars` for typed tool
schemas, `jsonschema` for schema validation, `base64` for binary data, and
`tracing` for observations. It uses no executor API; `tokio` is a test dependency only.

## Building

Requires Rust 1.100.0 or newer, edition 2024. Run the workspace gate from the
repository root:

```sh
cargo xtask gate    # fmt --check, clippy -D warnings, test, doc
cargo xtask audit   # machete, deny check, audit
```

## Declared model capabilities

`Model::validate_request` and `Model::context_window` have defaults, so existing
implementations need no changes. `CapabilityModel<M>` checks a target's
`ModelCapabilities` before inference, forwards feedback, and supports models
that only return settled output. Unsupported input stays intact and returns a
typed `CapabilityError` as a terminal response. A declared context window can
tighten runtime compaction without changing the operator's configured ceiling.

| Code | Missing capability |
| --- | --- |
| `unsupported_tools` | Tool definitions or tool-call history |
| `unsupported_images` | User image input |
| `unsupported_tool_images` | Images in tool results |
| `unsupported_reasoning_effort` | Explicit reasoning effort |

## Tool error interoperability

`ToolError` implements `Display` and `std::error::Error`; its display text is
for humans. Tool authors keep typed errors internally and map them at the
`Tool::call` boundary to a documented, stable `ToolError::code`. This open
namespace lets third-party tools report failures without extending a framework
enum. Use a tool-specific prefix for application codes. The model receives one
structured failure document with `error` and `message`; branch on `error`, never
on the display text.

## Structured output contracts

`OutputSchema::new` validates and compiles a named JSON Schema. Local `$defs`
references and built-in formats are supported; external retrieval is refused.
The immutable compiled validator is shared across clones. `OutputError` exposes
`invalid_output_schema` for construction errors and `structured_output_invalid`
for invalid final JSON or schema mismatches.

`ExecutionProfile::with_output_schema` pins the contract in accepted records.
`next` validates only terminal successful output; tool exchanges keep their
normal shape. Truncated answers fail even when their prefix parses as JSON.
No automatic schema-repair calls are made. Profiles without a schema retain
existing behavior. See the [guide](../../docs/structured-output.md).

## Typed tools and accepted tool contracts

`typed_tool` derives an object input schema from a Rust type and adapts an async
function to `Tool`. Arguments validate before dispatch. Success is JSON with
`outcome: "success"` and `value`; errors retain the `{ "error", "message" }`
format. `Registry::with_tool` registers owned tools. See the
[tool guide](../../docs/tools.md) for complete examples.

`ExecutionProfile::with_tool_contract` stores exact allowed tool names, approval
requirements, and required successful calls. `ToolContract::default` permits
all registered tools without adding requirements. Setters replace their sets;
names are case-sensitive, sorted, and deduplicated.

Only committed non-error tool results in the current operation satisfy
`require_success`. Compaction preserves this evidence. Custom drivers restoring
a `Thread` must call `with_tool_successes` with the current operation's complete
successful result names, including an empty list when none succeeded. Prior
operations and progress events are excluded. `run` enforces allowlists; approval
contracts require the durable runtime and fail before inference under `run`.

| Code | Meaning |
| --- | --- |
| `invalid_tool_definition` | A typed tool has an invalid name, description, or input schema |
| `invalid_tool_arguments` | Arguments fail schema validation or deserialization; handler not called |
| `tool_output_invalid` | Successful output cannot serialize or exceeds `MAX_TOOL_RESULT_BYTES` |
| `tool_durability_required` | A handler requires a retry key that its driver did not supply |
| `invalid_tool_contract` | Names are invalid or required names are outside the allowlist |
| `tool_requirement_unmet` | Successful terminal output lacks a required committed tool result |
| `tool_evidence_unavailable` | Recovery omitted authoritative current-operation evidence |
| `tool_approval_unsupported` | The non-durable driver cannot authorize the accepted contract |

Missing required results and missing recovery evidence fail without a model
repair call. They do not override an existing failed outcome. Successful tool
execution remains the handler's assertion, not proof of business correctness.
