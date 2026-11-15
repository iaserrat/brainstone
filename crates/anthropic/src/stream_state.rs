use std::collections::BTreeMap;

use agent::{ModelResponse, Progress, Sink, StopReason};

use crate::content_block::Block;
use crate::decode::Decoder;
use crate::failure::{Failure, TRANSPORT_ERROR};
use crate::stream::{Error, Read, Source, read};
use crate::stream_event::Item;

pub(crate) struct State {
    pub(crate) decoder: Decoder,
    pub(crate) progress: Progress,
    pub(crate) blocks: BTreeMap<usize, Block>,
    pub(crate) stop: Option<StopReason>,
}

impl State {
    pub(crate) fn new(endpoint: &crate::Endpoint) -> Self {
        Self {
            decoder: Decoder::for_endpoint(endpoint),
            progress: Progress::default(),
            blocks: BTreeMap::new(),
            stop: None,
        }
    }

    pub(crate) async fn run<S: Sink + Send>(
        mut self,
        mut source: Source,
        sink: &mut S,
    ) -> Result<ModelResponse, Error<S::Error>> {
        loop {
            match read(&mut source).await.map_err(Error::Provider)? {
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
                        TRANSPORT_ERROR,
                        "stream ended before message_stop",
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
            let item = crate::stream_event::parse(&data).map_err(Error::Provider)?;
            if let Some(done) = self.apply(item, sink).await? {
                return Ok(Some(done));
            }
        }
        Ok(None)
    }

    async fn apply<S: Sink + Send>(
        &mut self,
        item: Item,
        sink: &mut S,
    ) -> Result<Option<ModelResponse>, Error<S::Error>> {
        match item {
            Item::Start { index, block } => self.start(index, &block).map_err(Error::Provider)?,
            Item::Delta { index, delta } => {
                self.delta(index, &delta).map_err(Error::Provider)?;
                self.progress(&delta, sink).await?;
            }
            Item::BlockStop(index) => self.block_stop(index).map_err(Error::Provider)?,
            Item::StopReason(reason) => self.set_stop(&reason).map_err(Error::Provider)?,
            Item::Error(error) => return Err(Error::Provider(error)),
            Item::Done => {
                self.flush(sink).await?;
                return self.finish().map(Some).map_err(Error::Provider);
            }
            Item::Ignore => {}
        }
        Ok(None)
    }
}
