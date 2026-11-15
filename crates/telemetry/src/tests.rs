// The cases assert the wire-facing schema and abandoned-future semantics.
#![allow(clippy::too_many_lines)]
use super::*;

#[test]
fn lifecycle_metrics_are_bounded_and_independent_of_trace_sampling() {
    use opentelemetry_sdk::metrics::{InMemoryMetricExporter, SdkMeterProvider};
    use tracing_subscriber::prelude::*;
    let exporter = InMemoryMetricExporter::default();
    let provider = SdkMeterProvider::builder()
        .with_periodic_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry().with(lifecycle::Layer::new(&provider));
    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!(target: "brainstone::telemetry", "work", kind = "tool", outcome = tracing::field::Empty, operation_id = 42);
        span.record("outcome", "ok");
        drop(span);
        drop(
            tracing::info_span!(target: "brainstone::telemetry", "work", kind = "model", outcome = tracing::field::Empty),
        );
        drop(tracing::info_span!("untrusted", secret = "PATIENT-DATA"));
    });
    provider.force_flush().unwrap();
    let metrics = format!("{:?}", exporter.get_finished_metrics().unwrap());
    assert!(metrics.contains("brainstone.work.duration"));
    assert!(metrics.contains("abandoned"));
    assert!(metrics.contains("ok"));
    assert!(!metrics.contains("operation_id"));
    assert!(!metrics.contains("PATIENT-DATA"));
}

#[test]
fn explicit_configuration_is_validated_without_exposing_values() {
    for (values, valid, enabled) in [
        (vec![], true, false),
        (
            vec![("OTEL_EXPORTER_OTLP_ENDPOINT", "http://localhost:4318")],
            true,
            true,
        ),
        (vec![("OTEL_EXPORTER_OTLP_PROTOCOL", "grpc")], false, false),
        (vec![("OTEL_TRACES_EXPORTER", "unknown")], false, false),
        (
            vec![
                ("OTEL_TRACES_SAMPLER", "traceidratio"),
                ("OTEL_TRACES_SAMPLER_ARG", "0.25"),
            ],
            true,
            false,
        ),
        (
            vec![
                ("OTEL_TRACES_SAMPLER", "traceidratio"),
                ("OTEL_TRACES_SAMPLER_ARG", "NaN"),
            ],
            false,
            false,
        ),
        (
            vec![
                ("OTEL_TRACES_SAMPLER", "parentbased_traceidratio"),
                ("OTEL_TRACES_SAMPLER_ARG", "1.01"),
            ],
            false,
            false,
        ),
        (
            vec![("OTEL_TRACES_SAMPLER", "PATIENT-SECRET")],
            false,
            false,
        ),
        (
            vec![
                ("OTEL_SDK_DISABLED", "true"),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", "bad"),
            ],
            true,
            false,
        ),
    ] {
        let result = config::Config::read(|key| {
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_owned())
        });
        assert_eq!(result.is_ok(), valid);
        if let Ok(config) = result {
            assert_eq!(config.traces, enabled);
        } else {
            assert_eq!(result.err(), Some(Error::Configuration));
        }
    }
    assert_eq!(Error::Configuration.code(), "telemetry_configuration");
    assert_eq!(
        Error::AlreadyInstalled.code(),
        "telemetry_already_installed"
    );
}
