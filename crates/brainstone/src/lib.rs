//! Compose durable agents from a model, tools, and a runtime.
//!
//! [`Host`] owns admission and workers; [`Runtime`] owns durable execution.
//! Constructing a host starts no tasks and performs no I/O. Call `execute`
//! for one accepted operation, or `run` for a resident worker pool.
//!
//! ```no_run
//! use brainstone::{Address, Host, Model, Prompt, Registry, Runtime};
//! use brainstone::runtime::SqliteStore;
//!
//! # async fn example(model: impl Model) -> Result<(), Box<dyn std::error::Error>> {
//! let store = SqliteStore::connect("sqlite://agent.db").await?;
//! let host = Host::new(Runtime::new(store.clone()), model, Registry::default());
//! let address = Address::new("app", "conversation", "main")?;
//! let prompt = Prompt::new(address, "request-1", "Be useful.", "Hello")?;
//! let accepted = host.accept(&prompt).await?;
//! let operation = host.execute(accepted.operation.id).await?;
//! host.shutdown().await;
//! store.close().await;
//! # Ok(())
//! # }
//! ```
//!
//! The caller owns external resources and closes them after host shutdown.
//! With the default `configured` feature, `configured(settings)` assembles
//! the providers and capabilities from environment settings and an agent
//! directory. Its `Engine::shutdown` closes the resources it created.

#![deny(missing_docs)]

#[cfg(feature = "configured")]
mod configured;
#[cfg(feature = "configured")]
pub use configured::service_build::configured;
#[cfg(feature = "configured")]
pub use configured::{
    BedrockApi, ConfigError, CoreSettings, Engine, Error, FallbackModel, ModelProvider, Settings,
    build,
};

#[cfg(feature = "telemetry")]
mod logging;
#[cfg(feature = "telemetry")]
pub use logging::init_tracing;
#[cfg(feature = "telemetry")]
/// Install platform observability before starting agent work.
pub use telemetry::init as init_observability;

pub use agent;
pub use agent::{
    ExecutionProfile, Model, Outcome, OutputError, OutputSchema, Registry, Tool, ToolContract,
    ToolContractError, ToolError, ToolInvocation, ToolOutput, ToolRequest, TypedTool, typed_tool,
};
pub use host;
pub use host::{Admission, Host};
pub use media;
pub use runtime;
pub use runtime::{Address, Operation, OperationId, Prompt, Runtime};

#[cfg(feature = "anthropic")]
pub use anthropic;
#[cfg(feature = "mcp")]
pub use mcp;
#[cfg(feature = "memory")]
pub use memory;
#[cfg(feature = "openai")]
pub use openai;
#[cfg(feature = "policy")]
pub use policy;
#[cfg(feature = "skills")]
pub use skills;
#[cfg(feature = "subagents")]
pub use subagents;
#[cfg(feature = "surface")]
pub use surface;

#[cfg(feature = "telemetry")]
pub use telemetry;
