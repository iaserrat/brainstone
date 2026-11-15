# host

The process-lifecycle layer around a durable `runtime`. It gives any surface
the same small contract: construct a validated `Prompt`, await durable
acceptance, and acknowledge the external event only after `Host::accept`
returns. The `Admission` seam returns the durable operation id, so synchronous
surfaces can wait for exactly what they admitted while event surfaces may ignore
the receipt.

The host owns worker polling, in-process wakeups, admission, and graceful
shutdown. It does not own surfaces, process signals, model configuration, or
tools; those belong to the embedding application.

## Wiring

```rust
use agent::Registry;
use host::Host;
use runtime::{Address, PostgresStore, Prompt, Runtime};

let store = PostgresStore::connect(&database_url).await?;
let host = Host::new(Runtime::new(store.clone()), model, Registry::default());
let worker = host.clone();
let worker_task = tokio::spawn(async move { worker.run().await });

let address = Address::new("app", "conversation-42", "thread-9")?;
let prompt = Prompt::new(address, "event-id", "Be useful.", "Hello")?;
let accepted = host.accept(&prompt).await?;
acknowledge_surface(accepted).await?;

wait_for_process_signal().await;
host.shutdown().await;
worker_task.await??;
store.close().await;
# Ok::<(), Box<dyn std::error::Error>>(())
```

There is deliberately no host binary in this crate. A binary must choose a
surface, credential source, model, tools, and signal environment, and compose
the host around them.

## Named one-shot execution

`Host::execute(operation_id)` drives only that operation and returns its durable
terminal `Operation`. If another process owns the session, it waits and reads
the same stored outcome instead of launching a competing model call. Completed
idempotent retries return immediately. It never drains unrelated queued work,
which makes it suitable for CI and request/response surfaces.

## Worker behavior

`Host::run` supervises 20 durable worker loops by default. Each loop executes
one operation at a time, and session leases still permit only one writer per
session. A successful local `accept` wakes a worker immediately. The pool also
polls every 250 milliseconds by default
so one replica discovers work accepted by another. Override that fallback with
`Config::new`; the supported range is 10 milliseconds through 1 minute. The
lower bound prevents a failed-notification path from hammering PostgreSQL; the
upper bound limits cross-replica discovery latency and timer size.

One `run` future is the minimal deployment. `Config::with_workers` selects 1
through 64 local workers. Additional replicas scale across
sessions safely because PostgreSQL leases and fencing remain authoritative.
The host notification is only a latency optimization and is never durability
state.

A `store_unavailable` or `lease_lost` from `run_next` pauses only the worker
that saw it. The worker logs `worker store failure; retrying` and retries after
250 ms, 500 ms, 1 s, 2 s, 5 s, then 10 s, starting over after any successful
cycle. The interrupted operation stays leased until its lease expires; then any
worker resumes it from its records. On a seventh consecutive failure, or on any
other runtime error, the worker closes admission and `run` returns that error.
The embedding process should report the failure and restart under its
supervisor rather than spin on a broken dependency.

## Graceful shutdown

`Host::shutdown` is idempotent. It takes the exclusive admission boundary,
which has three consequences:

- New and waiting `accept` calls return `host_shutting_down`.
- An idle worker wakes and exits.
- A durable acceptance or claimed operation already inside the boundary is
  allowed to finish before shutdown returns.

Unclaimed accepted operations remain in PostgreSQL for another replica or the
next process start. The host never cancels a claimed operation halfway through
its event journal. The embedding process may impose a hard external termination
deadline; killing it then falls back to the runtime lease-expiry recovery path.

Call `shutdown` from an independent task, such as the process-signal task, not
from a model or tool currently executing inside the same host boundary.

## Stable failure codes

| Code | Meaning |
|---|---|
| `invalid_input` | The polling interval is outside 10 ms through 1 minute |
| `host_shutting_down` | Admission has closed |
| `worker_failed` | A supervised worker panicked or was cancelled |
| `admission_denied` | `GatedAdmission` rejected a prompt whose actor is missing or not allowlisted |
| `admission_quota_exceeded` | `GatedAdmission` rejected a prompt whose actor is at its daily operation limit |
| `model_selection_failed` | `ModelSelection` rejected a new prompt because its `ModelSelector` failed or answered something other than one model and one effort level |
| *runtime code* | Durable acceptance or execution failed; see the runtime catalogue |

Consumers branch on `Error::code()`, never its display text.

## Testing

The host suite uses public in-memory `Store` implementations and never calls
PostgreSQL:

```sh
cargo test -p host
```

The boundary follows [Pi's embeddable session lifecycle](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/sdk.md),
keeps the UI/core separation and drain semantics visible in
[Codex's protocol](https://github.com/openai/codex/blob/main/codex-rs/docs/protocol_v1.md),
and leaves lifecycle I/O outside the step machine as in
[Rig's sans-I/O loop](https://docs.rs/rig/latest/rig/agent/run/index.html).
