use crate::{Error, config::Config};
use opentelemetry_otlp::{WithExportConfig as _, WithHttpConfig as _};
use opentelemetry_sdk::{
    Resource,
    metrics::SdkMeterProvider,
    trace::{BatchConfigBuilder, BatchSpanProcessor, SdkTracerProvider},
};
use std::time::Duration;

pub(crate) const EXPORT_TIMEOUT: Duration = Duration::from_secs(3);
pub(crate) const QUEUE_CAPACITY: usize = 1024;
pub(crate) const BATCH_SIZE: usize = 128;

pub(crate) struct Providers {
    pub(crate) resource: Resource,
    pub(crate) traces: Option<SdkTracerProvider>,
    pub(crate) metrics: SdkMeterProvider,
}
impl Providers {
    pub(crate) fn build(config: &Config) -> Result<Self, Error> {
        let resource = Resource::builder().build();
        let traces = config
            .traces
            .then(|| traces(resource.clone()))
            .transpose()?;
        let metrics = metrics(resource.clone(), config.metrics)?;
        Ok(Self {
            resource,
            traces,
            metrics,
        })
    }
}
fn traces(resource: Resource) -> Result<SdkTracerProvider, Error> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_timeout(EXPORT_TIMEOUT)
        .with_retry_policy(opentelemetry_otlp::RetryPolicy::disabled())
        .build()
        .map_err(|_| Error::Configuration)?;
    let config = BatchConfigBuilder::default()
        .with_max_queue_size(QUEUE_CAPACITY)
        .with_max_export_batch_size(BATCH_SIZE)
        .build();
    let processor = BatchSpanProcessor::builder(crate::trace_export::Exporter::new(exporter))
        .with_batch_config(config)
        .build();
    Ok(SdkTracerProvider::builder()
        .with_resource(resource)
        .with_span_processor(processor)
        .with_max_attributes_per_span(16)
        .with_max_events_per_span(0)
        .build())
}
fn metrics(resource: Resource, enabled: bool) -> Result<SdkMeterProvider, Error> {
    let builder = SdkMeterProvider::builder().with_resource(resource);
    if !enabled {
        return Ok(builder.build());
    }
    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_timeout(EXPORT_TIMEOUT)
        .with_retry_policy(opentelemetry_otlp::RetryPolicy::disabled())
        .build()
        .map_err(|_| Error::Configuration)?;
    Ok(builder
        .with_periodic_exporter(crate::metric_export::Exporter::new(exporter))
        .build())
}
