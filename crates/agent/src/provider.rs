//! The provider seam: one settled response for one thread.

use serde_json::Value;

use crate::event::Sink;
use crate::model::ModelResponse;
use crate::thread::Thread;

/// Produce one settled response for `thread`.
///
/// Implementations own their own retries, backoff, and timeouts, and map every
/// failure they could not recover from into
/// [`StopReason::Error`](crate::StopReason::Error). A half-response must never
/// escape this seam: the loop appends whatever comes back before it classifies
/// it, so anything partial becomes part of the conversation.
///
/// `&self`, not `&mut self`, so one client can serve concurrent runs without
/// serialising them behind whichever is currently waiting on the network.
pub trait Model: Send + Sync {
    /// Check whether this target can honor a request before routing or network I/O.
    /// # Errors
    /// Returns a typed unsupported capability when a target cannot preserve the input.
    fn validate_request(
        &self,
        _thread: &Thread,
        _tools: &[Value],
    ) -> Result<(), crate::CapabilityError> {
        Ok(())
    }

    /// Declared context capacity. `None` preserves the caller's configured budget.
    fn context_window(&self) -> Option<usize> {
        None
    }

    /// Runs one response to completion, reporting progress through `sink`.
    fn generate(
        &self,
        thread: &Thread,
        tools: &[Value],
    ) -> impl Future<Output = ModelResponse> + Send;

    /// Validate terminal output before settlement. The driver journals this feedback
    /// at most once per answer, then requests one correction through this same seam.
    fn output_feedback(&self, _thread: &Thread) -> impl Future<Output = Option<String>> + Send {
        async { None }
    }

    /// Produce a response while optionally reporting provisional progress.
    ///
    /// Settled-only implementations inherit this adapter. Streaming providers
    /// override it and must stop if the sink rejects an event.
    fn generate_stream<S: Sink + Send>(
        &self,
        thread: &Thread,
        tools: &[Value],
        _sink: &mut S,
    ) -> impl Future<Output = Result<ModelResponse, S::Error>> + Send {
        async move { Ok(self.generate(thread, tools).await) }
    }
}
