# anthropic

A native Anthropic Messages API implementation of `agent::Model`.

The adapter sends streaming requests to `/v1/messages`. It preserves signed and
redacted thinking blocks, translates tool calls and results, and batches text
and thinking deltas through `agent::Progress` into provisional `ModelDelta` and
`ReasoningDelta` events. Requests send `thinking.display: "summarized"`, which
costs the same as `"omitted"` and returns readable thinking summaries to stream;
signatures and redacted thinking never appear in progress events.

Messages accepts only tool IDs matching `^[a-zA-Z0-9_-]+$`. The adapter maps
replayed IDs one-to-one: `-` becomes `--`, each other rejected byte becomes
`-` followed by two hex digits, and an empty ID becomes `-`. Other IDs pass
through unchanged. Connection failures and retryable HTTP statuses reuse the
same request body.

`AnthropicModel::new` accepts an `Endpoint`, model name, `Auth`, and positive
`max_tokens` value. `Endpoint::messages(url)` and `Endpoint::bedrock_mantle(region)`
stream SSE over reqwest; `Auth::ApiKey` sends `x-api-key` and `Auth::SigV4`
signs each request with credentials from `SigV4::load(region)`, which resolves
the standard AWS chain and caches temporary credentials until five minutes
before expiry. `Endpoint::bedrock_runtime(region, url, &auth)` builds an
official AWS SDK client for `InvokeModelWithResponseStream`; the body drops
`model` and `stream` and adds `anthropic_version`, and each event-stream chunk
is one Messages event. Bedrock exceptions map onto the same failure codes.
If HTTP, stream, or Bedrock errors report an overlong prompt, the adapter
returns `context_window_exceeded` instead of `invalid_request`. The caller
can then compact and retry.
A stream that ends before `message_stop` is a `transport_error`; a tool call
cut off by `max_tokens` is kept and reported as `StopReason::MaxTokens`, never
`invalid_response`.
Connections time out after thirty seconds. A stream with no bytes for two
minutes returns `request_timeout`. Streams have no total deadline; long turns
can continue while bytes arrive.
Secrets never appear in `Debug` output.

Configuration codes: `invalid_endpoint`, `empty_model`, `empty_api_key`,
`empty_region`, `invalid_max_tokens`, `client_build_failed`. `SigV4` comes
from the [sigv4](../sigv4/README.md) crate; `SigV4::load` fails with
`empty_region` or `missing_credentials`. Credential resolution or signing failures at request
time end the turn with `authentication_failed`.

Set reasoning effort with `.with_effort(anthropic::Effort::High)`. Supported
values are `Low`, `Medium`, `High`, `XHigh`, and `Max`. If neither effort nor
an output schema is configured, the adapter leaves `output_config` out and lets
the provider choose its default.

Tests use a local HTTP server. They do not call Anthropic.

## Structured final output

An `agent::OutputSchema` on the thread's durable `ExecutionProfile` is sent as
`output_config.format` with `type: "json_schema"` and the original schema.
Effort and format remain independent fields of `output_config`: either may be
configured alone, or both together. Messages and Bedrock requests use the same
schema encoding. Tool definitions coexist with the final-output contract;
setting a schema does not force a tool choice.

An endpoint or model that rejects the schema reports its ordinary provider
failure. The adapter never drops the schema or falls back to plain text.
Runtime validation still checks the returned final text before authoritative
settlement. Without a schema, existing effort and text-output requests keep
their previous wire shape.
