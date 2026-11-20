# Security policy

Brainstone runs tools, child processes, and network clients for a model. A
defect can expose the operator's credentials or use them without permission.

## Reporting

Report vulnerabilities through GitHub's private advisory form on this
repository ("Security" tab, "Report a vulnerability"). Do not open a public
issue, and do not include credentials in a report.

Include the version or commit, the backend (SQLite or PostgreSQL), the
provider, and the smallest reproduction you can build. This project offers
no paid support or guaranteed response time. The maintainers acknowledge
reports as time permits.

## Scope

In scope: the durable runtime (admission, leases, recovery, journal),
approval binding, tool execution boundaries, MCP child-process and Streamable
HTTP transports, credential handling for the Anthropic, Bedrock, and
OpenAI-compatible adapters, and the integrity of stored records.

Out of scope: the correctness of model output, defects inside third-party MCP
servers you configure, and misconfiguration documented as unsupported in
[`docs/operations.md`](docs/operations.md).

## Supported versions

Before version 1.0, maintainers support only the default branch. Fixes land
there without backports to tags.
