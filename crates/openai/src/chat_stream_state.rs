use crate::{
    failure::{CODE_TRANSPORT, Failure},
    sse::Decoder,
    stream::{Error, Read, read},
};
use agent::{ModelResponse, Progress, Sink};
use reqwest::Response;

const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct State {
    decoder: Decoder,
    output: crate::chat_stream_output::Output,
    progress: Progress,
    bytes: usize,
}

impl State {
    pub(crate) async fn run<S: Sink + Send>(
        mut self,
        mut response: Response,
        sink: &mut S,
    ) -> Result<ModelResponse, Error<S::Error>> {
        loop {
            match read(&mut response).await.map_err(Error::Provider)? {
                Read::Idle => self.flush(sink).await?,
                Read::Chunk(chunk) => {
                    self.push(&chunk).map_err(Error::Provider)?;
                    if let Some(response) = self.events(sink).await? {
                        return Ok(response);
                    }
                }
                Read::End => {
                    return Err(Error::Provider(Failure::new(
                        CODE_TRANSPORT,
                        "chat stream ended before [DONE]",
                    )));
                }
            }
        }
    }

    fn push(&mut self, chunk: &[u8]) -> Result<(), Failure> {
        self.bytes = self.bytes.saturating_add(chunk.len());
        if self.bytes > MAX_RESPONSE_BYTES {
            return Err(crate::sse::invalid("chat response exceeds size limit"));
        }
        self.decoder.push(chunk)
    }

    async fn events<S: Sink + Send>(
        &mut self,
        sink: &mut S,
    ) -> Result<Option<ModelResponse>, Error<S::Error>> {
        while let Some(data) = self.decoder.next().map_err(Error::Provider)? {
            if data == "[DONE]" {
                let response = self.output.settled().map_err(Error::Provider)?;
                self.flush(sink).await?;
                return Ok(Some(response));
            }
            self.fragment(&data, sink).await?;
        }
        Ok(None)
    }

    async fn fragment<S: Sink + Send>(
        &mut self,
        data: &str,
        sink: &mut S,
    ) -> Result<(), Error<S::Error>> {
        let value = serde_json::from_str(data)
            .map_err(|error| Error::Provider(crate::sse::invalid(error.to_string())))?;
        let text = self.output.push(&value).map_err(Error::Provider)?;
        let reasoning = self.output.reasoning_delta(&value);
        self.progress
            .reasoning(&reasoning, sink)
            .await
            .map_err(Error::Sink)?;
        self.progress.text(&text, sink).await.map_err(Error::Sink)
    }

    async fn flush<S: Sink + Send>(&mut self, sink: &mut S) -> Result<(), Error<S::Error>> {
        self.progress.flush(sink).await.map_err(Error::Sink)
    }
}
#[cfg(test)]
#[path = "chat_reasoning_tests.rs"]
mod tests;
