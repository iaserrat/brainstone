# runtime

The durable authority around the provider-neutral `agent` loop. It accepts a
surface prompt before work starts, maps conversations to sessions and
sub-conversations to lanes, and runs one fenced writer per session. PostgreSQL
and file-backed SQLite are durable backends.

## Setup

```rust
use std::time::Duration;
use agent::Registry;
use runtime::{Address, Config, PostgresStore, Prompt, Runtime};

let store = PostgresStore::connect(&database_url).await?;
let config = Config::new("deployment-instance-7", Duration::from_secs(30))?;
let runtime = Runtime::with_config(store, config);

let address = Address::new("app", "conversation-42", "thread-9")?;
let prompt = Prompt::new(address, "event-id", "Be useful.", "Hello")?;
runtime.accept(&prompt).await?;

while runtime.run_next(&model, &Registry::default()).await?.is_some() {}
# Ok::<(), Box<dyn std::error::Error>>(())
```

`PostgresStore::connect` creates a pool, requires `synchronous_commit`, `fsync`,
and `full_page_writes` to be on, and applies the embedded schema baseline
(`0001_runtime.sql`, one transaction). The database role therefore needs
connect, schema DDL, and table DML privileges. Use a stable, unique worker
identity per live process. `Runtime::new` generates a process-local identity
and uses a 30-second lease; explicit `Config` is preferred in a multi-replica
deployment.

The schema ships as one baseline: a database either matches it or is
unsupported. Startup records the applied version, and SQLx rejects a database
whose bookkeeping names a version outside this binary's embedded manifest
rather than guessing. Future schema changes extend that manifest forward-only:
a database can always move to a newer binary, never back to an older one once
its recorded version advances.

## Backend contract

`Runtime<S>` remains generic and existing store implementations remain valid.
`DurableStore::connect` selects PostgreSQL or SQLite from the URL and implements
every store trait used by the application. Embedders can also construct
`SqliteStore` directly. No existing trait method became mandatory.

Both backends commit admission, fencing, journal writes, settlement, schedule
release, and subagent notifications at their existing atomic boundaries.
Replay codecs, attachment validation, recurrence decoding, and operation-state
interpretation are shared. PostgreSQL uses row and advisory locks; SQLite uses
immediate write transactions. SQLite applies its schema under an immediate
write transaction, holding the write lock during inspection and application.

SQLite startup requires file storage, WAL, full synchronous commits, and foreign
keys. All processes sharing the file must be on one host. PostgreSQL remains
appropriate for workers on separate hosts. See [operations](../../docs/operations.md#sqlite)
for storage, backup, and deployment rules. Switching a URL does not migrate data.

`backend_contracts` runs the same application behavior on both backends. SQLite
cases run in `cargo xtask gate`; PostgreSQL cases run in `cargo xtask postgres-test`. SQLite
also tests cross-pool ownership, concurrent migrations, unreadable records, and
recovery after an abrupt child-process exit.

## Identity and acceptance

An `Address` maps `(surface, conversation)` to one session and the lane name to
one lane in that session. The lane's system prompt is immutable. `(lane,
request_id)` is the idempotency boundary: replaying identical input returns the
original operation, while reusing the ID with changed input is rejected.

`accept` explicitly queues and commits the session, lane, original surface
text, model-visible prompt, accepted operation, and optional `DeliveryTarget`
in one transaction. Admission layers may enrich model-visible text with current
skills or memories; idempotent retries compare the separately stored original
text, so changing enrichment after a crash neither spends twice nor hides a
caller changing its actual input. A surface may acknowledge receipt only after
acceptance returns.

`admit_with_tools` is the surface-facing policy boundary. On an idle lane it
accepts an operation; on an active lane it stores the prompt as steering for
that operation. Both paths deduplicate by `(lane, request_id)`. Its supplied system prompt is a default for a new lane;
an existing lane keeps its stored prompt, including after a deployment changes
the surface configuration. Low-level `accept` remains strict and rejects a
configuration mismatch.

An idle lane adopts the execution profile supplied with its next operation,
so deployment-level turn-budget changes do not invalidate existing lanes. An
active operation and steering accepted into it keep that operation's existing
profile; the new profile takes effect after the lane becomes idle.

```mermaid
stateDiagram-v2
    [*] --> Accepted: accept transaction commits
    Accepted --> Running: session lease acquired
    Running --> Completed: terminal event commits
    Running --> Failed: terminal failure commits
    Running --> Running: heartbeat renews lease
    Running --> Running: expired lease is reclaimed
```

## Leases, recovery, and visibility

PostgreSQL claims lock the operation, lane, and session using `FOR UPDATE ... SKIP LOCKED`.
SQLite claims hold an immediate transaction while selecting work and advancing
the session fence.
The session lease enforces one writer across lanes and replicas. Every claim
increments a fencing token; event writes and heartbeats require the current
owner, token, unexpired lease, and running state. Terminal state, outcome, and
lease release commit atomically.

The runtime records model and tool-loop events at durable boundaries:

- A crash during a model request may cause that request to be sent again after
  lease expiry. No response becomes conversation history until its event
  commits.
- A durable tool call receives
  `operation/{operation}/step/{step}/call/{provider-call-id}`. A tool must
  atomically store this key with its side effect and return the prior result on
  retry. Ignoring the key forfeits exactly-once effects.
  The step component preserves historical counting for compatibility with
  already-issued keys; it is not the remaining turn budget.
- A persistence error stops the driver. Another replica may resume only after
  the lease expires or the terminal transaction releases it.

`ModelDelta` records provisional presentation progress. It never rebuilds the
agent thread. Only the settled assistant `Appended` event becomes provider
context, and only `RunFinished` carries the authoritative outcome.

The guarantee is durable accepted work and a single observable committed
outcome. It is not exactly-once provider billing, and it cannot make an
external tool idempotent on the tool's behalf.

## Surface delivery

A delivery worker claims work by adapter with its own fenced lease. Unread
journal events are always claimable. Accepted work and a still-running
operation whose cursor has caught up are also claimable after the adapter's
presence interval, so a surface can show work waiting for the session writer
and keep presenting a live turn whose indicator expires independently of the
journal. Each checkpoint advances the cursor when events were claimed, records
`presented_at`, and may save the surface's external message ID. A crash replays
from the last committed cursor. A stale worker cannot
checkpoint after another replica takes its lease. A worker may renew the same
lease while an external presentation is in flight; if renewal loses its fence,
it returns `lease_lost` and the batch remains owed. A new journal event fences
an empty presence-only lease so durable progress never waits for an indicator
refresh to expire.

Presence is scoped to a lane and adapter, not to every queued operation. The
oldest accepted or running operation represents that live lane. A later
operation in the same lane and adapter remains ineligible until its predecessor
has no pending delivery, so a caught-up terminal checkpoint from an older
binary cannot strand the lane. Parallel delivery workers cannot post
authoritative replies out of conversation order. Other lanes and adapters remain independently claimable.
This keeps presence work bounded when several prompts wait behind the same
session writer.
A final terminal checkpoint leaves a settled delivery tombstone, so idempotent
acceptance cannot resurrect a delivered operation. Claim polling uses a partial
index that excludes those tombstones, keeping idle work proportional to
claimable deliveries rather than accumulated history. Polling is proportional
to the unsettled working set across adapters; a synthetic 200,000-row
adapter-skew backlog measured 135 ms. Deployments that expect a working set of
that size need adapter partitioning beyond this schema. If a failed index
build leaves an index invalid, the next startup serializes its repair before
retrying migrations, without blocking operation writes.

Accepting new steering clears `presented_at` and fences any presentation lease
in the same transaction, making the running operation immediately claimable
until delivery successfully presents and checkpoints it again. Checkpointing a
terminal event hands presence to the next accepted or running operation in the
same lane and adapter, fencing that handoff if it is already in flight. Other
lanes and adapters are unchanged.

Every image and file a tool call returns is stored by `Store::save_media`
before the call's `ToolFinished` is recorded, as `Media` keyed by its reference
(`media::reference` or `file_reference`) under the operation's root session
(its own, or the one its subagent run descends from). Files live only there;
the thread never holds them. Storing the same media again is a no-op that refreshes its
age. A file row keeps its name; the reference's prefix, not the media type,
says what a row holds, so a file whose media type is `image/png` reads back
as a file. A row this build cannot interpret is rejected as `corrupt_record`
rather than guessed at.
`DeliveryStore::media(operation, reference)` reads one back for a reply from
any lane of that root, after compaction or a restart. It is a read: it takes
no lease. PostgreSQL keeps rows in `tool_media` for 7 days from their last
store and prunes older rows on every write; a row it cannot interpret is
`corrupt_record`. The memory store keeps media for the life of the process.

## Steering

The built-in stores expose a fenced, non-consuming `Store::has_steering`
probe. During ordinary inference the runtime polls it every 100 ms. On pending
input it drops the old generation future, then converts one queued prompt to
ordinary durable user events and starts a new request. A probe never consumes
input, so cancellation cannot commit a correction without applying it.
Partial output and unfinished reasoning never enter provider history. Completed
reasoning stays unchanged, including across tool results, steering, and recovery.

Before each unstarted tool call, the runtime checks for steering again. An
already running tool finishes; remaining calls receive structured
`tool_call_superseded` errors without executing. The full result batch commits
before the steering user event, preserving provider call/result pairing.
A crash between those commits recovers the closed batch and pending correction.
Superseded means not started in the current attempt. After a crash, an earlier
attempt may already have produced an external effect without storing its full
result; the error explicitly requires reconciliation before retrying that action.
This cannot undo an effect that has already started. Tools must still honor
their durable idempotency keys. Cancelling generation does not guarantee the
provider stops work or billing.

Custom stores retain boundary-only steering through the default probe and can
opt into interruption by implementing its cancellation-safe read contract.
`None` means the probe is unsupported; `Some(false)` avoids an empty consuming
transaction, and `Some(true)` permits interruption.
FIFO admission and one-prompt-at-a-time consumption remain unchanged.
The terminal settlement transaction locks the operation and lane,
then either appends steering and continues the loop or records `RunFinished`,
marks the operation terminal, and releases its lease. A process crash leaves
unconsumed steering available to the replacement writer. New steering also
invalidates the active operation's surface presentation atomically; a duplicate
admission does not invalidate it again.
Each consumed steering prompt starts a fresh model-turn budget. Compacted
steps from the preceding turn do not consume that budget, including on replay.

`failure_boundaries` exercises lost model-commit acknowledgements, external
effects before tool-result persistence, partial tool batches, interrupted
streaming, failed heartbeats, steering-commit acknowledgement loss, and delivery
receipt recovery with stale-lease rejection. The SQLite campaign also holds a
real writer lock past the busy timeout. The same commit-boundary cases run on
PostgreSQL through `cargo xtask postgres-test`.

## Compaction

`Runtime::compact` appends a summary for a completed operation boundary. It
never deletes the event journal. Future context starts with
`Message::Summary` and then replays operations after that boundary. Empty,
backward, conflicting, cross-lane, non-terminal, and concurrently running
boundaries are rejected.

`Config::with_compaction` also enables automatic compaction. Before every model
request, the runtime estimates the serialized thread plus the claim's tool
definitions. At the configured token limit it asks the same model for a summary
without tools, records `Event::Compacted`, and continues from
`Message::Summary`. The journal remains append-only, steering is applied before
the next regular model request, and compaction does not reset the turn budget.

A summary request must itself fit the provider's context window, and the
history it summarizes may not: an estimate can undercount, and a lane may hold
tool results recorded before results were bounded. So the request is asked
from a ladder, most faithful first, and each rung answers a provider that
refused the one before with `context_window_exceeded`:

1. The whole history, with its images when the replay bound
   allows them.
2. The whole history as text.
3. The newest half of its user turns, halving until one turn is left. A rung
   always starts on a user turn, because a provider rejects a request that
   opens on an assistant message or on tool results whose calls it cannot see.

Every rung holds each tool result to `agent::MAX_TOOL_RESULT_BYTES`. Turns left
out of the rung that fits are left out of the summary; that loss is the price
of a lane that answers again. Only a single turn too long for the window on its
own fails, under `context_window_exceeded`, and the next prompt on that lane
starts a newer turn that fits without it, so no lane stays stuck on the
failure. Nothing is recorded between rungs, so a crash mid-ladder resumes by
climbing it again.

The estimate decides when to compact, but the provider has the last word: a
regular request it refuses with `context_window_exceeded` compacts the thread
the same way, and the loop asks again from the summary. The refused response is
never appended. A thread of one message has nothing to leave out, so that
refusal ends the turn under `context_window_exceeded`, after one request.

The summarization request is not a turn of the loop, so it asks for `low`
effort rather than inheriting the lane's. Thinking is drawn from the same
response budget as the summary on every current provider, and a run-depth
budget spent on reasoning leaves nothing for the summary itself.

A summary whose stream broke (`transport_error`, `request_timeout`,
`provider_unavailable`) is retried up to three attempts with sub-second
backoff. The provider seam owns retries, but only until the response stream
opens: a failure raised mid-stream reaches the runtime having been tried once,
and that gap is what this budget covers. `rate_limited` is deliberately not in
that set — the provider seam already waits a throttle out properly, honouring
`retry-after` on the HTTP path and 1 to 20 seconds of SDK backoff on Bedrock,
so a throttle arriving here needs far longer than this loop waits, and
repeating a thread-sized request into a rate limiter adds load without changing
the answer. Retrying a mid-stream throttle belongs at the provider seam, where
`retry-after` is visible. Nothing else is retried — a refused request answers
the same way every time, and a response that arrived and was unusable would
exhaust the same budget again. The attempt budget belongs to the run, not to
each queued steering prompt: once a summary has failed, the rest of the run
reuses that outcome without calling the provider again.

A failed summary then ends the turn, never the worker. The operation settles
through the ordinary path: terminal, so no replica re-claims it, *unless
steering arrived while it ran*, in which case the steering is absorbed and the
run continues, exactly as for any other failed outcome. That loop is bounded by
the steering queue — one queued prompt per iteration — but not by the turn
budget, because a failed summary appends no assistant message. It costs no
further provider calls, because the failed outcome is reused.

A provider failure keeps the provider's own code (`rate_limited`,
`provider_unavailable`), identical to that failure during a regular turn, with
the stage named in the message so an operator can tell a throttled summary from
a throttled reply — a throttle during compaction is reported after one attempt,
not three. A nonterminal, empty, or tool-calling response reports
`compaction_failed` with the stop reason in its message. Neither is a
`runtime::Error`.

The lane's own effort writes the summary; `low` is a fallback taken only after
an observed `MaxTokens`, never on speculation. A summary is durable and
replaces the prefix permanently, so writing every one at reduced reasoning
would trade a crash for a quiet downgrade of every later turn on that lane.

A summary the model cannot fit in `ANTHROPIC_MAX_TOKENS` even at `low` effort
is not recoverable by retrying: every turn on that lane will fail until the
output budget is raised. `Runtime::compact` would clear such a lane by hand,
but no surface in this workspace calls it, so there is no operator recovery
path today.
Operators running long threads should size `ANTHROPIC_MAX_TOKENS` for a summary
plus its thinking, not for a chat reply.

The estimator is intentionally provider-neutral and conservative: the greater
of Unicode scalar count or UTF-8 bytes divided by four. Its units are not
provider tokens, and the ratio is script-dependent: measured against a 4
bytes-per-token rule it runs 4.0x real tokens for ASCII and 1.25x for CJK, so
one configured limit gives ASCII roughly three times the headroom it gives CJK.
Operators should size the limit from the provider's real context window and
keep a safety margin, rather than reading the limit as a token count.

## Scheduled work

`ScheduleStore` persists follow-up prompts against their originating lane.
Relative timing is resolved once; retries with the same operation and
idempotency key return the original schedule. A schedule's owner is the
authenticated actor of the operation that created it. Listing and cancellation
reach the schedules created in the caller's lane and, when the caller is in a
conversation private to that principal, every schedule the same provider,
tenant, and actor owns; a shared conversation never lists one person's
schedules to everyone in it. Unauthenticated schedules have no owner and stay
lane scoped.

A `ScheduleRequest` with a `Destination` posts each run as a new conversation
root elsewhere instead of replying in the origin lane. The destination names a
surface session (`surface`, `conversation`), the surface's routing record for a
new root (`target`), and the principal the runs execute as (`identity`). The
store accepts one only from an authenticated operation that has a delivery
target and that no schedule released (a person's own surface message, never a
scheduled run or a subagent), posting as that same provider, tenant, and actor
into a conversation that is not private to them, whose session key is the
identity's `conversation_key()`; anything else fails with
`schedule_destination_forbidden`.
One owner may create `MAX_POSTS_PER_OWNER_PER_HOUR` (10) such schedules in a
trailing hour (`schedule_post_rate_exceeded`), counted under the owner's
advisory lock. Each release opens a new lane in the destination session, named
by the run's request ID and carrying the origin lane's system prompt, tool
definitions, and profile, with the destination identity. The surface posts the
root, then calls `DeliveryStore::bind_lane` under its delivery lease: the lane
takes the root's external ID and the operation's delivery target becomes the
reply route, so a retry after the bind is an ordinary thread delivery and
replies to the root join the run's lane with its history. `bind_lane` returns
`false` when another lane already holds that ID. Every released operation
records `schedule_id`. Anyone whose operation is in the destination
conversation (same provider, tenant, and conversation) can list and cancel the
schedule. `DeliveryBatch::accepted_unix_ms` bounds a surface's search for a
root it posted before a crash, and `DeliveryStore::source_text` returns what a
person wrote for an operation, before any context was added.

An ordinary claim transaction first promotes at most one due schedule into an
accepted operation with request ID `schedule/<id>`, copies its delivery target
and execution identity, and marks it released. PostgreSQL uses database time
and `FOR UPDATE SKIP LOCKED`, so replicas cannot promote the same schedule
twice. No scheduler daemon or in-process timer owns correctness.

A standing schedule (`ScheduleTime::Repeat`) holds a `Recurrence`: elapsed
minutes, or a local time in an IANA zone every N days, every N weeks on ISO
weekdays, or every N months on a day of the month (clamped to a shorter
month's last day) or on the 1st to 4th or last weekday. `RepeatSpec` is its
field form, the RFC 5545 subset FREQ, INTERVAL, BYDAY (with ordinal), and
BYMONTHDAY; the runtime validates it on creation and again when it reads a
stored rule, and rejects an unreadable one as `corrupt_record`. Intervals and
every-N rules count from the row's `created_at`: an interval runs whole steps
after it, and an every-N rule runs the periods of its first occurrence after
it. New rows store the rule as JSON under kind `recur`; rows of kind `repeat`
keep their `repeat_*` columns and decode to the same type. A standing row stays
pending: each release promotes an operation with
request ID `schedule/<id>/<run_at_unix_ms>` and advances `run_at_unix_ms` to
the first occurrence strictly after now, so downtime yields one catch-up run
and DST changes keep the wall-clock time (a time inside a spring-forward gap
runs just after it). A row whose previous run is still
accepted or running is not due, so runs never overlap. A schedule that replies
in its origin lane is not due while that lane is unfollowed
(`LaneStore::unfollow_lane`); `Scheduled::paused` reports it, and following
the lane again resumes it with at most one catch-up run. `MAX_REPEAT_PER_LANE`
(20) standing rows per lane are admitted under the lane lock, and
`MAX_REPEAT_PER_OWNER` (50) per owner across lanes under a transaction
advisory lock on the owner, taken in its own statement before the count. The finish
transaction settles the row's `consecutive_failures`: a success resets it and
the `REPEAT_FAILURE_CANCEL_AFTER`th (14) consecutive failure cancels the row.
From `REPEAT_FAILURE_NUDGE_AFTER` (3) the failed run's `DeliveryBatch` carries
`notice`, runtime-owned text a surface appends to its failure reply.

## Tool catalog

A claim offers the tools of the registry that runs it. Before the first model
request the runtime lists every `agent::ToolSource` in that registry again,
then fixes the result for the claim, so every request in its loop shares one
cached prefix. The next operation on any lane, including lanes created by
earlier releases, sees tools added or removed since; nothing is stored, so
nothing is backfilled. A resumed operation lists again too, so after a deploy
it offers the schemas the new code accepts. `lanes.tool_definitions` is still
written when a lane is created so that a rollback to an older binary, which
offers that column to the model, keeps working; nothing in this release reads
it.

A subagent's grant is the names of the tools it was spawned with, read from its
`subagent_runs` request. Its operations offer those names at their current
schemas. A tool added after the spawn is not granted, and a child spawned with
no tools can call none.

A catalog change alters the request prefix, so each lane misses the provider
cache once on its first request after the change.

## Durable subagents

`SubagentStore` creates each child as an isolated session linked to its parent
operation and root session. The spawn key is idempotent, the resolved execution
profile is stored with the child lane, the tool names it was spawned with
are its grant (see Tool catalog below), and
`run_operation` claims only the named child. Follow-ups append operations to
that same lane. Retrying a follow-up returns its original operation and keeps
the latest child operation unchanged. Stop changes accepted or running work to
`agent_cancelled` and advances the session fence before returning.

Background completion atomically creates one idempotent operation in the
parent lane. PostgreSQL serializes the per-tree active-child count by locking
the root session; the limit is 20 across all replicas.

## Stable failure codes

| Code | Meaning |
|---|---|
| `invalid_input` | An external identity is empty or a lease duration is zero or unrepresentable |
| `idempotency_conflict` | A request ID was reused with different input |
| `lane_configuration_conflict` | A low-level accept changed a lane's system prompt or execution profile |
| `durability_not_guaranteed` | A required database durability setting is off, or SQLite uses memory storage |
| `store_unavailable` | The database could not be reached or aborted the operation for a transient reason (connection, pool timeout, serialization conflict, deadlock, shutdown, resources); a retry can succeed |
| `store_failed` | The database refused the operation itself (for example a constraint or a NUL byte in text); a retry fails the same way |
| `session_busy` | Session deletion refused an active execution or delivery lease |
| `lease_lost` | The writer no longer owns the current unexpired fence |
| `corrupt_record` | Stored state cannot be interpreted safely |
| `invalid_compaction` | The summary or boundary is unsafe |
| `schedule_not_found` | Scheduled work is absent or not visible to the caller |
| `schedule_limit_exceeded` | The lane already holds the maximum standing schedules |
| `schedule_owner_limit_exceeded` | The owner already holds the maximum standing schedules across lanes |
| `schedule_destination_forbidden` | A schedule that posts elsewhere was not requested by a person's own surface message, as them, into a shared conversation |
| `schedule_post_rate_exceeded` | The owner already created the maximum posting schedules in the trailing hour |
| `invalid_delivery_checkpoint` | The cursor moves backward, beyond the journal, or under a stale delivery fence |
| `agent_concurrency_exceeded` | A root tree already has 20 active children |

`CODE_COMPACTION_FAILED` (`compaction_failed`) is an operation outcome code,
not an error code: it reaches the surface as a failed turn. So is
`context_window_exceeded` when even the smallest summary request did not fit.

Consumers branch on `Error::code()`, never on display text. Stored events carry
schema version 1; unknown or malformed versions are corruption rather than
being guessed at during recovery.

## Testing

The normal workspace gate uses an in-memory implementation of the same `Store`
contract and never opens a PostgreSQL connection:

```sh
cargo xtask gate
```

Migration, transaction, compaction, and cross-pool fencing checks are opt-in.
The database is migrated and receives test rows, so point this only at a
dedicated disposable database:

```sh
BRAINSTONE_TEST_DATABASE_URL=postgres://... cargo xtask postgres-test
```

## Deliberate limits

- `run_next` claims and finishes at most one operation; `run_operation` targets
  one id but waits behind earlier nonterminal work in that lane. The host owns
  polling, shutdown, admission control, and surface acknowledgements.
- Tool batches run sequentially.
- Retention is operator-owned. Automatic compaction keeps the full event
  journal and deliberately changes only provider context.
- Recurrence has no end date or run count, and no fifth-weekday rule (most
  months lack one). The operator's interval floor is enforced by the
  `Schedule` tool; `ScheduleStore` callers enforce their own.
- Cancelling stops future runs only. A run already in progress finishes: ending
  it from outside its lease would make the cancel path a second writer of that
  session's journal. A steering message reaches the run before its next model
  call, and whether it stops there is the model's decision.
- A standing schedule's identity is the acceptance-time snapshot. Nothing
  re-checks that the owner still exists; the failure streak is the only
  automatic stop. Delivery failures retry and never count toward the streak.
- `Store` is public for deterministic contract tests and alternate durable
  implementations. Implementations must define `admit_with_tools` explicitly:
  surface admission preserves an existing lane's system prompt while low-level
  `accept` stays configuration-strict. `PostgresStore` and `SqliteStore` carry the durability contract for their
  supported deployment profiles.

The append-only conversation tree follows
[Pi's session design](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/src/core/session-manager.ts),
the replayable step machine follows the same principle as
[Rig's sans-I/O agent loop](https://docs.rs/rig/latest/rig/agent/run/index.html),
and claim/heartbeat/fencing boundaries were checked against
[Codex's durable runtime](https://github.com/openai/codex/tree/main/codex-rs).


## Operator inspection and authorization

`DurableStore::{operations,inspect,journal}` provide bounded operator reads.
Metadata omits conversation bodies; explicit journal access decodes stored event
versions and rejects unknown versions. No read changes lease ownership.

`Config::with_tool_policy` adds a gate at the shared execution boundary.
`ToolPolicy::new` accepts exact names or `*`; deny wins. `Store::authorize_tool`
has an additive default returning `ApprovalState::Unsupported`, so existing
custom stores compile unchanged and required authorization fails closed.
`DurableStore` persists exact `ApprovalRequest` bindings and immutable decisions;
`approval`, `pending_approvals` and `decide_approval` support operator review.
Actor names are attribution; database access is authority. Decisions expire
15 minutes after creation. Pending waits renew the lease and remain steerable.
Recovery requires the same request and current policy/executor binding.

Tool-result codes: `tool_policy_denied`, `tool_approval_denied`,
`tool_approval_expired`, and `tool_approval_unsupported` all mean this attempt
did not dispatch the tool. Earlier attempts may still need effect
reconciliation. Storage conflicts use `invalid_input`; malformed/unknown
approval records use `corrupt_record`; stale writers use `lease_lost`.
Native tools, remote servers and custom stores retain their existing trust
boundaries. See [operator and isolation operations](../../docs/operations.md#operator-commands-and-tool-approvals).

### Deleting a durable session

`DurableStore::delete_session(&address)` permanently removes the root session
identified by the address's surface and conversation; the lane is ignored. It
returns `false` if the session is absent. All lanes and subagent descendants,
operations, journals, compactions, steering, approvals, interactions, schedules,
deliveries, and stored media owned by the session are removed in one transaction.
A later acceptance at that address creates a new session and may use a new
system prompt or execution profile.

Stop admission, execution, and delivery workers for that conversation before
calling it. Active execution or delivery leases in the session tree return
`session_busy`; addressing a subagent directly returns `invalid_input` with
field `root_session`. Storage failures roll back the deletion. The API does
not terminate external tool processes. Schedules owned by deleted sessions are
removed; schedule links into surviving conversations are detached while their
operations remain. Shared memories have independent ownership and are
retained. No database migration is required.
