# agent

The agent core loop. The workspace charter in `../../AGENTS.md` applies in full;
this file adds only what is specific to this crate. Do not restate the charter
here.

## What this crate is for

One loop that drives any provider, any tool, and any surface. Everything that
makes it reusable is a thing it refuses to know about.

## The seam that must not be crossed

**This crate never persists anything, and never learns that persistence
exists.** No store, no records, no lanes, no sessions, no coordinator, no
session ids. A durable store is a `Sink` implementation living in another
crate, and it stays that way.

The previous attempt at this harness fused durability into the loop. Its drive
loop became a claim/settle protocol over a record log, which meant the loop
could not be read, tested, or reused without the record catalogue, restore
reducer, and coordinator all in scope. It reached 125 files in one crate before
it was abandoned. If a change wants the loop to know about records, the change
belongs somewhere else.

## Invariants a change must not break

Each of these has a test. If you change the behaviour, the test should go red
first; if it does not, the test is wrong and fixing it is part of the work.

- **`next` is pure.** No I/O, no clock, no randomness, no store. If a decision
  seems to need any of those, the decision is in the wrong place. This purity is
  what makes resume free rather than a second code path, and it is the single
  most valuable property in the crate.
- **The event stream alone rebuilds the thread.** Every message appended must be
  emitted as `Event::Appended` carrying the message in full. Adding a message
  the loop appends without emitting it silently breaks every future durable
  consumer. `ToolStarted` and `ToolFinished` report progress and rebuild
  nothing; do not make a consumer depend on them for state.
- **Record before append.** A store that dies mid-run must be ahead of memory,
  never behind it. Reversing the order in `run::append` loses data.
- **Every call in a batch gets a result.** Providers reject a batch with an
  unanswered call. There is no path out of `call.rs` that returns fewer results
  than it was given calls, and there must not be.
- **Reasoning blocks are verbatim.** Signed thinking must replay unmodified and
  in order in the assistant turn after tool use, or the provider rejects the
  whole request. Never rewrite, reorder, drop, or normalise a `Reasoning` its
  producer is shown. `Thread::replay` removes a response's reasoning whole,
  and only for a model that did not produce it.
- **A truncated batch never executes.** Streamed tool arguments are
  salvage-parsed, so a call cut off by the token limit can parse and validate
  while being incomplete. `Step::FailTruncated` exists only for this.

## Retries do not belong here

A `Model` implementation owns its backoff, timeouts, and retries, and maps what
it could not recover from into `StopReason::Error { code, message }`. The loop
sees settled responses only. No backoff, jitter, sleep, or timeout code in this
crate. The loop enforces only explicitly configured execution-profile turn limits;
new profiles are unlimited by default.

## Dependencies

`serde` and `serde_json`. That is the whole list, and it should stay that way.

**No async runtime.** The crate is async but touches no executor API, which is
why it has no `tokio` dependency outside dev. Reaching for `tokio` or `futures`
here means something that belongs in a caller has leaked in. Concurrent tool
execution is the one change that will tempt this; solve it without making the
core runtime-specific.

## Adding a failure

A new failure shape needs all four, in the same patch:

1. A `const` in `step.rs` holding a stable slug.
2. The failure produced through it, never as an inline string.
3. A test asserting the code, not the message.
4. A row in `README.md` under Failure codes.

A missing code is a breaking change. Free text is for humans and is never the
branching signal.

## Tests

- No network, no sleeping, no real provider, no filesystem. Every test runs in
  microseconds and always will.
- Drive behaviour with the `Scripted` provider and `Echo` tool in
  `run_tests.rs`, and the `Recorder` sink for events. Extend those doubles
  rather than adding new ones.
- `next_tests.rs` is the table of transitions. A new `Step` variant or stop
  reason means a new row there before any driver work.
- Before trusting a test that guards one of the invariants above, change the
  line it covers by hand, confirm it goes red, and change it back. That is the
  same evidence `cargo mutants` gives, in seconds instead of hours, and the
  charter's rule against running mutants routinely applies here.
