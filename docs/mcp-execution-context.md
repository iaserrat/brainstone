# Identity-bound MCP tools

Configure requirements in the operator-owned `mcp.toml`:

```toml
[mcp_servers.personal]
command = "personal-mcp"
args = ["--stdio"]
access = { authenticated_actor = true, private_conversation = true }
```

The same `access` table works with Streamable HTTP. Both flags default to
false, preserving unrestricted integrations. `private_conversation` implies a valid
authenticated actor. Unknown configuration keys are rejected. Requirements
apply to every tool on that server, inside its `Tool::call` implementation,
before transport access. Guessed names, stale definitions, direct execution,
and reconnection do not bypass the policy. Other native tools can reuse
`agent::AccessPolicy::check` inside their own execution method.

## Request contract, version 1

Every adapted `tools/call` has runtime-owned request metadata, separate from
model-controlled `arguments`:

```json
{
  "method": "tools/call",
  "params": {
    "name": "list_items",
    "arguments": {},
    "_meta": {
      "io.brainstone/idempotencyKey": "operation/42/step/0/call/call_1",
      "io.brainstone/executionContext": {
        "version": 1,
        "identity": {
          "provider": "app",
          "tenant": "acme",
          "actor": "user-456",
          "conversation": "dm-789",
          "private": true
        }
      }
    }
  }
}
```

`identity` is null for executions without an authenticated principal. Only
non-durable drivers omit the idempotency key. `private` is true only when the
conversation is readable by this principal alone; a surface that does not
model visibility sends `false`, and personal-data features stay off.
Missing/blank principal components never satisfy restrictive access. Consumers
must reject unsupported versions and unknown identity fields. They cannot
use these fields to infer authorization. No identity field in `arguments`, including a nested `_meta`,
is authoritative.

A consuming MCP binary should:

1. Authenticate its transport peer as an authorized Brainstone runtime.
2. Read `_meta["io.brainstone/executionContext"]`, require version 1, and validate
   its own identity/conversation policy.
3. Use `(provider, tenant, actor)` as the owner key. Qualify conversation IDs
   with provider and tenant too. Local user IDs alone are not globally unique.
4. Apply ownership predicates to every row/resource lookup and mutation,
   including model-supplied resource IDs. Runtime admission is not an IDOR check.
5. Deduplicate durable writes using the runtime idempotency key and the owner.
   Capture identity in each asynchronous task; never use a process-global user.

In Rust `rmcp` servers the request metadata is available in
`RequestContext<RoleServer>.meta.0.0`. The JSON identity can be decoded into
`agent::ExecutionIdentity` if sharing that crate, or an equivalent strict local
struct. Authorization must precede the domain query. Tool results and domain
error schemas remain the consuming server's responsibility.

The contract applies to stdio, Streamable HTTP, and `Client` byte transports.
Multi-round input retries and task get/update requests retain the invocation's
metadata. Reconnect reuses that call's parameters. The client serializes calls
per shared server for existing elicitation routing; identity is carried in the
request, never stored as mutable session state. Raw SDK `peer()` calls are a
trusted embedding escape hatch, not an identity-propagating tool API.

## Transport trust boundary

Metadata asserts identity; it does not sign or authenticate a request. A
private stdio child can trust its parent when the runtime alone controls its
pipes and launch. HTTP servers must authenticate and authorize the runtime
using a bearer token, gateway mTLS, or an equivalent boundary. Protect transit
and prevent other clients from supplying trusted metadata. A publicly reachable unauthenticated server cannot trust this JSON.
Do not grant untrusted plugins, shell tools, or processes access to runtime
credentials, database records, or MCP pipes. Brainstone does not sandbox trusted code.

## Ingress and durable inheritance

A surface's identity comes from its own authenticated ingress; the surface
decides what authenticates a principal and asserts the result. The actor is an
opaque principal id, never a name, prompt instruction, or argument. Unknown
facts are not upgraded to a stronger conversation kind. Synthetic or
machine-originated posts carry no authenticated human identity and cannot call
identity-restricted tools.
An embedding surface may use `Prompt::with_identity` only after authentication;
this Rust API is a trusted-code boundary, not an authenticator.

Operations persist identity independently of model text and execution profiles.
Restart claims restore the exact stored value; malformed/unknown stored identity
fails as `corrupt_record`. Identity participates in duplicate request matching.
A different caller's message queues a new operation instead of steering
an active operation under someone else's authority.

Initial children, follow-up child messages, nested children, and background
completion notifications inherit the original stored parent's identity. Child
control additionally requires the caller's identity to match the target's
parent. A child cannot choose a different principal through its task/config.
Scheduled operations inherit their origin identity and stored delivery target.
Children without a delivery target cannot invent one through scheduling.
Identity is an acceptance-time snapshot: no live membership/revocation refresh
is performed on retry or scheduled execution. Consumers requiring live access
checks must perform them themselves. Existing tool/agent allowlists still apply.

## Histories and memory

Conversation keys include provider/tenant, and direct keys include
the owner. Lane admission returns `execution_identity_conflict` for incompatible
identities, including a mix of unauthenticated and authenticated operations.
Lane histories and compaction summaries therefore cannot cross these boundaries.
Shared conversation histories remain shared: `authenticated_actor` alone does not
make the returned information private. Mark the conversation `private` for
personal data.

Trusted memory scopes include the workspace. Children resolve the origin's
memory identity, not a synthetic subagent user. User memory injection is skipped
in authenticated shared conversations and user-memory tools return
`memory_private_conversation_required` there. Conversation and surface memory
are shared within their configured scope. Use conversation or user scope for
personal agents and avoid surface-wide memory. Unrestricted integrations can
still expose anything their credentials permit. Review enabled tools before
offering private personal behavior.

There is no application-level shared model-response cache. Provider context
appends within each lane. This does not guarantee provider cache isolation or
change provider retention policies.

Identity columns are nullable and never infer identity from prompt text or an
actor name. New traffic uses scoped conversation/history and memory keys.
Records written without a trusted identity stay untrusted: restrictive tools
fail closed for them. Do not roll back to a build that ignores identity after
storing one.

Stable policy errors are `authenticated_actor_required` and
`private_conversation_required`, carried by the existing structured tool error document
(`error` plus human-readable `message`). Runtime identity conflicts use
`execution_identity_conflict`; invalid persisted records use `corrupt_record`.
