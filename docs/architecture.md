# Architecture

[Back to the README](../README.md)


Green is implemented and covered by local, SQLite, or PostgreSQL contract tests. Red is
not implemented. Blue is an external service.

```mermaid
flowchart TB
    subgraph Consumer["Your binary"]
        Surface["Your interface<br/>surface::Destinations · ThreadSource"]
    end

    subgraph Facade["Facade"]
        Brainstone["brainstone<br/>settings · composition · lifecycle"]
        Config["prompts/ · agents/ · skills/<br/>mcp.toml"]
        Metrics["telemetry<br/>OTLP traces · metrics · correlated JSON logs"]
    end

    subgraph Core["Core"]
        Host["host<br/>admission · workers · shutdown"]
        Runtime["runtime<br/>sessions · lanes · leases · journal<br/>recovery · delivery · compaction"]
        Agent["agent<br/>pure transition · model/tool loop · events"]
    end

    subgraph Capabilities["Capabilities"]
        MCP["MCP client<br/>stdio · Streamable HTTP · elicitation"]
        Skills["Agent Skills<br/>progressive disclosure"]
        Subagents["Project agents<br/>durable child sessions"]
        Memory["Cross-session memory<br/>save · recall · injection"]
        ProductionTools["Production tools"]
    end

    subgraph Policy["Autonomy policy"]
        Schedule["Scheduled work"]
        Compact["Automatic compaction"]
        Steering["Mid-turn steering"]
        Delegation["Model-directed delegation"]
    end

    subgraph Providers["Model adapters"]
        Anthropic["Anthropic Messages"]
        OpenAI["OpenAI Responses · Chat Completions<br/>shared transport · explicit target capabilities"]
    end

    Postgres["PostgreSQL<br/>events · leases · delivery cursors<br/>interactions · child runs · schedules · memories"]
    SQLite["SQLite<br/>same durable contracts · local WAL"]
    AnthropicAPI["Anthropic API"]
    OpenAIAPI["OpenAI API"]
    MCPServers["MCP servers"]

    Config --> Brainstone
    Surface --> Brainstone
    Brainstone --> Host
    Host --> Runtime
    Runtime --> Agent
    Runtime --> Postgres
    Runtime --> SQLite
    Agent --> Anthropic
    Agent --> OpenAI
    Agent --> MCP
    Agent --> Skills
    Agent --> Subagents
    Agent --> Memory
    Memory --> Runtime
    Agent -.-> ProductionTools
    MCP <-->|"MCP"| MCPServers
    Subagents --> Runtime
    Schedule --> Runtime
    Schedule -.->|"where it may post"| Surface
    Compact --> Runtime
    Steering --> Runtime
    Delegation --> Subagents
    Anthropic --> AnthropicAPI
    OpenAI --> OpenAIAPI
    Brainstone --> Metrics

    classDef present fill:#163d2a,stroke:#52c788,color:#ffffff
    classDef missing fill:#432525,stroke:#e06c75,color:#ffffff
    classDef external fill:#26344d,stroke:#70a5eb,color:#ffffff

    class Surface,Brainstone,Config,Host,Runtime,Agent,MCP,Skills,Subagents,Memory,Schedule,Compact,Steering,Delegation,Anthropic,OpenAI,Postgres,SQLite,Metrics present
    class ProductionTools missing
    class AnthropicAPI,OpenAIAPI,MCPServers external
```
## Crates

| Crate | Responsibility |
| --- | --- |
| [agent](../crates/agent) | Model/tool loop and extension interfaces |
| [runtime](../crates/runtime) | Durable operations, shared record codecs, and PostgreSQL/SQLite storage |
| [host](../crates/host) | Prompt admission, workers, and shutdown |
| [telemetry](../crates/telemetry) | Safe correlated logs, portable OTLP traces/metrics, exporter workers and shutdown |
| [brainstone](../crates/brainstone) | The composition root: settings, engine build, and re-exports of every crate below |
| [media](../crates/media) | Model-input images (validation, bounds, fitting) shared by surfaces and MCP, the stable reference a reply uses to attach a tool-returned image or file, and `CreateFile`, which lets any agent write a file for the reply |
| [surface](../crates/surface) | Surface-neutral thread input (attachments, caps, validation, rendering) and the `Destinations` seam a surface implements so schedules can post in its conversations |
| [anthropic](../crates/anthropic), [openai](../crates/openai) | Model adapters |
| [sigv4](../crates/sigv4) | AWS SigV4 identity and request signing shared by the Bedrock paths of both adapters |
| [mcp](../crates/mcp), [skills](../crates/skills), [subagents](../crates/subagents), [memory](../crates/memory), [policy](../crates/policy) | Agent capabilities |


## Observability ownership

The `agent::observation` seam describes finite execution boundaries using
`tracing`; it has no exporter, backend or durability policy. Runtime, provider,
MCP and surface owners annotate actual outcomes. The `telemetry` crate owns the
OpenTelemetry SDK, OTLP transport, safe JSON formatting, fixed metric dimensions,
bounded workers and shutdown. The composition root owns the guard's process lifetime. Provider
and MCP adapters use the crate's content-free W3C context accessor for outbound
propagation. No observability state is added to journal records or used by the
execution/recovery state machines. See the [contract](../crates/telemetry/README.md).

## Backend and model selection

The composition root selects one `DurableStore` at startup and passes it
through host admission, memory, MCP, scheduling, and subagents. The core remains generic over its existing store traits. Database
adapters own SQL, locking, connection setup, migrations, and transient-error
classification. Shared codecs own the meaning of stored records.

`ProviderModel` scopes replay to a stable target and model. Custom endpoint
identity includes the protocol and an endpoint digest, without recording its
credentials. Responses and Chat Completions share transport policy. Capability
validation precedes I/O, and fallback validates its candidate before sending
history. A model's declared window reaches compaction through the same model
seam. No provider-specific branching enters the agent loop.


## Authorization and operator boundary

Optional tool policy lives at runtime dispatch, before `ToolStarted` and before
native/MCP/subagent effects. PostgreSQL and SQLite own an immutable approval
ledger bound to the durable call and current executor/policy configuration.
Pending approval waits share the normal heartbeat and steering lifecycle;
unsupported stores fail closed only when authorization is required. Operator
recovery calls the existing host with an existing operation ID and retains
normal lease fencing. Read-only metadata and explicit sensitive journal reads
are separate operations.
