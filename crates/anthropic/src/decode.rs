//! One event decoder per wire framing, yielding Messages event JSON.

use crate::Endpoint;
use crate::failure::Failure;

pub(crate) enum Decoder {
    Sse(crate::sse::Decoder),
    /// Bedrock runtime delivers one whole event per chunk.
    Whole(Option<String>),
}

impl Decoder {
    pub(crate) fn for_endpoint(endpoint: &Endpoint) -> Self {
        match endpoint {
            Endpoint::Messages(_) => Self::Sse(crate::sse::Decoder::default()),
            Endpoint::BedrockRuntime(_) => Self::Whole(None),
        }
    }

    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<(), Failure> {
        match self {
            Self::Sse(decoder) => decoder.push(bytes),
            Self::Whole(event) => {
                let text = String::from_utf8(bytes.to_vec())
                    .map_err(|error| crate::sse::invalid(error.to_string()))?;
                *event = Some(text);
                Ok(())
            }
        }
    }

    pub(crate) fn next(&mut self) -> Result<Option<String>, Failure> {
        match self {
            Self::Sse(decoder) => decoder.next(),
            Self::Whole(event) => Ok(event.take()),
        }
    }
}
