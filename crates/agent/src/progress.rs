//! Provisional progress a streaming [`Model`](crate::Model) coalesces for a
//! [`Sink`]. Adapters classify their wire deltas; this owns batching and order.

use crate::event::{Event, Sink};

/// Enough buffered text to be worth one surface update.
const FLUSH_CHARS: usize = 512;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum Kind {
    #[default]
    Text,
    Reasoning,
}

/// One buffer of a single kind, so events leave in the order deltas arrived.
#[derive(Default, Debug)]
pub struct Progress {
    kind: Kind,
    pending: String,
}

impl Progress {
    /// Buffer assistant text, emitted as [`Event::ModelDelta`].
    ///
    /// # Errors
    /// Returns the sink's error when a flush does not land.
    pub async fn text<S: Sink + Send>(
        &mut self,
        delta: &str,
        sink: &mut S,
    ) -> Result<(), S::Error> {
        self.push(Kind::Text, delta, sink).await
    }

    /// Buffer provider-visible reasoning, emitted as [`Event::ReasoningDelta`].
    /// Never pass encrypted payloads or signatures.
    ///
    /// # Errors
    /// Returns the sink's error when a flush does not land.
    pub async fn reasoning<S: Sink + Send>(
        &mut self,
        delta: &str,
        sink: &mut S,
    ) -> Result<(), S::Error> {
        self.push(Kind::Reasoning, delta, sink).await
    }

    /// Emit whatever is buffered. Call it when the stream idles and before
    /// returning the settled response.
    ///
    /// # Errors
    /// Returns the sink's error when the write does not land.
    pub async fn flush<S: Sink + Send>(&mut self, sink: &mut S) -> Result<(), S::Error> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let delta = std::mem::take(&mut self.pending);
        let event = match self.kind {
            Kind::Text => Event::ModelDelta { delta },
            Kind::Reasoning => Event::ReasoningDelta { delta },
        };
        sink.emit(event).await
    }

    async fn push<S: Sink + Send>(
        &mut self,
        kind: Kind,
        delta: &str,
        sink: &mut S,
    ) -> Result<(), S::Error> {
        if delta.is_empty() {
            return Ok(());
        }
        if kind != self.kind {
            self.flush(sink).await?;
            self.kind = kind;
        }
        self.pending.push_str(delta);
        if self.pending.chars().count() >= FLUSH_CHARS {
            self.flush(sink).await?;
        }
        Ok(())
    }
}
