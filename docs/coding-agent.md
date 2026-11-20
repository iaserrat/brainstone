# A coding agent

The [example](../crates/brainstone/examples/coding_agent.rs) composes a model,
three typed tools, a SQLite store, and a host. It performs an editing task,
requires approval before changes or checks, and returns a validated JSON
summary. It uses the same execution path for a scripted demo and a real model.

## Run the local demo

Run these commands from the repository root:

```sh
cargo run -p brainstone --no-default-features --features openai --example coding_agent -- \
  demo data/coding-demo
```

The first run creates a small Rust library with a failing addition test. It
only creates fixtures in a new or empty directory. The scripted model reads
`src/lib.rs`, replaces subtraction with addition, and runs library tests.
The demo approves only these fixed calls. It needs no credentials and makes
no model requests. Cargo runs offline.

The final answer is a JSON object with `summary`, `files`, and `checks`.
Operation IDs and approval requests go to stderr; the committed answer goes
to stdout. Rerun the command to read that same answer without another edit or
check. Existing demo files stay unchanged on setup retries.

## Run on your project

The live example uses an OpenAI Responses-compatible endpoint:

```sh
export OPENAI_API_KEY='your-api-key'
export OPENAI_MODEL='your-model-id'
cargo run -p brainstone --no-default-features --features openai --example coding_agent -- \
  run ./my-project fix-1 'Fix the failing test in src/lib.rs'
```

Set `OPENAI_ENDPOINT` for another Responses-compatible server. It must be a
complete Responses URL. The example requires a non-blank API key and model ID.

Choose a Rust library project with a lockfile and dependencies already cached
for offline use. This workflow requires both an edit and passing library tests.
Use a different contract for read-only questions or projects needing other
commands. The response limit is 12 model turns.

Each edit or check pauses for operator approval. The terminal displays the
stored request, including its operation, exact arguments, tool definition,
and binding. Approve the requested action or deny it; end of input does not
grant approval. Cargo executes project code with the agent process's
permissions. The example is for a trusted workspace, not an OS sandbox.
Keep other writers out of that workspace while it runs.

A new task needs a new request ID. Retrying the same ID and input returns the
original operation; changing input under the same ID fails with
`idempotency_conflict`.

## Composition

The example keeps application behavior outside the framework:

| Part | Coding-agent use |
| --- | --- |
| `Model` | A deterministic demo or `ResponsesModel` |
| `typed_tool` | Argument schemas, validation, typed results, and stable errors |
| `Registry` | `ReadFile`, `EditFile`, and `RunChecks` |
| `ExecutionProfile` | A response limit, tool contract, and output schema |
| `ToolContract` | Approval for edits and checks; both must succeed before completion |
| `OutputSchema` | JSON summary with `summary`, `files`, and `checks` |
| `Runtime` / SQLite | Accepted operations, journals, leases, and recovery |
| `Host` | Accept one task, execute its operation, then shut down |

The [profile](../crates/brainstone/examples/coding_support/profile.rs) names
the allowed tools and requires successful `EditFile` and `RunChecks` results.
A model's claim that it edited or tested cannot satisfy those requirements.
The application also rejects edits after successful checks in that operation.
Edit and check calls must use separate model turns. The tool-name contract
alone does not establish this ordering.

The [main function](../crates/brainstone/examples/coding_agent.rs) owns setup
and shutdown. File handling, command execution, approvals, and the demo model
live in small modules beside it. Replace those application pieces to adapt the
agent; no new runtime or provider interface is needed.

## Tools and boundaries

`ReadFile` inspects workspace paths. `EditFile` replaces an existing UTF-8
file using its exact `expected` and `replacement` contents. It refuses
conflicting contents, protected paths, traversal, and symlinks. File versions
must fit within 64 KiB.

The check tool runs only:

```sh
cargo test --offline --locked --lib -- --test-threads=1
```

The model cannot select a shell command or change its arguments. Checks have
a 30-second deadline and a 100 KiB output limit. A failing test, timeout, or excess
output returns a tool error. Successful checks mean this command passed;
they do not establish that every possible project check passed. Killing Cargo
does not guarantee that every process started by project code also stops.

## Tool failures

These codes belong to the example tools, not the framework:

| Code | Meaning |
| --- | --- |
| `coding_invalid_path` | Absolute, parent, protected, symlink, or non-file paths where a file is required |
| `coding_io_failed` | Filesystem or process I/O failed |
| `coding_file_too_large` | A file version exceeds 64 KiB |
| `coding_invalid_utf8` | File contents are not UTF-8 |
| `coding_directory_too_large` | A directory has more than 64 visible entries |
| `coding_file_conflict` | Contents match neither the approved old nor new version |
| `coding_no_change` | Expected and replacement contents are equal |
| `coding_checks_failed` | Cargo returned a failing exit status |
| `coding_checks_timeout` | The check exceeded its deadline |
| `coding_checks_output_limit` | Captured or encoded output exceeds its limit |
| `coding_checks_already_passed` | This operation already committed passing checks; further edits need a new operation |
| `coding_checks_order` | An edit shared a model turn with a check |
| `coding_context_required` | The driver supplied no durable operation context |
| `coding_journal_failed` | The tool could not read its operation's committed history |

## Recovery

Try the crash boundary in a fresh demo directory:

```sh
cargo run -p brainstone --no-default-features --features openai --example coding_agent -- \
  demo data/coding-recovery --crash
cargo run -p brainstone --no-default-features --features openai --example coding_agent -- \
  demo data/coding-recovery
```

The first process exits with code 73 after the atomic file replacement but
before the tool result commits. The second reopens its records. The pending
call still contains the approved old and new contents. If the file matches
the replacement, the handler returns the same receipt without rewriting it.
If it matches the expected contents, it applies the change. Any third state
fails with a conflict.

Once the tool result commits, recovery replays that result instead of running
the edit again. This is file-state reconciliation using durable arguments;
it is not a transaction spanning SQLite and the filesystem.

For a live operation, use the ID printed at acceptance:

```sh
cargo run -p brainstone --no-default-features --features openai --example coding_agent -- \
  resume ./my-project 42
```

Replace `42` with your operation ID and keep the same provider configuration.
Resume supplies no new task. It waits for an existing writer's lease when
necessary. Ctrl-C leaves accepted work durable for a later resume.

State lives in `.brainstone/` under the project root. A stored canonical root
binds it to that workspace; copying the state to another root does not silently
redirect pending edits. The demo crash run uses a five-second lease; normal execution uses 30 seconds.

See [tool contracts](tools.md) and the
[runtime recovery contract](../crates/runtime/README.md#leases-recovery-and-visibility)
for the framework guarantees behind this example.
