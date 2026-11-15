# Security policy

Brainstone runs tools, child processes, and network clients on behalf of a
model, so a defect in it can act with the operator's credentials.

## Reporting

Report vulnerabilities through GitHub's private advisory form on this
repository ("Security" tab, "Report a vulnerability"). Do not open a public
issue, and do not include credentials in a report.

Include the version or commit, the backend (SQLite or PostgreSQL), the
provider, and the smallest reproduction you can build. Reports are acknowledged
on a best-effort basis; this project has no paid support and no response-time
guarantee.

## Scope

In scope: the durable runtime (admission, leases, recovery, journal),
approval binding, tool execution boundaries, MCP child-process and Streamable
HTTP transports, credential handling for the Anthropic, Bedrock, and
OpenAI-compatible adapters, and the integrity of stored records.

Out of scope: the correctness of model output, defects inside third-party MCP
servers you configure, and misconfiguration documented as unsupported in
[`docs/operations.md`](docs/operations.md).

## Supported versions

Pre-1.0: only the default branch is supported. Fixes land there and are not
backported to tags.
