//! Amazon Bedrock runtime calls through the official SDK. Each stream chunk
//! is one complete Messages event.

use std::pin::Pin;

use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::primitives::Blob;
use aws_sdk_bedrockruntime::primitives::event_stream::EventReceiver;
use aws_sdk_bedrockruntime::types::ResponseStream;
use aws_sdk_bedrockruntime::types::error::ResponseStreamError;

use crate::failure::Failure;
use crate::runtime_error::{invoke_failure, stream_failure};
use crate::sse::invalid;
use crate::stream::Read;

pub(crate) type Receiver = EventReceiver<ResponseStream, ResponseStreamError>;

const JSON: &str = "application/json";

pub(crate) async fn send(client: &Client, model: &str, body: Vec<u8>) -> Result<Receiver, Failure> {
    let request = client
        .invoke_model_with_response_stream()
        .model_id(model)
        .content_type(JSON)
        .body(Blob::new(body));
    // The SDK's send future nests generics deeply enough that, inlined into
    // the host's async state machine, rustc overflows its layout depth.
    // Erasing the type here keeps that concern out of every caller.
    let send: Pin<Box<dyn Future<Output = _> + Send + '_>> = Box::pin(request.send());
    agent::observation::run(
        agent::observation::span(agent::observation::Kind::Provider),
        send,
        agent::observation::result_outcome,
    )
    .await
    .map(|output| output.body)
    .map_err(|error| invoke_failure(&error))
}

pub(crate) async fn recv(receiver: &mut Receiver) -> Result<Read, Failure> {
    match receiver
        .recv()
        .await
        .map_err(|error| stream_failure(&error))?
    {
        None => Ok(Read::End),
        Some(ResponseStream::Chunk(part)) => Ok(Read::Chunk(
            part.bytes()
                .map_or_else(Vec::new, |bytes| bytes.as_ref().to_vec()),
        )),
        Some(_) => Err(invalid("unexpected Bedrock stream event")),
    }
}
