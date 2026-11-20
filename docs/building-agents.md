# Build an agent

Start with two provider-backed examples: a chat assistant and an order-support
agent.
Both call an OpenAI Responses endpoint and store work in local SQLite files.
The chat example has no tools. The support example adds a native Rust tool.

## Setup

Run commands from the repository root with the pinned Rust toolchain installed.
Set credentials and choose a model your account can use:

```sh
export OPENAI_API_KEY='your-api-key'
export OPENAI_MODEL='your-model-id'
```

The examples read these variables; Brainstone's core does not require them.
For another Responses-compatible server, set `OPENAI_ENDPOINT` to its complete
Responses URL. The examples still require a non-blank API key and model name.
These examples use the Responses protocol, not Chat Completions.

The commands below create `data/chat.db` and `data/order-support.db`. Each
example creates `data/` if needed. Set `DATABASE_URL` to choose another SQLite
file; its parent directory must exist. These examples construct `SqliteStore`
directly. For PostgreSQL, construct `PostgresStore` instead, or use the
[configured preset](../crates/brainstone/README.md#configured-preset).

## 1. A chat assistant that remembers

[chat.rs](../crates/brainstone/examples/chat.rs) supplies a system prompt and
an empty tool registry. Its [shared runner](../crates/brainstone/examples/support/session.rs)
constructs the provider, runtime, and host, then accepts and executes one message.

```sh
cargo run -p brainstone --no-default-features --features openai --example chat -- \
  intro-1 'My name is Zoë. I am learning Rust.'
cargo run -p brainstone --no-default-features --features openai --example chat -- \
  question-2 'What is my name, and what am I learning?'
```

Each command starts a new process. The second reads the first turn from the
database, so the model has the earlier conversation. The program prints only
the committed answer to stdout. The model writes that answer, so exact wording
can vary. Operation IDs and duplicate receipts go to stderr.

Retry the second command unchanged:

```sh
cargo run -p brainstone --no-default-features --features openai --example chat -- \
  question-2 'What is my name, and what am I learning?'
```

It prints the same stored answer and reports `duplicate=true`. It does not
call the model again. Give each new message a new request ID. Reusing an ID
with different text fails with `idempotency_conflict`.

Set `CONVERSATION_ID` to start another conversation in the same database:

```sh
CONVERSATION_ID=another-chat cargo run -p brainstone --no-default-features \
  --features openai --example chat -- intro-1 'My name is Sam.'
```

Request IDs are scoped to a lane, so this new conversation can use `intro-1`.
To keep chatting, keep the same database and conversation ID and pass a new
request ID with each message.

## 2. An order-support agent with a Rust tool

[order_support.rs](../crates/brainstone/examples/order_support.rs) registers
a typed `LookupOrder` tool and asks the model to use it before reporting an order's status.
The tool reads a small fixture catalog: `BS-1001` is shipped and `BS-1002` is
processing. These are demo records, not a live store.

```sh
cargo run -p brainstone --no-default-features --features openai --example order_support -- \
  order-1 'Where is order BS-1001?'
cargo run -p brainstone --no-default-features --features openai --example order_support -- \
  order-2 'What about BS-1002?'
```

[order_lookup.rs](../crates/brainstone/examples/support/order_lookup.rs) uses
`typed_tool` with a Rust input struct and an async handler. `JsonSchema` derives
the advertised input schema; Serde decodes arguments before the handler runs.
The handler returns a serializable `Order` or a typed `LookupError`.

The registry keeps the handler and its definition together:

```rust
let tools = Registry::default().with_tool(orders::tool()?);
```

Replace the catalog lookup with your application's data source. Input validation
checks the schema and Serde shape. Business checks, such as whether an order
exists, belong in the handler. Success is JSON with `outcome: "success"` and
`value` containing the order.

| Code | Meaning |
| --- | --- |
| `invalid_tool_arguments` | Input does not match the derived schema or Rust input type |
| `invalid_arguments` | The order ID contains only whitespace |
| `order_not_found` | The fixture catalog has no matching ID |

An unknown order returns a structured failure to the model, which can ask the
user to check the ID. Try it with a new message:

```sh
cargo run -p brainstone --no-default-features --features openai --example order_support -- \
  order-3 'Where is order BS-9999?'
```

This tool only reads data, so it needs no side-effect ledger. If your tool
charges a card, sends a message, or changes a record, atomically save the
`ToolInvocation::require_idempotency_key()` with that effect and return the prior result on
retry. A crash can repeat an unfinished call. See the
[tool retry contract](../crates/runtime/README.md#leases-recovery-and-visibility).

## 3. A simulated refund with approval and recovery

The [refund example](../crates/brainstone/examples/refund.rs) runs without a
provider or credentials. A local model requests a simulated 500-cent refund.
An accepted `ToolContract` permits only `Refund`, requires approval, and requires
a committed successful tool result before the operation can complete.

```sh
cargo run -p brainstone --no-default-features --example refund -- --approve --crash
cargo run -p brainstone --no-default-features --example refund -- --approve
```

The first command exits with code 73 after the application ledger commits the
simulated refund, before the tool result commits. The second reopens
`refund-demo.db`, retries the tool with the same key, and returns its stored
receipt. Further runs read the terminal outcome. Pass another SQLite URL as the
last argument to repeat with fresh records.

`--approve` explicitly approves this fixed simulation. The program prints the
exact call and arguments, then records its decision through `decide_approval`.
Without the flag it exits before opening a database. It never contacts a payment
service. In a real application, show the stored approval request to an authorized
operator before recording their decision.

The effect ledger belongs to the example application. It commits the retry key
and refund together and rejects changed arguments under the same key. A real
payment service must provide equivalent idempotency or a way to reconcile an
uncertain result. See [typed tools and tool contracts](tools.md).

## How the parts fit

| Part | Provider-backed examples |
| --- | --- |
| `Model` | A `ResponsesModel` with your endpoint, credentials, and model ID |
| `Registry` | Empty for chat; one native tool for order support |
| `Runtime` | SQLite storage with default execution settings |
| `Host` | Durable acceptance and execution of one named operation |
| `Address` | Agent name as surface, `CONVERSATION_ID` as conversation, `main` as lane |
| `Prompt` | Address, request ID, system prompt, and message |

`accept` commits the prompt before work begins. `execute` drives that operation
or returns its stored outcome. The runner closes host admission and the database
even when execution returns an error. Provider failures produce a stored failed
outcome and a nonzero process exit. A retry of that same request reads that
failure too; use a new request ID when you intend a new attempt.

Changing a stored lane's system prompt through this strict acceptance path
returns `lane_configuration_conflict`. Start a new conversation when trying a
new prompt. Existing lanes retain their original system prompt.

## Keep a worker running

The examples run one message per process to make retries easy to inspect.
For HTTP handlers, a chat bot, or a queue consumer:

1. Compose one host and spawn `host.run()` for its worker pool.
2. Clone the host into handlers and accept prompts there.
3. Acknowledge the incoming event only after acceptance succeeds.
4. From a separate owner task, shut down the host, join its worker task, and
   close the resources your application opened.

Use `Host::admit` when new input should steer an active operation. `Host::accept`
queues a separate operation. The [composition guide](../crates/brainstone/README.md)
explains admission decorators, execution settings, and resource ownership.

For a network-free crash demonstration, run the
[recovery example](../crates/brainstone/examples/recover.rs). For prompts,
MCP servers, skills, and child agents loaded from a directory, use the
[configured example](../crates/brainstone/examples/compose.rs) and
[minimal agent directory](../examples/minimal-agent).

To build a coding agent, follow the [coding-agent guide](coding-agent.md).
It uses typed tools, approvals, checks, and crash recovery.
