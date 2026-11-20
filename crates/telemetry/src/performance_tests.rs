#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Optional measured overhead of the complete local observation pipeline.
// The benchmark keeps setup and the measured scenario together.
#![allow(clippy::too_many_lines)]
use agent::{Model, ModelResponse, Registry, StopReason, Thread};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::{
    metrics::{InMemoryMetricExporter, SdkMeterProvider},
    trace::{InMemorySpanExporter, SdkTracerProvider},
};
use runtime::{Address, MemoryStore, Prompt, Runtime};
use std::time::Instant;
use tracing_subscriber::prelude::*;

#[tokio::test]
#[ignore = "local performance measurement"]
async fn operation_latency_observed() {
    let traces = SdkTracerProvider::builder()
        .with_batch_exporter(InMemorySpanExporter::default())
        .build();
    let metrics = SdkMeterProvider::builder()
        .with_periodic_exporter(InMemoryMetricExporter::default())
        .build();
    let (writer, logs) = crate::logging_worker::start(std::io::sink()).unwrap();
    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(traces.tracer("brainstone"))
                .with_context_activation(true)
                .with_filter(tracing_subscriber::filter::filter_fn(
                    crate::schema::enabled,
                )),
        )
        .with(crate::lifecycle::Layer::new(&metrics).with_filter(
            tracing_subscriber::filter::filter_fn(crate::schema::enabled),
        ))
        .with(
            tracing_subscriber::fmt::layer()
                .event_format(crate::logging::SafeFormat::new(
                    &opentelemetry_sdk::Resource::builder().build(),
                ))
                .with_writer(writer),
        );
    let guard = tracing::subscriber::set_default(subscriber);
    let mut times = Vec::with_capacity(10_100);
    for _ in 0..10_100 {
        let runtime = Runtime::new(MemoryStore::default());
        let prompt = Prompt::new(
            Address::new("test", "channel", "thread").unwrap(),
            "event",
            "system",
            "prompt",
        )
        .unwrap();
        let started = Instant::now();
        runtime.accept(&prompt).await.unwrap();
        runtime
            .run_next(&Answer, &Registry::default())
            .await
            .unwrap()
            .unwrap();
        times.push(started.elapsed().as_nanos());
    }
    let mut times = times.split_off(100);
    times.sort_unstable();
    println!(
        "observed operation latency ns p50={} p95={}",
        times[5_000], times[9_500]
    );
    drop(guard);
    tokio::task::spawn_blocking(move || {
        traces.shutdown().unwrap();
        metrics.shutdown().unwrap();
        logs.shutdown();
    })
    .await
    .unwrap();
}
struct Answer;
impl Model for Answer {
    async fn generate(&self, _: &Thread, _: &[serde_json::Value]) -> ModelResponse {
        ModelResponse {
            reasoning: vec![],
            text: "ok".into(),
            tool_calls: vec![],
            stop: StopReason::EndTurn,
            model: None,
        }
    }
}
