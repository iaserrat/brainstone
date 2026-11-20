//! Provisional text and visible thinking handed to live surfaces while a
//! response streams. Signatures and redacted thinking never become progress.

use agent::Sink;
use serde_json::Value;

use crate::stream::Error;
use crate::stream_state::State;

impl State {
    /// Report a content delta the block already validated.
    pub(crate) async fn progress<S: Sink + Send>(
        &mut self,
        delta: &Value,
        sink: &mut S,
    ) -> Result<(), Error<S::Error>> {
        let text = |field: &str| delta[field].as_str().unwrap_or_default();
        match delta["type"].as_str() {
            Some("text_delta") => self.progress.text(text("text"), sink).await,
            Some("thinking_delta") => self.progress.reasoning(text("thinking"), sink).await,
            _ => Ok(()),
        }
        .map_err(Error::Sink)
    }

    pub(crate) async fn flush<S: Sink + Send>(
        &mut self,
        sink: &mut S,
    ) -> Result<(), Error<S::Error>> {
        self.progress.flush(sink).await.map_err(Error::Sink)
    }
}
