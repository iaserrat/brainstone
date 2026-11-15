use opentelemetry_sdk::{
    error::OTelSdkResult,
    metrics::{Temporality, data::ResourceMetrics, exporter::PushMetricExporter},
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub(crate) struct Exporter {
    inner: opentelemetry_otlp::MetricExporter,
    warned: AtomicBool,
}
impl Exporter {
    pub(crate) fn new(inner: opentelemetry_otlp::MetricExporter) -> Self {
        Self {
            inner,
            warned: AtomicBool::new(false),
        }
    }
}
impl PushMetricExporter for Exporter {
    async fn export(&self, metrics: &ResourceMetrics) -> OTelSdkResult {
        let result = self.inner.export(metrics).await;
        if result.is_err() && !self.warned.swap(true, Ordering::Relaxed) {
            tracing::warn!(name: "telemetry_metric_export_failed", "telemetry dropped; agent work continues");
        }
        result
    }
    fn force_flush(&self) -> OTelSdkResult {
        self.inner.force_flush()
    }
    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }
    fn temporality(&self) -> Temporality {
        self.inner.temporality()
    }
}
