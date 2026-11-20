use crate::providers::Providers;
use std::time::Duration;

/// Owns exporter workers. Drain application work, then explicitly shut down.
/// Dropping without shutdown does not guarantee delivery; crashes lose telemetry.
#[must_use]
pub struct Telemetry {
    pub(crate) providers: Providers,
    pub(crate) logs: crate::logging_worker::Worker,
}
const SHUTDOWN_BUDGET: Duration = Duration::from_secs(10);
impl std::fmt::Debug for Telemetry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Telemetry").finish_non_exhaustive()
    }
}

impl Telemetry {
    /// Flush on an OS worker, waiting at most ten seconds. Reports safe
    /// diagnostics to stderr; export failure never changes the application result.
    /// Must run within Tokio after all producers stop. SIGKILL cannot flush.
    pub async fn shutdown(self) {
        let (done, flush) = tokio::sync::oneshot::channel();
        let started = std::thread::Builder::new()
            .name("brainstone-telemetry-shutdown".into())
            .spawn(move || {
                self.flush();
                let _ = done.send(());
            });
        if started.is_err()
            || !matches!(
                tokio::time::timeout(SHUTDOWN_BUDGET, flush).await,
                Ok(Ok(()))
            )
        {
            tracing::warn!(name: "telemetry_shutdown_timeout", "pending telemetry may be lost");
        }
    }
    fn flush(self) {
        if let Some(traces) = self.providers.traces
            && traces
                .shutdown_with_timeout(crate::providers::EXPORT_TIMEOUT)
                .is_err()
        {
            tracing::warn!(name: "telemetry_trace_shutdown_failed", "pending telemetry may be lost");
        }
        if self.providers.metrics.shutdown().is_err() {
            tracing::warn!(name: "telemetry_metric_shutdown_failed", "pending telemetry may be lost");
        }
        self.logs.shutdown();
    }
}
