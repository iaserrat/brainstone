# subagents

Loads Anthropic-compatible project agents and adds durable controls.

`Catalog::load` reads `agents/**/*.md` recursively. It checks YAML frontmatter
and keeps the Markdown body as the child's system prompt. `Control` checks
definitions against the tool registry. It adds `Agent`, `SendMessage`,
`TaskOutput`, and `TaskStop` as `agent::Tool` handlers.

`runtime::SubagentStore` owns child state. Spawning is a durable, idempotent
operation. Each child has its own session, and control calls stay within the
same root tree. `Control::foreground` uses the same catalog but runs all requests inline,
even background requests. This supports one-shot hosts without a resident
worker. PostgreSQL and SQLite persist child state. `runtime::MemoryStore`
supports repeatable contract tests but cannot survive a process exit.

An omitted `maxTurns` inherits the parent's accepted response limit, including
unlimited execution. A positive integer sets a finite limit; `unlimited`
explicitly removes an inherited limit. Children keep this snapshot across
recovery and subsequent definition edits. `Definition::max_turns()` returns `Option<agent::TurnLimit>`. `None` inherits
the limit. `Some(TurnLimit::Unlimited)` removes it.

See the [operations reference](../../docs/operations.md#capabilities) for the file format, supported
fields, limits, operator behavior, and known compatibility boundaries.
