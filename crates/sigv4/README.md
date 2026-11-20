# sigv4

AWS `SigV4` credentials and request signing for Bedrock HTTP adapters. These
include Anthropic Messages on `bedrock-mantle` and OpenAI Responses on
`bedrock-runtime`.

`SigV4::load(region)` resolves the standard AWS credential chain once. It
checks the environment, profiles, SSO, web identity such as IRSA, and container
or instance roles. It caches temporary credentials until five minutes before
expiry.

`SigV4::headers(service, url, base, body)` signs one POST. Send exactly the
`base` headers and `body` bytes used to sign it.

| Code | Cause |
| --- | --- |
| `empty_region` | The region is blank |
| `missing_credentials` | No credential source is configured |
| `credentials_unavailable` | The chain failed to produce an identity at request time |
| `signing_failed` | The request could not be signed |

The adapters report the last two as `authentication_failed`.
