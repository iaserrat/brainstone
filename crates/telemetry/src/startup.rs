use crate::{Error, Telemetry, config::Config, providers::Providers};
use opentelemetry::trace::TracerProvider as _;
use tracing_subscriber::{EnvFilter, prelude::*};

/// Install safe JSON stderr logs and optional OTLP HTTP/protobuf traces/metrics.
/// Call once before workers. Reads `OTEL_*` and `RUST_LOG`; no endpoint means no export.
/// Invalid local configuration or an existing subscriber returns a stable error.
/// Keep the guard until workers drain; remote failures never fail execution.
///
/// # Errors
/// Returns `telemetry_configuration` or `telemetry_already_installed`.
/// Requires a Tokio runtime only for `Telemetry::shutdown`, not initialization.
pub fn init() -> Result<Telemetry, Error> {
    let config = Config::from_env()?;
    let providers = Providers::build(&config)?;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let (writer, logs) = crate::logging_worker::start(std::io::stderr())?;
    install(&providers, writer, filter)?;
    Ok(Telemetry { providers, logs })
}

fn install(
    providers: &Providers,
    writer: crate::logging_writer::Writer,
    filter: EnvFilter,
) -> Result<(), Error> {
    let traces = providers.traces.as_ref().map(trace_layer);
    tracing_subscriber::registry()
        .with(traces)
        .with(
            crate::lifecycle::Layer::new(&providers.metrics).with_filter(
                tracing_subscriber::filter::filter_fn(crate::schema::enabled),
            ),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .event_format(crate::logging::SafeFormat::new(&providers.resource))
                .with_writer(writer)
                .with_filter(filter),
        )
        .try_init()
        .map_err(|_| Error::AlreadyInstalled)?;
    Ok(())
}

fn trace_layer(
    provider: &opentelemetry_sdk::trace::SdkTracerProvider,
) -> impl tracing_subscriber::Layer<tracing_subscriber::Registry> + use<> {
    tracing_opentelemetry::layer()
        .with_tracer(provider.tracer("brainstone"))
        .with_context_activation(true)
        .with_location(false)
        .with_threads(false)
        .with_target(false)
        .with_filter(tracing_subscriber::filter::filter_fn(
            crate::schema::enabled,
        ))
}
