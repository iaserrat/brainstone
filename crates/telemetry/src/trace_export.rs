use opentelemetry_sdk::{
    Resource,
    error::OTelSdkResult,
    trace::{SpanData, SpanExporter},
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct Exporter {
    inner: opentelemetry_otlp::SpanExporter,
    warned: AtomicBool,
}
impl Exporter {
    pub(crate) fn new(inner: opentelemetry_otlp::SpanExporter) -> Self {
        Self {
            inner,
            warned: AtomicBool::new(false),
        }
    }
}
impl SpanExporter for Exporter {
    async fn export(&self, mut batch: Vec<SpanData>) -> OTelSdkResult {
        for span in &mut batch {
            sanitize(span);
        }
        let result = self.inner.export(batch).await;
        if result.is_err() && !self.warned.swap(true, Ordering::Relaxed) {
            tracing::warn!(name: "telemetry_trace_export_failed", "telemetry dropped; agent work continues");
        }
        result
    }
    fn set_resource(&mut self, resource: &Resource) {
        self.inner.set_resource(resource);
    }
    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }
}
fn sanitize(span: &mut SpanData) {
    span.attributes.retain(crate::safe_fields::attribute);
    if !span
        .attributes
        .iter()
        .any(|attr| attr.key.as_str() == "outcome")
    {
        span.attributes
            .push(opentelemetry::KeyValue::new("outcome", "abandoned"));
    }
    if let opentelemetry::trace::Status::Error { .. } = &span.status {
        span.status = opentelemetry::trace::Status::error("");
    }
    span.events = opentelemetry_sdk::trace::SpanEvents::default();
    span.links = opentelemetry_sdk::trace::SpanLinks::default();
    let kind = span
        .attributes
        .iter()
        .find(|attr| attr.key.as_str() == "kind");
    span.name = kind.map_or("work".into(), |attr| {
        attr.value.as_str().into_owned().into()
    });
}
