use agent::{ModelResponse, Progress, Sink};
use reqwest::Response as HttpResponse;

use crate::failure::{CODE_TRANSPORT, Failure};
use crate::sse::Decoder;
use crate::stream::{Error, Read, read};
use crate::stream_event::{Item, parse};

#[derive(Default)]
pub(crate) struct State {
    decoder: Decoder,
    progress: Progress,
}

impl State {
    pub(crate) async fn run<S: Sink + Send>(
        mut self,
        mut response: HttpResponse,
        sink: &mut S,
    ) -> Result<ModelResponse, Error<S::Error>> {
        loop {
            match read(&mut response).await.map_err(Error::Provider)? {
                Read::Idle => self.flush(sink).await?,
                Read::Chunk(chunk) => {
                    self.decoder.push(&chunk).map_err(Error::Provider)?;
                    if let Some(done) = self.events(sink).await? {
                        return Ok(done);
                    }
                }
                // A clean EOF before the terminal event is a cut connection, not
                // a malformed reply: transient, so the caller may retry.
                Read::End => {
                    return Err(Error::Provider(Failure::new(
                        CODE_TRANSPORT,
                        "stream ended before a terminal event",
                    )));
                }
            }
        }
    }

    async fn events<S: Sink + Send>(
        &mut self,
        sink: &mut S,
    ) -> Result<Option<ModelResponse>, Error<S::Error>> {
        while let Some(data) = self.decoder.next().map_err(Error::Provider)? {
            if let Some(done) = self.event(&data, sink).await? {
                return Ok(Some(done));
            }
        }
        Ok(None)
    }

    async fn event<S: Sink + Send>(
        &mut self,
        data: &str,
        sink: &mut S,
    ) -> Result<Option<ModelResponse>, Error<S::Error>> {
        match parse(data).map_err(Error::Provider)? {
            Item::Delta(delta) => {
                self.progress
                    .text(&delta, sink)
                    .await
                    .map_err(Error::Sink)?;
                Ok(None)
            }
            Item::Reasoning(delta) => {
                self.progress
                    .reasoning(&delta, sink)
                    .await
                    .map_err(Error::Sink)?;
                Ok(None)
            }
            Item::Done(response) => {
                self.flush(sink).await?;
                response.into_model().map(Some).map_err(Error::Provider)
            }
            Item::Ignore => Ok(None),
        }
    }

    async fn flush<S: Sink + Send>(&mut self, sink: &mut S) -> Result<(), Error<S::Error>> {
        self.progress.flush(sink).await.map_err(Error::Sink)
    }
}
