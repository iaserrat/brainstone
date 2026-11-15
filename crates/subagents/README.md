# subagents

Anthropic-compatible project-agent definitions and durable control tools for
Brainstone.

`Catalog::load` recursively reads `agents/**/*.md`, validates YAML
frontmatter, and preserves the Markdown body as the child's system prompt.
`Control` validates definitions against the composed tool registry and adds
`Agent`, `SendMessage`, `TaskOutput`, and `TaskStop` through the ordinary
`agent::Tool` seam.

The crate does not own child state. It uses `runtime::SubagentStore`, so a spawn
is an idempotent durable operation, every child has an isolated session, and
control calls are restricted to the same root tree. `Control::foreground`
provides the same agent catalog while overriding every background request to
run inline, for one-shot hosts that deliberately have no resident worker. The
durable stores use PostgreSQL or SQLite; `runtime::MemoryStore` exists for
deterministic contracts and does not provide process durability.

An omitted `maxTurns` inherits the parent's accepted response limit, including
unlimited execution. A positive integer sets a finite limit; `unlimited`
explicitly removes an inherited limit. Children keep this snapshot across
recovery and subsequent definition edits. `Definition::max_turns()` returns
`Option<agent::TurnLimit>`: `None` means inheritance, and
`Some(TurnLimit::Unlimited)` explicitly removes the limit.

See the [operations reference](../../docs/operations.md#capabilities) for the file format, supported
fields, limits, operator behavior, and known compatibility boundaries.
