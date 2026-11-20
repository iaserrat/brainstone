//! Portable, content-free platform telemetry. Execution never waits for export.

#![deny(missing_docs)]
mod config;
mod error;
mod lifecycle;
mod logging;
mod logging_worker;
mod logging_writer;
mod metric_export;
mod providers;
mod safe_fields;
mod schema;
mod shutdown;
mod startup;
#[cfg(test)]
mod tests;
mod trace_export;

pub use error::Error;
pub use shutdown::Telemetry;
pub use startup::init;

/// The current W3C trace context, for transport metadata only. No baggage.
#[must_use]
pub fn traceparent() -> Option<String> {
    use opentelemetry::trace::TraceContextExt as _;
    let context = current_context();
    let span = context.span();
    let sc = span.span_context();
    sc.is_valid().then(|| {
        format!(
            "00-{}-{}-{:02x}",
            sc.trace_id(),
            sc.span_id(),
            sc.trace_flags().to_u8()
        )
    })
}

#[cfg(test)]
mod runtime_tests;

fn current_context() -> opentelemetry::Context {
    use opentelemetry::trace::TraceContextExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;
    let context = tracing::Span::current().context();
    if context.span().span_context().is_valid() {
        context
    } else {
        opentelemetry::Context::current()
    }
}

#[cfg(test)]
mod performance_tests;

#[cfg(test)]
mod logging_tests;
