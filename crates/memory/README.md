# memory

Memory across sessions. The crate adds
`MemorySave` and `MemoryRecall` to an existing `agent::Registry` and provides
`MemoryAdmission`, a surface-boundary decorator that prepends recent memories
to each prompt before durable acceptance.

Each memory feature has a separate switch in `Config`; all default to off.
Saving requires a `MemorySave` call. Injection requires `inject`, and
searching requires exposing `MemoryRecall`. `Scope` decides ownership: each conversation remembers
independently, every conversation on a surface shares one memory, or each
surface user remembers independently across conversations. The user scope
requires the prompt's actor: the surface user recorded at acceptance. Without
an actor, admission skips injection and memory tools return
`memory_actor_required`. Subagent operations carry no actor, so their
user-scoped saves return the same code.

Storage and idempotency delegate to `runtime::Memories`. Tool calls resolve
their scope through the runtime's durable operation context, and `MemorySave`
keys each memory by the stable tool-call idempotency key, so a crash-retried
call saves exactly one memory. Injection happens before acceptance, which
makes the block part of the durable prompt record: recovery replays it
unchanged, and provider context still grows only at the tail. The durable
stores use PostgreSQL or SQLite, while normal tests use `MemoryStore` and never
open a database connection.

Recall uses recency and case-insensitive substring matching. The crate does
not add a worker or change the agent loop. Providers and surfaces remain
application concerns.
