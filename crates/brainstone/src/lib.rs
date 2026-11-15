//! Composition root for building durable agents in Rust.
//!
//! Parse [`Settings`], compose an [`Engine`] with [`build`], then drive it:
//! `admission.accept` stores a prompt durably, `host.run` drains accepted
//! prompts, and the store holds every committed outcome. Hang your own
//! interface off the `surface` crate's traits.
//!
//! # Example
//!
//! The whole lifecycle a surface needs, against the agent directory in the
//! README. It runs only with a database and a provider, so this test compiles
//! it rather than executing it.
//!
//! ```no_run
//! use brainstone::agent::ToolError;
//! use brainstone::host::Admission;
//! use brainstone::runtime::{Address, Destination, Prompt};
//! use brainstone::surface::{Destinations, PostRequest};
//! use brainstone::Settings;
//! use std::path::Path;
//!
//! struct MySurface;
//!
//! impl Destinations for MySurface {
//!     async fn resolve(&self, _: PostRequest<'_>) -> Result<Destination, ToolError> {
//!         unimplemented!("map a model-named conversation to a durable target")
//!     }
//! }
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let settings = Settings::from_map(&std::env::vars().collect(), Path::new("my-agent"))?;
//! let mut engine = brainstone::build(settings, Option::<MySurface>::None).await?;
//!
//! let address = Address::new("demo", "conversation-42", "main")?;
//! let prompt = Prompt::new(address, "request-1", "You are useful.", "Hello")?;
//! let id = engine.admission.accept(&prompt).await?;
//! let _outcome = engine.host.execute(id).await?;
//!
//! engine.host.shutdown().await;
//! engine.mcp.close().await;
//! engine.store.close().await;
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]
mod config_error;
mod core_model_access;
mod core_settings;
mod core_settings_access;
mod error;
mod logging;
mod model_fallback;
mod provider_effort;
mod provider_model;
mod provider_wire;
mod service_admission;
mod service_build;
mod service_core;
mod service_model;
mod service_model_compatible;
mod service_policy;
mod service_skills;
mod service_subagents;
mod service_tools;
mod settings;
mod settings_access;
mod settings_admission;
mod settings_aliases;
mod settings_debug;
mod settings_fallback;
mod settings_fallback_model;
mod settings_memory;
mod settings_model_selector;
mod settings_parse;
mod settings_prompt;
mod settings_provider;
mod settings_provider_parse;
mod settings_targets;
mod settings_tool_policy;
mod settings_tools;
mod settings_turn_limit;
mod target_capabilities;
mod target_limits;
mod target_names;
mod target_options;
mod target_parse;
mod target_payload;
mod target_validation;
mod tls;

pub use config_error::ConfigError;
pub use core_settings::CoreSettings;
pub use error::Error;
pub use logging::init_tracing;
pub use service_build::{Engine, build};
pub use settings::Settings;
pub use settings_fallback_model::FallbackModel;
pub use settings_provider::{BedrockApi, ModelProvider};

/// Install platform observability before starting agent work.
pub use telemetry::init as init_observability;

pub use agent;
pub use anthropic;
pub use host;
pub use mcp;
pub use media;
pub use memory;
pub use openai;
pub use policy;
pub use runtime;
pub use skills;
pub use subagents;
pub use surface;
