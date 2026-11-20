# Contributing to Brainstone

These instructions apply to every change in the workspace. Make the smallest
correct change after tracing the affected behavior.

## Runtime invariants

- An accepted prompt survives a crash or deploy. Recovery completes it or closes
  it explicitly. No partial outcome is durable or authoritative. A surface may
  show provisional progress only when terminal reconciliation replaces it with
  the exact durable outcome.
- Provider context only grows at the tail. Compaction and a tool catalog
  change between operations are the two deliberate cache invalidations.
- One process is the single writer for a session. States a single writer cannot
  produce are corruption, not edge cases.

## Architecture

- Follow [Large Rust Workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html) for workspace layout.
- Consult [Codex](https://github.com/openai/codex/tree/main/codex-rs) when evaluating agent runtime design.

## Work Order

1. Understand the request and trace the affected flow end to end.
2. Research current Rust best practice before changing architecture, async,
   error handling, dependencies, or performance-sensitive code.
3. Refactor first when the existing shape blocks the change.
4. Write or update a failing test first.
5. Make the smallest change that turns the test green.
6. Run formatter, lints, tests, and any relevant smoke checks.
7. Refactor again only while tests stay green.

No red-green-refactor loop means the change is unfinished.

## Documentation Rule

`README.md` is a short quick start for humans: what Brainstone is, how to compose an
agent in Rust, plus links. Keep reference detail out of it.

Any change that affects how an operator installs, configures, runs, or safely
interprets the harness updates the matching doc in the same patch:

- settings, hooks, deployment, and known limits: `docs/operations.md`
- contracts and crate-level failure codes: the owning crate's README
- service-level and extension failure codes: `docs/failure-codes.md`
- crate layout and diagrams: `docs/architecture.md`

Change `README.md` only when the commands to compose or run an agent change.
Run every command you put there.

## The Minimality Ladder

Before writing new code, stop at the first rung that solves the problem:

1. Does this need to exist at all? If not, do nothing.
2. Does this already exist in the codebase? Reuse it.
3. Does the Rust standard library solve it? Use it.
4. Does the platform already provide it? Use it deliberately.
5. Does an already-installed dependency solve it? Reuse it.
6. Can the existing design be simplified so the problem disappears?
7. Only then write new code, and only the minimum that works.

No speculative abstractions. No framework habits. No feature creep disguised as
cleanup. No new dependency without a written reason and a smaller rejected
alternative.

## Rust Design Rules

- Model the data first. If the data shape is wrong, the algorithm is wrong.
- Prefer simple structs, enums, functions, and modules over clever trait
  hierarchies. A new trait needs the same justification as a new dependency.
- Use types to make invalid states unrepresentable.
- Prefer owned values at boundaries and borrowed values inside hot or local flows.
- Do not clone to silence the borrow checker. Explain ownership instead.
- Async is demanded by the domain (concurrent lanes, long-lived surface
  connections), so tokio is sanctioned — but `Arc`, `Mutex`, `RefCell`, and
  channels still require the data lifecycle to demand them. Prefer generics over
  `dyn`; `dyn` is for genuinely heterogeneous sets.
- Errors are part of the API. Use `Result` for expected failures and panic only
  for programmer bugs or impossible states guarded by invariants.
- Public behavior is userspace. Do not break it unless explicitly requested.

## Error Reporting For Machine Consumers

The model is the principal consumer of tool results, and recovery code is the
principal consumer of stored records, so failures both must react to are part of
the public API and get the same rigor as success output.

- Model expected failures as typed enums, not strings. Each variant exposes a
  stable, documented code the consumer can branch on without parsing free text.
  Free-text messages are for humans, never the branching signal.
- A tool result returned to the model is one structured document: outcome, stable
  error code on failure, nothing else.
- Never introduce a new failure shape without a stable code, a test asserting it,
  and a documentation entry when operator-visible. A missing code is a breaking
  change.
- Stored records are forward-only: recovery must reject records it cannot
  interpret rather than guess.

### Public enums

A public enum this workspace only produces and a caller only reads carries
`#[non_exhaustive]`, so a new variant is a compatible change. An enum a caller
must construct stays exhaustive: `#[non_exhaustive]` forbids construction, and
a `Store`, `Admission`, or `ThreadSource` implementation has to build the error
it returns. Marking an enum is a decision about who constructs it, not about
how many variants it has.

## Simplicity Rules

- KISS and YAGNI are mandatory.
- Single responsibility applies to crates, modules, files, functions, types, and
  tests.
- A junior engineer must be able to read the code without reconstructing hidden
  intent.
- Prefer a clear `match` over clever combinator chains when readability suffers.
- No magic numbers or magic strings. Use named `const` values.
- Zero avoidable repetition. Reuse existing helpers and extract only when
  duplication is real, current, and harmful.
- Comments explain non-obvious intent, invariants, or tradeoffs. They do not
  narrate obvious code.

## Size And Complexity Budgets

- Rust source file hard limit: 100 lines of code. Comments and blank lines do
  not count, which is the same rule `clippy.toml` enforces for functions. If a
  `.rs` file exceeds 100 code lines, refactor before adding behavior.
  Documentation and config files are exempt. Test targets are exempt too —
  `*_tests.rs`, `src/tests.rs` or any `#[cfg(test)]` module file, and
  `tests/*.rs` — because table-driven cases and their rationale comments are
  the point of those files, but the shared setup helpers inside them still
  follow the function limits below.
- Function hard limit: 25 lines.
- Cyclomatic complexity hard limit: 5 per function.
- Nesting hard limit: 2 levels.
- Function parameters hard limit: 4. Use a named input type when that is clearer.
- Module responsibility hard limit: one reason to change.

Breaking a budget requires refactoring first, not an excuse.

These budgets are enforced by `clippy.toml`, not by good intentions. Clippy's
own defaults (100 lines, 7 arguments) are far looser and would let every
function-level violation through silently.

One exemption, and only one: a **flat dispatch match** — one arm per variant,
each arm delegating rather than deciding — may exceed the function line limit.
Splitting a dispatch table across helpers makes it harder to read, not easier,
and hides whether every variant is handled. Mark it explicitly:

```rust
#[allow(clippy::too_many_lines, reason = "flat dispatch: one arm per variant")]
```

An unmarked over-budget function is a violation. A marked one whose arms
contain logic rather than delegation is also a violation — the exemption is
for tables, not for long functions wearing a `match`.

Test files are exempt from the file limit, and from the function line limit
where the length is table-driven cases: state that with a module-level
`#![allow(clippy::too_many_lines)]` and a comment. Their helper functions are
not exempt.

## Tooling Gate

Every meaningful change must pass the strictest available local gate:

```sh
cargo xtask gate    # fmt --check, clippy -D warnings, tests, doctests, doc
cargo xtask audit   # machete, deny check
```

`xtask/src/gate.rs` is the gate's source of truth and `xtask/src/audit.rs` the
audit's. Change a step there, not here, so the two cannot drift. The
`postgres-test` task is the opt-in durable suite; it refuses to run without
`BRAINSTONE_TEST_DATABASE_URL`.

**Mutation testing is not part of the gate.** `cargo mutants` rebuilds the
workspace once per mutant, which on this codebase means tens of gigabytes of
build artifacts and hours of CPU for a signal that is usually already available
by reading the test. Do not run it routinely, and never run it across the whole
workspace.

Reach for it only when a specific test's _ability to fail_ is genuinely in
doubt and reading the code has not settled the question — a security gate, a
crash-resume path, a comparison that looks like it might never be false. Scope
it to those files (`cargo mutants --file …`), and prefer the cheaper answer
first: change the line the test covers by hand, confirm the test goes red, and
change it back. That is the same evidence in seconds instead of hours.

The standard it serves still stands: a test that cannot fail is worse than no
test. Mutation is one way to check that, not the required one.

Use `rust-analyzer` diagnostics while editing. Treat compiler, formatter, linter,
LSP, docs, and test failures as blockers.

Cargo uses edition `2024` and declares the supported Rust version, shared
dependency versions, and lint tables in the workspace manifest. The workspace
forbids unsafe code and denies Clippy’s `all` and `pedantic` groups.

## Test Requirements

- Tests describe behavior, not implementation trivia.
- Every bug fix starts with a failing regression test.
- Every durability claim gets a crash-resume test: stop at a boundary, rebuild
  from records, assert the lane restores and resumes.
- Edge cases belong in tests: empty input, invalid input, boundary values,
  Unicode (user-authored text routinely carries it), and failure paths.
- Performance claims require benchmarks or measured before/after numbers.

## Review Standard

Reject patches that are broad, vague, clever, untested, or user-hostile. Call out:

- unnecessary abstraction
- unrelated churn
- cloned data hiding ownership mistakes
- special cases caused by bad data modeling
- dependency additions without proof
- unverified performance claims
- functions or files over budget
- behavior changes without tests

Every changed line must have a direct reason to exist.

## Done Means Done

A task is complete only when the code is smaller or no larger than necessary, tests
prove the behavior, tools are clean, and the resulting design is easy to explain.
