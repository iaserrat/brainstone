# Changelog

Notable changes to this workspace. The crates are versioned together at
`0.1.0`; entries are ordered by date.

## Unreleased

- `ExecutionIdentity` carries `private: bool` instead of a conversation-kind
  taxonomy: true when the conversation is readable by that principal alone.
  Surfaces that do not model visibility send `false`, and the personal-data
  features (user-scoped memory, privacy-gated tool access, schedule ownership)
  stay off — fail-closed. A schedule may now post into any conversation that
  is not private to the requester, not only broadcast-style ones.
  `AccessPolicy`'s `one_to_one` flag is `private_conversation`, with stable
  codes `private_conversation_required` and
  `memory_private_conversation_required` replacing `one_to_one_required`.
- First public import: durable sessions and lanes, admission, leases, recovery,
  journal, steering, compaction, durable approvals, MCP tools, skills,
  sub-agents, schedules, memory, OTLP telemetry, and the Anthropic, Bedrock, and
  OpenAI-compatible adapters over SQLite or PostgreSQL.
- The workspace publishes as `brainstone` plus `brainstone-*` crates, versioned
  together at `0.1.0`, with SQLite and PostgreSQL sharing one behavioral
  contract.
- Each backend's schema ships as one baseline migration (`0001_runtime.sql`).
  There is no migration history: a database either matches the baseline or is
  unsupported, and startup rejects other migration bookkeeping rather than
  guessing.
- The `Schedule` tool's optional post target argument is `conversation`, named
  in surface-neutral vocabulary; the surface's `Destinations` seam still
  decides where a run may post.
- Video is not a tool output: a `video/*` MCP blob becomes the
  `mcp_video_unsupported` note, and a stored `video:` row is rejected as
  `corrupt_record`. No stored row is rewritten.
- Seventeen output-only public enums are `#[non_exhaustive]`; error enums a
  `Store`, `Admission`, or `ThreadSource` implementation must construct stay
  exhaustive.
- Every public item is documented, and `#![deny(missing_docs)]` holds that
  line. The gate fails on rustdoc warnings and runs doctests.
- MSRV is 1.95.0, proven by a CI job that runs the declared floor rather than
  the pinned compiler.
