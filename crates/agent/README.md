# agent

The agent core loop: the conversation data, three seams, and the driver that
turns one into the other. One loop that serves any provider, any tool, and any
surface.

Everything that makes it reusable is something it refuses to know about. It
does not persist, choose retry policy, or know what a session is. It exposes
provider progress without making that progress conversation state.

## The loop

Read a `Thread`, decide one `Step`, perform it, repeat. Deciding and performing
are separate on purpose:

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

One tool name is reserved. A tool result naming `SILENCE_TOOL` (`Silence`)
that did not error ends the run as `Completed` with empty text: declining to
reply is a structured act the transcript keeps, not the absence of output an
empty assistant turn leaves behind. An errored result is not that decision —
the likeliest cause is that the registry carries no such tool — so the model
sees the error and answers. Registering a handler under that name is the
caller's job; `policy::Silence` is the built-in one.

Because `next` is pure, **resume is not a code path**. Rebuild the thread from
whatever you stored and call `next` again. A restored thread and a live one at
the same shape get the same answer.

## The seams

Four traits, implemented outside this crate:

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

The loop persists nothing, but it is built so that persisting it is trivial.

Every message the loop appends is emitted as `Event::Appended` carrying that
message in full. Replaying those events in order rebuilds the conversation
exactly. The runtime crate persists that event contract together with accepted
operations, leases, and outcomes; a `Sink` alone does not make prompt acceptance
durable.

Messages are **recorded before they are appended**, so a store that dies mid-run
is ahead of memory rather than behind it. Replay can drop a duplicate; it cannot
invent a message it never saw.

`ToolStarted` and `ToolFinished` report progress and rebuild nothing. Do not
depend on them for state.

A `ToolOutput` may carry images beside its text. They replay inside the same
tool result, and only the appended `ToolResults` message stores them:
`ToolFinished` carries the text alone. It may also carry files, which only the
reply shows: they never enter the result. `execute_tool_with_media` hands them
back beside it so a durable driver can keep them for the reply; every other
path drops them. A `File` is a name, a
media type, and bytes. `File::new` keeps a name only if a person could save it
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
The durable driver also supplies `ToolRequest::context` with the caller's
operation identity and inherited `ExecutionProfile`; ordinary standalone calls
leave it `None`.

## Retries live in the provider

A `Model` implementation owns its own backoff, timeouts, and retries, and
reports whatever it could not recover from as
`StopReason::Error { code, message }`. The loop only ever sees settled
responses, so it holds no backoff logic at all.

Execution profiles are unlimited by default. `ExecutionProfile::new` accepts
`None` for unlimited responses or a positive integer for an explicit limit.
`max_turns()` returns `Option<usize>`. An exhausted finite limit returns
`step_budget_exhausted`; unlimited execution has no response-count cutoff.
Output correction shares that same limit and still permits only one correction.

`TurnLimit` serializes as a positive integer or `"unlimited"`. Existing numeric
profiles retain their accepted counts. Missing, zero, negative, or malformed
stored limits are rejected. Compaction and recovery preserve the accepted limit.
Thread snapshots predating profiles retain their original 100-response limit.
`MAX_STEPS` remains that legacy count and an opt-in limit for explicit callers;
new executions never use it implicitly.

## Reasoning is data, not behaviour

`Reasoning::Thinking { text, signature }` and `Reasoning::Redacted { data }` are
stored verbatim and replayed ahead of text and tool calls.
`Reasoning::Opaque { provider, data }` carries a provider's complete reasoning
item when reducing it to those two common shapes would lose replay data. Only
the named provider may replay it; provider adapters must reject foreign items
rather than silently dropping them.

Signed thinking must replay unmodified and in order in the assistant turn that
follows tool use, or the provider rejects the whole request. Rewriting,
reordering, or dropping a block is a correctness bug, not a cosmetic one.

`ModelResponse::model` names the model that produced a response; `None`, and
absent from the stored JSON, means the producer is unknown. `Thread::replay`
is the history one named model may be shown: responses that ended in an error
are left out, responses stored under another name or none keep their text and
tool calls but lose all their reasoning, never part of it, and every tool call
no result answers gets an `unanswered_tool_call` result. It borrows the thread
when nothing changes, so a model's own history replays byte-identical. The
app's provider model calls it before every request.

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

Every result is bounded where it is made, in `execute_tool_with_context`, the
one path every tool runs through. Text past `MAX_TOOL_RESULT_BYTES` (100 KiB)
keeps its first and last 50 KiB around one marker line,
`[tool_result_truncated: <omitted> of <total> bytes omitted from the middle of
this result]`. `CODE_TOOL_RESULT_TRUNCATED` names it. It is not a failure: the
result keeps its `is_error`. A failure's message is cut the same way before its
document is built, so the document stays parseable. Every later request replays
a result, so without the bound one call could outgrow the context window.

## Known limitations

- **Tool batches run sequentially.** Providers emit batches meant to run
  concurrently, so a multi-call turn costs more wall time than it needs to.
  Both the standalone and durable drivers preserve provider order.
- **The core does not choose compaction boundaries.** It can replay a stored
  `Message::Summary`; the durable runtime commits explicit summaries.
- **No steering or follow-up.** A run cannot be interrupted or extended once
  started.

## Dependencies

`serde` and `serde_json`, and nothing else. In particular **no async runtime**:
the crate is async but touches no executor API, so `tokio` is a dev-dependency
for its tests only.

## Building

Requires Rust 1.95.0 or newer, edition 2024. Run the workspace gate from the
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
