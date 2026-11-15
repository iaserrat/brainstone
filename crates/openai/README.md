# openai

A minimal production implementation of `agent::Model` for the OpenAI
Responses API and compatible endpoints. It sends a complete stateless thread
on every call, sets `store: false`, preserves provider reasoning items
verbatim, streams provisional text, and returns one settled
`agent::ModelResponse`.

## Usage

OpenAI uses `OPENAI_API_KEY`:

```rust
let model = openai::ResponsesModel::openai("gpt-5.6-terra")?
    .with_effort(openai::Effort::High);
```

An OpenAI Responses-compatible service takes an exact endpoint and bearer
token. Read the token from the environment or a secret manager; do not put it
in source or configuration files.

```rust
let model = openai::ResponsesModel::new(
    "https://provider.example/v1/responses",
    "provider-model-id",
    bearer_token,
)?;
```

Amazon Bedrock serves the Responses API on `bedrock-runtime` for GPT models
named by a system inference profile. It takes a Bedrock API key
(`Auth::Bearer`) or AWS IAM credentials (`Auth::SigV4`, signed for service
`bedrock`). `base` overrides the endpoint origin, for a VPC endpoint:

```rust
let identity = openai::SigV4::load("ca-central-1").await?;
let model = openai::ResponsesModel::bedrock_runtime(
    "ca-central-1", None, "us.openai.gpt-5.6-sol", openai::Auth::SigV4(identity),
)?;
```

GPT OSS is served with the Responses API only on `bedrock-mantle`, which is
not supported here: it rejects tool results that carry images, and the model
reads text only.

The model is `Clone + Send + Sync`; clones share reqwest's connection pool.
Its `Debug` output redacts credentials. `.with_max_output_tokens(n)` caps each
response, reasoning included.

`Effort` supports `None`, `Minimal`, `Low`, `Medium`, `High`, `XHigh`, and
`Max`. Model support varies. If effort is omitted, the adapter leaves
`reasoning` out and lets the provider choose its default. With an effort set,
the request also asks for `include: ["reasoning.encrypted_content"]`: with
`store: false`, an encrypted reasoning item is the only form the provider
accepts back on the next turn.

Configuration errors also expose stable codes: `invalid_endpoint`,
`empty_model`, `empty_bearer`, `empty_region`, `missing_environment`, and
`client_build_failed`. A `SigV4` identity that cannot be resolved or signed at
request time ends the turn with `authentication_failed`.

## Wire contract

- `Thread::system` becomes `instructions`.
- Every user and assistant turn is replayed in `input`.
- A deliberate `Message::Summary` is replayed as a labelled user-context item.
- Calls become `function_call`; results become `function_call_output`.
- Tool definitions are validated and translated from `input_schema` to
  Responses `parameters`.
- Opaque reasoning output is stored in `agent::Reasoning::Opaque` with the
  `openai-responses` provider tag and replayed byte-for-byte at the JSON value
  level. Reasoning from another provider is rejected, never dropped.
- `store` is always `false`; remote response IDs are not durability state.

`generate_stream` requests SSE and accepts `response.output_text.delta`,
`response.reasoning_summary_text.delta`, and `response.reasoning_text.delta`
until a terminal `response.completed`, `response.incomplete`, or
`response.failed` event. `agent::Progress` buffers them to at most 512
characters or 100 milliseconds before emitting `ModelDelta` or
`ReasoningDelta`; this avoids one durable write per provider token without
making the UI feel buffered. `with_reasoning_summary` asks for summaries
(`reasoning.summary: "auto"`) whenever effort is set; `OpenAI` rejects that for
unverified organizations, so it is off by default. The terminal event is decoded into the same
settled response used by `generate`. Deltas are presentation only and never
enter replayed provider context.

## Timeouts and retries

Connection setup is bounded to 30 seconds. Response silence is bounded to
120 seconds; a stream can continue while bytes arrive. HTTP 408, 409, 429, 500, 502, 503, and
504 responses retry at most three times (four total attempts). A numeric
`Retry-After-Ms` or numeric `Retry-After` is honored up to 60 seconds;
otherwise delays are 250ms, 500ms, and 1s. Redirects are rejected so prompts
and credentials cannot cross to an unexpected origin.
Retries happen only after an initial retryable HTTP status, before any stream
delta is accepted. Transport or SSE body failures are not retried because a
dropped POST may already have reached the provider and compatible endpoints do
not share a documented idempotency contract.

## Stable failure codes

Failures are returned as `StopReason::Error { code, message }`. Consumers
branch on `code`; `message` is human detail.

| Code | Cause |
|---|---|
| `invalid_tool_definition` | A tool lacks a valid name, description, or object schema |
| `foreign_reasoning` | The thread contains reasoning this provider cannot replay |
| `request_timeout` | HTTP timeout or exhausted 408 responses |
| `transport_error` | DNS, TLS, connection, or response-body transport failure, including a stream that ends before its terminal event |
| `invalid_request` | HTTP 400, 413, or 422 |
| `context_window_exceeded` | OpenAI's error code `context_length_exceeded`, on an HTTP error, a failed response, or a stream error event |
| `authentication_failed` | HTTP 401 |
| `permission_denied` | HTTP 403 |
| `model_or_endpoint_not_found` | HTTP 404 |
| `request_conflict` | Exhausted HTTP 409 responses |
| `rate_limited` | Exhausted HTTP 429 responses |
| `provider_unavailable` | Exhausted HTTP 5xx responses |
| `http_error` | Another non-success HTTP status |
| `invalid_response` | Malformed, unknown, or non-terminal provider JSON |
| `content_filtered` | Incomplete output explicitly reports `content_filter` |
| `provider_failed` | A terminal Responses object has status `failed` |
| `request_cancelled` | A terminal Responses object has status `cancelled` |
| `refused` | Assistant message content is a refusal |

## Building

Requires Rust 1.95.0 or newer, edition 2024. From the workspace root:

```sh
cargo xtask gate
cargo xtask audit
```

## Compatible protocols

`ResponsesModel` retains its constructors and wire behavior.
`ChatCompletionsModel` implements the same `agent::Model` seam using complete
stateless Chat Completions history. Both share authentication, headers,
timeouts, redirect refusal, HTTP-status retries, and stable failure codes.
Both accept `unauthenticated(endpoint, model)` for local servers,
`with_headers`, and `with_idle_timeout`.

Chat Completions translates tools into nested function definitions, preserves
call IDs and JSON arguments, and replays tool results under `role: tool`.
It preserves `reasoning`, `reasoning_content`, and `reasoning_details` as opaque
`openai-chat-completions` state. Another protocol's reasoning is rejected.
Streaming assembles indexed call and reasoning fragments before settlement.
An empty terminal choice repeated with usage is accepted only when its finish
reason agrees; late content or tool calls remain invalid.
It requires a finish reason and `[DONE]`; a cut stream returns no partial calls
or answer. Stream bodies are bounded to 16 MiB. Output cut by `length` returns
`MaxTokens`, so the agent never executes its calls.

`RequestParameters::new` validates provider-specific options for either
protocol. `with_parameters` adds them without replacing conversation, tools,
streaming, or remote-state fields. Reserved keys fail with
`invalid_request_parameter`; a duplicate adapter-owned key fails with
`invalid_request`. Values stay out of parameter diagnostics. For a Chat model
that requires `max_completion_tokens`, supply that parameter and omit
`with_max_output_tokens`, which sends `max_tokens`.

Endpoint compatibility does not establish model capabilities. The application
uses `agent::CapabilityModel` to enforce each configured target's contract.
Library callers can use that wrapper too. Protocol tests share HTTP fixtures
and cover hosted and unauthenticated targets, tool loops, opaque reasoning,
retries, stream cuts, and truncation.

Chat Completions streaming emits `ReasoningDelta` for visible `reasoning`,
`reasoning_content`, and `reasoning_details` text or summaries. Aliases carried
alongside structured details are displayed once. Encrypted data and signatures
stay opaque in the settled assistant message and are never progress text.
Both protocols batch reasoning and text through `agent::Progress`.
