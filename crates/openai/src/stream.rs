use std::time::Duration;

use agent::{ModelResponse, Sink};
use reqwest::Response as HttpResponse;

use crate::failure::Failure;
use crate::stream_state::State;

const FLUSH_INTERVAL: Duration = Duration::from_millis(100);

pub(crate) enum Error<E> {
    Provider(Failure),
    Sink(E),
}

pub(crate) async fn decode<S: Sink + Send>(
    response: HttpResponse,
    sink: &mut S,
) -> Result<ModelResponse, Error<S::Error>> {
    let response = crate::transport::require_success(response)
        .await
        .map_err(Error::Provider)?;
    State::default().run(response, sink).await
}

pub(crate) enum Read {
    Idle,
    Chunk(Vec<u8>),
    End,
}

pub(crate) async fn read(response: &mut HttpResponse) -> Result<Read, Failure> {
    match tokio::time::timeout(FLUSH_INTERVAL, response.chunk()).await {
        Err(_) => Ok(Read::Idle),
        Ok(Ok(Some(bytes))) => Ok(Read::Chunk(bytes.to_vec())),
        Ok(Ok(None)) => Ok(Read::End),
        Ok(Err(error)) => Err(error.into()),
    }
}
