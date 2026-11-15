use std::time::Duration;

use agent::{ModelResponse, Sink, Thread};
use serde_json::Value;

use crate::failure::{Failure, INVALID_REQUEST};
use crate::model::AnthropicModel;
use crate::request::Request;
use crate::{Endpoint, runtime};

const FLUSH_INTERVAL: Duration = Duration::from_millis(100);

pub(crate) async fn generate<S: Sink + Send>(
    model: &AnthropicModel,
    thread: &Thread,
    tools: &[Value],
    sink: &mut S,
) -> Result<ModelResponse, S::Error> {
    let selected_model = thread.profile().model().unwrap_or(&model.model);
    let effort = thread.profile().effort().map(Into::into).or(model.effort);
    let source = match open(model, selected_model, effort, thread, tools).await {
        Ok(source) => source,
        Err(error) => return Ok(error.response()),
    };
    match crate::stream_state::State::new(&model.endpoint)
        .run(source, sink)
        .await
    {
        Ok(response) => Ok(response),
        Err(Error::Provider(error)) => Ok(error.response()),
        Err(Error::Sink(error)) => Err(error),
    }
}

async fn open(
    model: &AnthropicModel,
    selected: &str,
    effort: Option<crate::Effort>,
    thread: &Thread,
    tools: &[Value],
) -> Result<Source, Failure> {
    let request = Request::new(model, selected, effort, thread, tools)?;
    let body = serde_json::to_vec(&request)
        .map_err(|error| Failure::new(INVALID_REQUEST, error.to_string()))?;
    match &model.endpoint {
        Endpoint::Messages(url) => crate::transport::send(model, url, &body)
            .await
            .map(Source::Http),
        Endpoint::BedrockRuntime(client) => runtime::send(client, selected, body)
            .await
            .map(Source::Bedrock),
    }
}

pub(crate) enum Error<E> {
    Provider(Failure),
    Sink(E),
}

pub(crate) enum Source {
    Http(reqwest::Response),
    Bedrock(runtime::Receiver),
}

pub(crate) enum Read {
    Idle,
    Chunk(Vec<u8>),
    End,
}

pub(crate) async fn read(source: &mut Source) -> Result<Read, Failure> {
    match source {
        Source::Http(response) => {
            match tokio::time::timeout(FLUSH_INTERVAL, response.chunk()).await {
                Err(_) => Ok(Read::Idle),
                Ok(Ok(Some(bytes))) => Ok(Read::Chunk(bytes.to_vec())),
                Ok(Ok(None)) => Ok(Read::End),
                Ok(Err(error)) => Err(error.into()),
            }
        }
        Source::Bedrock(receiver) => {
            match tokio::time::timeout(FLUSH_INTERVAL, runtime::recv(receiver)).await {
                Err(_) => Ok(Read::Idle),
                Ok(read) => read,
            }
        }
    }
}
