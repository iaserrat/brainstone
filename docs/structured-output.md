# Structured outputs

Attach a JSON Schema to the prompt's execution profile. Brainstone stores that
contract with accepted work, sends it through the provider's native structured
output API, and validates the final answer before committing a successful outcome.
This fragment assumes a `host` composed as in the quick start.

```rust
use brainstone::{Address, OutputSchema, Prompt};
use brainstone::agent::ExecutionProfile;
use serde_json::json;

let schema = OutputSchema::new("ticket", json!({
    "type": "object",
    "properties": {
        "category": {"type": "string", "enum": ["billing", "technical", "other"]},
        "urgent": {"type": "boolean"}
    },
    "required": ["category", "urgent"],
    "additionalProperties": false
}))?;
let profile = ExecutionProfile::default().with_output_schema(schema);
let prompt = Prompt::new(
    Address::new("support", "ticket-42", "main")?,
    "message-1",
    "Classify the support request.",
    "My payment failed twice."
)?.with_profile(profile);
let accepted = host.accept(&prompt).await?;
let operation = host.execute(accepted.operation.id).await?;
```

On `Outcome::Completed { text }`, deserialize `text` into your own type with
`serde_json::from_str::<YourType>(&text)`. The JSON Schema checks the document;
your Rust type must describe the same contract. The durable outcome keeps the
original JSON text, so retries return the exact committed answer.

## Validation and recovery

`OutputSchema::new` checks the name and compiles the schema before any model
call. Names contain 1 through 64 ASCII letters, digits, underscores, or hyphens.
Clones share the compiled validator. Deserialization repeats construction, so
stored schemas cannot bypass validation.

Validation supports local `$defs` references and checks built-in formats.
External schema retrieval is disabled, including file and network references.
Providers support different subsets of JSON Schema; a locally valid schema can
still receive a provider rejection. Brainstone returns that provider failure
without retrying as unrestricted prose.

A final answer that is malformed JSON, violates the schema, or stops at its
output-token limit fails with `structured_output_invalid`. It never becomes a
successful authoritative result. Tool calls and provisional streaming text are
not final documents and are not validated as answers. Successful silence is not
valid when the caller requires a JSON document.

Accepted work retains its schema across process exits. A crash after the
assistant response commits but before settlement resumes validation from that
record without calling the model again. Internal compaction summaries use a
profile without the final-answer schema; the agent's accepted contract remains
unchanged. Model selection preserves the output contract when it replaces model
and effort. Child agents have their own final-answer contracts; the parent's
schema does not constrain a child's response.

Strict acceptance rejects a different schema on an existing lane with
`lane_configuration_conflict`. Use a new conversation when changing the contract.

## Provider encoding

| Adapter | Native request field |
| --- | --- |
| OpenAI Chat Completions and compatible routers | `response_format.json_schema`, with `strict: true` |
| OpenAI Responses | `text.format`, with `strict: true` |
| Anthropic Messages, including its Bedrock paths | `output_config.format` |

The schema and tool definitions can share a request. Native support depends on
the selected model and endpoint. The small DeepSeek/OpenRouter evaluation found
that enabling a schema suppressed tool calls on the tested provider, including
when the task asked for one. Schema conformance does not prove that required
tools ran. See the [evaluation report](evaluations.md).

Without an `OutputSchema`, raw `RequestParameters` passthrough for
`response_format` and `text` is available. It does not pin or validate a
durable output contract. With a profile-owned schema, raw parameters cannot
replace the protocol's generated format field: a collision fails before HTTP
with `invalid_request`. Configure the durable schema through the profile.

| Code | Meaning |
| --- | --- |
| `invalid_output_schema` | Invalid schema name, schema syntax, or external reference |
| `structured_output_invalid` | The final document is malformed, mismatched, or truncated |
