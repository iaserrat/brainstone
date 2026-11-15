# sigv4

AWS `SigV4` identity and request signing for every adapter that reaches
Amazon Bedrock over plain HTTP: the Anthropic Messages API on
`bedrock-mantle` and the OpenAI Responses API on `bedrock-runtime`.

`SigV4::load(region)` resolves the standard AWS credential chain
(environment, profile, SSO, web identity such as IRSA, container and instance
roles) once and caches temporary credentials until five minutes before they
expire. `SigV4::headers(service, url, base, body)` signs one POST; the caller
must send exactly the `base` headers and `body` bytes it signed.

| Code | Cause |
| --- | --- |
| `empty_region` | The region is blank |
| `missing_credentials` | No credential source is configured |
| `credentials_unavailable` | The chain failed to produce an identity at request time |
| `signing_failed` | The request could not be signed |

The adapters report the last two as `authentication_failed`.
