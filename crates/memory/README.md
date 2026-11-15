# memory

Cross-session memory as composable capability pieces. The crate adds
`MemorySave` and `MemoryRecall` to an existing `agent::Registry` and provides
`MemoryAdmission`, a surface-boundary decorator that prepends recent memories
to each prompt before durable acceptance.

Every trigger is a separate operator switch in `Config`, and everything
defaults to off: saving happens only when the model calls `MemorySave`,
injection only when `inject` is set, and searching only when `MemoryRecall`
is exposed. `Scope` decides ownership: each conversation remembers
independently, every conversation on a surface shares one memory, or each
surface user remembers independently across conversations. The user scope
needs the prompt's actor — the surface user recorded at acceptance. A prompt
without one skips injection, and the memory tools fail its operations with
the stable code `memory_actor_required`; subagent operations carry no actor,
so user-scoped saves from subagents fail with the same code.

Storage and idempotency delegate to `runtime::Memories`. Tool calls resolve
their scope through the runtime's durable operation context, and `MemorySave`
keys each memory by the stable tool-call idempotency key, so a crash-retried
call saves exactly one memory. Injection happens before acceptance, which
makes the block part of the durable prompt record: recovery replays it
unchanged, and provider context still grows only at the tail. The durable
stores use PostgreSQL or SQLite, while normal tests use `MemoryStore` and never
open a database connection.

The crate intentionally owns no worker, model loop, provider, surface, or
ranking: recall is recency plus case-insensitive substring match, and the
agent loop is never touched.
