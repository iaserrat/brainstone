# anthropic

A native Anthropic Messages API implementation of `agent::Model`.

The adapter sends streaming requests to `/v1/messages`. It preserves signed and
redacted thinking blocks, translates tool calls and results, and batches text
and thinking deltas through `agent::Progress` into provisional `ModelDelta` and
`ReasoningDelta` events. Requests send `thinking.display: "summarized"`, which
costs the same as `"omitted"` and returns readable thinking summaries to stream;
signatures and redacted thinking never become progress. Messages accepts tool ids
matching `^[a-zA-Z0-9_-]+$` only, so replayed ids are mapped one-to-one: `-`
becomes `--`, every other rejected byte `-` and two hex digits, and an empty id
`-`. An id with neither is sent unchanged. It retries connection failures
and retryable HTTP statuses with the same request body.

`AnthropicModel::new` accepts an `Endpoint`, model name, `Auth`, and positive
`max_tokens` value. `Endpoint::messages(url)` and `Endpoint::bedrock_mantle(region)`
stream SSE over reqwest; `Auth::ApiKey` sends `x-api-key` and `Auth::SigV4`
signs each request with credentials from `SigV4::load(region)`, which resolves
the standard AWS chain and caches temporary credentials until five minutes
before expiry. `Endpoint::bedrock_runtime(region, url, &auth)` builds an
official AWS SDK client for `InvokeModelWithResponseStream`; the body drops
`model` and `stream` and adds `anthropic_version`, and each event-stream chunk
is one Messages event. Bedrock exceptions map onto the same failure codes.
An invalid request whose message says the prompt is too long, on HTTP, in the
stream, or from Bedrock, is `context_window_exceeded` rather than
`invalid_request`, so a caller can compact instead of failing.
A stream that ends before `message_stop` is a `transport_error`; a tool call
cut off by `max_tokens` is kept and reported as `StopReason::MaxTokens`, never
`invalid_response`.
A connection that takes over thirty seconds, or a stream that sends nothing for
two minutes, is a `request_timeout`. A stream has no total deadline, so a long
turn alone never produces one.
Secrets never appear in `Debug` output.

Configuration codes: `invalid_endpoint`, `empty_model`, `empty_api_key`,
`empty_region`, `invalid_max_tokens`, `client_build_failed`. `SigV4` comes
from the [sigv4](../sigv4/README.md) crate; `SigV4::load` fails with
`empty_region` or `missing_credentials`. Credential resolution or signing failures at request
time end the turn with `authentication_failed`.

Set reasoning effort with `.with_effort(anthropic::Effort::High)`. Supported
values are `Low`, `Medium`, `High`, `XHigh`, and `Max`. If omitted, the adapter
leaves `output_config` out and lets the provider choose its default.

Tests use a local HTTP server. They do not call Anthropic.
